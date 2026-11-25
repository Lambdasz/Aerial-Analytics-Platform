//! # Domain 1: Discovery, Registry & Health Check
//!
//! **Role**: Plugin Lifecycle & Package Manager (Discovery & Registry)
//! **Deliverable**: `plugin_manager/registry.rs`
//!
//! Responsible for:
//! - M2.2: Plugin Registration & indexing
//! - M2.3: Plugin Discovery (scan directory, validate manifest & sub-contracts)
//! - Querying compatible plugins based on image/AOI target
//! - Running fast environment/dependency healthcheck
//!
//! Refer to: `src-tauri/src/plugin_manager/DOCS/01_DOMAIN_1_DISCOVERY_REGISTRY.md`

use std::path::Path;

use super::error::{CommandError, PluginError};
use super::models::{
    HealthStatusDto, InputsSpec, PluginBundle, PluginDetailsDto, PluginManifest, PluginSummaryDto,
    TargetDescriptorDto,
};
pub(crate) use super::registry_core::{
    assemble_plugin_bundle, is_safe_relative_path, parse_and_validate_inputs,
    parse_and_validate_manifest, parse_and_validate_outputs, parse_and_validate_parameters,
};
pub use super::registry_core::{is_compatible, PluginRegistry};

// ---------------------------------------------------------------------------
// 1. TAURI IPC COMMANDS (pub — registered in lib.rs invoke_handler)
// ---------------------------------------------------------------------------

/// Returns a lightweight summary list of all installed plugins for UI cards and menus.
#[tauri::command]
pub async fn get_installed_plugins(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    let registry = state.plugins.read().await;
    Ok(registry.get_summaries())
}

/// Rescans the plugins directory from disk and updates the in-memory registry.
///
/// Discovers plugins on disk, validates their manifests and subcontracts,
/// restores persisted status from `plugin_state.json`, and updates the shared registry.
///
/// # Arguments
/// * `state` - Injected Tauri application state containing the plugin registry.
///
/// # Pre-condition
/// - `state` is initialized.
///
/// # Post-condition
/// - In-memory `PluginRegistry` is updated with freshly discovered plugins from disk.
/// - Returns a list of all installed plugin summaries.
///
/// # Errors
/// - This function returns `Ok` even if individual plugins fail validation (they are skipped and logged).
///
/// # Panics
/// - This function does not panic.
#[tauri::command]
pub async fn reload_plugins(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    let new_registry = rescan_plugins_internal(&state);
    let mut registry = state.plugins.write().await;
    *registry = new_registry;
    Ok(registry.get_summaries())
}

/// Internal helper to rescan the plugins directory, honoring custom `state_path` if configured.
pub(crate) fn rescan_plugins_internal(app_state: &crate::AppState) -> PluginRegistry {
    if let Some(state_path) = &app_state.state_path {
        if let Some(parent) = state_path.parent() {
            if parent.is_dir() {
                return crate::init_plugin_registry_from_dir(parent);
            }
        }
    }
    crate::init_plugin_registry()
}

/// Returns the fully resolved bundle including manifest and sub-contracts for a specific plugin.
#[tauri::command]
pub async fn get_plugin_details(
    plugin_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<PluginDetailsDto, CommandError> {
    let registry = state.plugins.read().await;
    registry
        .get_details(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id).into())
}

/// Filters and returns only installed plugins capable of processing the given image or AOI target.
#[tauri::command]
pub async fn query_compatible_plugins(
    target: TargetDescriptorDto,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    let registry = state.plugins.read().await;
    Ok(registry.query_compatible(&target))
}

/// Runs a fast pre-flight liveness and dependency health check on a plugin entrypoint.
#[tauri::command]
pub async fn check_plugin_health(
    plugin_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<HealthStatusDto, CommandError> {
    check_plugin_health_internal(&plugin_id, &state)
        .await
        .map_err(Into::into)
}

/// Internal helper for running health checks against a plugin entrypoint and updating in-memory status.
pub(crate) async fn check_plugin_health_internal(
    plugin_id: &str,
    app_state: &crate::AppState,
) -> Result<HealthStatusDto, PluginError> {
    let bundle = {
        let registry = app_state.plugins.read().await;
        registry
            .plugins
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?
    };

    let result = execute_healthcheck(&bundle).await;

    // Update in-memory health status
    {
        let mut registry = app_state.plugins.write().await;
        if let Some(b) = registry.plugins.get_mut(plugin_id) {
            b.health_status = result.status.clone();
        }
    }

    // Persist healthcheck status to plugin_state.json if state_path is configured or default exists
    let state_path_opt = app_state.state_path.clone().or_else(|| {
        let candidate = crate::plugin_manager::lifecycle::default_state_path();
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    });

    if let Some(state_path) = state_path_opt {
        let mut store =
            crate::plugin_manager::lifecycle::load_plugin_state(&state_path).unwrap_or_default();
        store.updated_at = chrono::Utc::now().to_rfc3339();
        let record = store
            .plugins
            .entry(plugin_id.to_string())
            .or_insert_with(|| crate::plugin_manager::models::PluginStateRecord {
                version: bundle.manifest.metadata.version.clone(),
                source: if bundle.source == "user" {
                    crate::plugin_manager::models::PluginSource::User
                } else {
                    crate::plugin_manager::models::PluginSource::Builtin
                },
                enabled: bundle.enabled,
                installed_at: chrono::Utc::now().to_rfc3339(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({}),
                stats: Default::default(),
            });
        record.last_healthcheck = Some(crate::plugin_manager::models::CachedHealthcheck {
            status: result.status.clone(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            message: result.error.clone(),
        });
        let _ = crate::plugin_manager::lifecycle::save_plugin_state(&state_path, &store);
    }

    Ok(result)
}

/// Executes the pre-flight entrypoint health check subprocess (`<entrypoint> --healthcheck`) with a 10s timeout.
pub(crate) async fn execute_healthcheck(bundle: &PluginBundle) -> HealthStatusDto {
    let plugin_id = bundle.manifest.metadata.id.clone();
    let version = bundle.manifest.metadata.version.clone();
    // Absolute paths only: a relative program with a relative `current_dir`
    // is resolved against the child's working directory by glibc, which
    // silently doubles path segments and fails to spawn.
    let plugin_dir =
        std::path::absolute(&bundle.install_path).unwrap_or_else(|_| bundle.install_path.clone());
    let entrypoint = &bundle.manifest.runtime.entrypoint;
    let healthcheck_flag = bundle
        .manifest
        .runtime
        .healthcheck_flag
        .as_deref()
        .unwrap_or("--healthcheck");

    let mut cmd = match bundle.manifest.runtime.runtime_type.as_str() {
        "python" => {
            let venv_python = if cfg!(windows) {
                plugin_dir.join(".venv").join("Scripts").join("python.exe")
            } else {
                plugin_dir.join(".venv").join("bin").join("python")
            };

            let interpreter = if venv_python.exists() {
                venv_python
            } else if cfg!(windows) {
                std::path::PathBuf::from("python")
            } else {
                std::path::PathBuf::from("python3")
            };

            let script_path = plugin_dir.join(entrypoint);
            if !script_path.exists() {
                return HealthStatusDto {
                    status: "unhealthy".to_string(),
                    plugin_id,
                    version,
                    details: None,
                    error: Some(format!(
                        "Plugin Python entrypoint script not found: {:?}",
                        script_path
                    )),
                };
            }

            let mut command = tokio::process::Command::new(interpreter);
            command
                .arg(&script_path)
                .arg(healthcheck_flag)
                .current_dir(plugin_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            #[cfg(unix)]
            {
                command.process_group(0);
            }
            command
        }
        "binary" => {
            let mut bin_path = plugin_dir.join(entrypoint);
            if cfg!(windows) && !entrypoint.ends_with(".exe") {
                let exe_candidate = plugin_dir.join(format!("{}.exe", entrypoint));
                if exe_candidate.exists() {
                    bin_path = exe_candidate;
                }
            }

            if !bin_path.exists() {
                return HealthStatusDto {
                    status: "unhealthy".to_string(),
                    plugin_id,
                    version,
                    details: None,
                    error: Some(format!(
                        "Plugin binary entrypoint not found: {:?}",
                        bin_path
                    )),
                };
            }

            let mut command = tokio::process::Command::new(&bin_path);
            command
                .arg(healthcheck_flag)
                .current_dir(plugin_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            #[cfg(unix)]
            {
                command.process_group(0);
            }
            command
        }
        other => {
            return HealthStatusDto {
                status: "unhealthy".to_string(),
                plugin_id,
                version,
                details: None,
                error: Some(format!(
                    "Unsupported runtime type for healthcheck: '{}'",
                    other
                )),
            };
        }
    };

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return HealthStatusDto {
                status: "unhealthy".to_string(),
                plugin_id,
                version,
                details: None,
                error: Some(format!("Failed to spawn healthcheck subprocess: {}", e)),
            };
        }
    };

    let child_pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        if let Some(mut out) = stdout {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = out.read_to_end(&mut buf).await;
            buf
        } else {
            Vec::new()
        }
    });

    let stderr_task = tokio::spawn(async move {
        if let Some(mut err) = stderr {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = err.read_to_end(&mut buf).await;
            buf
        } else {
            Vec::new()
        }
    });

    let wait_res = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait()).await;

    match wait_res {
        Err(_) => {
            let _ = child.kill().await;
            if let Some(pid) = child_pid {
                crate::plugin_manager::executor::kill_process_tree(pid);
            }
            stdout_task.abort();
            stderr_task.abort();
            HealthStatusDto {
                status: "unhealthy".to_string(),
                plugin_id,
                version,
                details: None,
                error: Some("Healthcheck execution timed out after 10 seconds".to_string()),
            }
        }
        Ok(Err(io_err)) => HealthStatusDto {
            status: "unhealthy".to_string(),
            plugin_id,
            version,
            details: None,
            error: Some(format!("I/O error waiting on healthcheck: {}", io_err)),
        },
        Ok(Ok(status)) => {
            let stdout_bytes = stdout_task.await.unwrap_or_default();
            let stderr_bytes = stderr_task.await.unwrap_or_default();
            let stdout_str = String::from_utf8_lossy(&stdout_bytes);
            let stderr_str = String::from_utf8_lossy(&stderr_bytes);

            if status.success() {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
                    let status_str = json_val
                        .get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("healthy")
                        .to_string();
                    let err_msg = json_val
                        .get("error")
                        .and_then(|e| e.as_str())
                        .map(ToString::to_string);
                    HealthStatusDto {
                        status: status_str,
                        plugin_id,
                        version,
                        details: Some(json_val),
                        error: err_msg,
                    }
                } else {
                    HealthStatusDto {
                        status: "healthy".to_string(),
                        plugin_id,
                        version,
                        details: None,
                        error: None,
                    }
                }
            } else {
                let (error_msg, details) = if let Ok(json_val) =
                    serde_json::from_str::<serde_json::Value>(&stderr_str)
                {
                    let err = json_val
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or(&stderr_str)
                        .to_string();
                    (err, Some(json_val))
                } else if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&stdout_str)
                {
                    let err = json_val
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or(&stdout_str)
                        .to_string();
                    (err, Some(json_val))
                } else if !stderr_str.trim().is_empty() {
                    (stderr_str.trim().to_string(), None)
                } else if !stdout_str.trim().is_empty() {
                    (stdout_str.trim().to_string(), None)
                } else {
                    (
                        format!("Healthcheck process exited with status: {}", status),
                        None,
                    )
                };

                HealthStatusDto {
                    status: "unhealthy".to_string(),
                    plugin_id,
                    version,
                    details,
                    error: Some(error_msg),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. DISCOVERY & SUBCONTRACT I/O (Imperative Shell)
// ---------------------------------------------------------------------------

/// Reads and schema-validates the three sub-contracts (`parameters.json`, `inputs.json`, `outputs.json`).
///
/// Acts as the **Imperative Shell** around the pure functions `parse_and_validate_parameters`,
/// `parse_and_validate_inputs`, and `parse_and_validate_outputs`.
///
/// **Pre-condition**: Files referenced in manifest exist within `dir`.  
/// **Post-condition**: All 3 sub-contracts parsed and schema-validated.
#[allow(dead_code)]
pub(crate) fn validate_subcontracts(
    dir: &Path,
    manifest: &PluginManifest,
) -> Result<(serde_json::Value, InputsSpec, serde_json::Value), PluginError> {
    if !is_safe_relative_path(&manifest.parameters) {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.parameters.clone(),
            message: "Parameters path escapes sandbox".to_string(),
        });
    }
    let params_path = dir.join(&manifest.parameters);
    if !params_path.exists() {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.parameters.clone(),
            message: format!("Parameters file not found at {:?}", params_path),
        });
    }
    let params_raw_str =
        std::fs::read_to_string(&params_path).map_err(|e| PluginError::SubcontractInvalid {
            path: manifest.parameters.clone(),
            message: e.to_string(),
        })?;
    let params_val = parse_and_validate_parameters(&params_raw_str, &manifest.parameters)?;

    if !is_safe_relative_path(&manifest.inputs) {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.inputs.clone(),
            message: "Inputs path escapes sandbox".to_string(),
        });
    }
    let inputs_path = dir.join(&manifest.inputs);
    if !inputs_path.exists() {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.inputs.clone(),
            message: format!("Inputs file not found at {:?}", inputs_path),
        });
    }
    let inputs_raw_str =
        std::fs::read_to_string(&inputs_path).map_err(|e| PluginError::SubcontractInvalid {
            path: manifest.inputs.clone(),
            message: e.to_string(),
        })?;
    let inputs_spec = parse_and_validate_inputs(&inputs_raw_str, &manifest.inputs)?;

    if !is_safe_relative_path(&manifest.outputs) {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.outputs.clone(),
            message: "Outputs path escapes sandbox".to_string(),
        });
    }
    let outputs_path = dir.join(&manifest.outputs);
    if !outputs_path.exists() {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.outputs.clone(),
            message: format!("Outputs file not found at {:?}", outputs_path),
        });
    }
    let outputs_raw_str =
        std::fs::read_to_string(&outputs_path).map_err(|e| PluginError::SubcontractInvalid {
            path: manifest.outputs.clone(),
            message: e.to_string(),
        })?;
    let outputs_val = parse_and_validate_outputs(&outputs_raw_str, &manifest.outputs)?;

    Ok((params_val, inputs_spec, outputs_val))
}

/// Scans the `plugins/` directory and builds a vector of valid `PluginBundle`s.
///
/// Fault-tolerant: Corrupt or unreadable plugin folders are logged and skipped.
#[allow(dead_code)]
pub(crate) fn discover_plugins(plugins_dir: &Path) -> Result<Vec<PluginBundle>, PluginError> {
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(plugins_dir).map_err(|e| PluginError::Io(e.to_string()))?;
    let mut bundles = Vec::new();

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: skipping unreadable directory entry: {}", e);
                continue;
            }
        };

        // Absolute paths only: spawning a relative program together with a
        // relative `current_dir` makes glibc resolve the program against the
        // *new* working directory, so `../plugins/x/../plugins/x/...` fails.
        let path = entry.path().canonicalize().unwrap_or_else(|_| entry.path());
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        let manifest_str = match std::fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "Warning: failed to read manifest at {:?}: {}",
                    manifest_path, e
                );
                continue;
            }
        };

        let manifest = match parse_and_validate_manifest(&manifest_str) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "Warning: skipping invalid manifest at {:?}: {}",
                    manifest_path, e
                );
                continue;
            }
        };

        let (parameters_raw, inputs_spec, outputs_raw) =
            match validate_subcontracts(&path, &manifest) {
                Ok(subs) => subs,
                Err(e) => {
                    eprintln!(
                        "Warning: skipping plugin at {:?} due to invalid subcontracts: {}",
                        path, e
                    );
                    continue;
                }
            };

        let inputs_path = path.join(&manifest.inputs);
        let inputs_raw = std::fs::read_to_string(&inputs_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::to_value(&inputs_spec).unwrap_or_default());

        bundles.push(assemble_plugin_bundle(
            path,
            manifest,
            inputs_spec,
            inputs_raw,
            parameters_raw,
            outputs_raw,
        ));
    }

    bundles.sort_by(|a, b| a.manifest.metadata.id.cmp(&b.manifest.metadata.id));
    Ok(bundles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::InputsSpec;
    use std::path::PathBuf;

    fn mock_bundle(id: &str) -> PluginBundle {
        PluginBundle {
            install_path: PathBuf::from("/mock/plugins").join(id),
            manifest: PluginManifest {
                schema: None,
                manifest_version: 1,
                metadata: crate::plugin_manager::models::PluginMetadata {
                    id: id.to_string(),
                    name: format!("Plugin {}", id),
                    version: "1.0.0".to_string(),
                    author: Some("Author".to_string()),
                    license: Some("MIT".to_string()),
                    description: "Description".to_string(),
                    category: Some("Vegetation".to_string()),
                    tags: vec!["vegetation".to_string()],
                    min_core_version: None,
                },
                runtime: crate::plugin_manager::models::PluginRuntime {
                    runtime_type: "python".to_string(),
                    entrypoint: "main.py".to_string(),
                    min_python_version: None,
                    dependencies_file: None,
                    healthcheck_flag: None,
                },
                parameters: "parameters.json".to_string(),
                inputs: "inputs.json".to_string(),
                outputs: "outputs.json".to_string(),
                execution: Some(crate::plugin_manager::models::PluginExecutionConfig {
                    timeout_seconds: 60,
                    provides_output_dir_arg: true,
                    concurrency: "sequential".to_string(),
                    gpu: None,
                }),
            },
            inputs_spec: InputsSpec {
                granularity: "single_image".to_string(),
                supported_mime_types: vec!["image/jpeg".to_string(), "image/tiff".to_string()],
                requires_metadata: Vec::new(),
                optional_metadata: Vec::new(),
                supports_aoi: true,
                aoi_format: None,
            },
            inputs_raw: serde_json::json!({
                "granularity": "single_image",
                "supported_mime_types": ["image/jpeg", "image/tiff"],
                "supports_aoi": true
            }),
            parameters_raw: serde_json::json!({}),
            outputs_raw: serde_json::json!({}),
            enabled: true,
            source: "builtin".to_string(),
            health_status: "healthy".to_string(),
            custom_parameters: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn test_app_state_has_plugins_and_concurrent_access() {
        let state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(PluginRegistry::new())),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: None,
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(std::path::PathBuf::from("/tmp")),
        };

        {
            let mut reg = state.plugins.write().await;
            reg.register(mock_bundle("rgb-exg"));
        }

        let reg = state.plugins.read().await;
        assert_eq!(reg.get_summaries().len(), 1);
    }

    #[test]
    fn test_validate_subcontracts_valid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();

        std::fs::write(
            plugin_dir.join("parameters.json"),
            r#"{"parameters": [{"name": "thresh", "label": "Threshold", "type": "number", "default": 0.5}]}"#,
        ).unwrap();

        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();

        std::fs::write(
            plugin_dir.join("outputs.json"),
            r#"{"metrics": [], "artifacts": []}"#,
        )
        .unwrap();

        let manifest = PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: crate::plugin_manager::models::PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: None,
        };

        let res = validate_subcontracts(plugin_dir, &manifest);
        assert!(res.is_ok());
        let (params, inputs_spec, outputs) = res.unwrap();
        assert!(params.get("parameters").is_some());
        assert_eq!(inputs_spec.granularity, "single_image");
        assert!(outputs.get("metrics").is_some());
    }

    #[test]
    fn test_validate_subcontracts_missing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();

        // only create inputs.json, omitting parameters.json and outputs.json
        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();

        let manifest = PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: crate::plugin_manager::models::PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: None,
        };

        let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
        assert!(matches!(err, PluginError::SubcontractInvalid { .. }));
    }

    #[test]
    fn test_validate_subcontracts_empty_mime_types() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();

        std::fs::write(plugin_dir.join("parameters.json"), r#"{"parameters": []}"#).unwrap();

        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": []}"#,
        )
        .unwrap();

        std::fs::write(plugin_dir.join("outputs.json"), r#"{"metrics": []}"#).unwrap();

        let manifest = PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: crate::plugin_manager::models::PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: None,
        };

        let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
        assert!(matches!(err, PluginError::SubcontractInvalid { .. }));
    }

    #[test]
    fn test_discover_plugins_success_and_fault_tolerance() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugins_root = temp_dir.path();

        // 1. Valid plugin A
        let dir_a = plugins_root.join("plugin-a");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::write(
            dir_a.join("manifest.json"),
            r#"{
                "manifest_version": 1,
                "metadata": {"id": "plugin-a", "name": "Plugin A", "version": "1.0.0", "description": "A"},
                "runtime": {"type": "python", "entrypoint": "main.py"},
                "parameters": "parameters.json",
                "inputs": "inputs.json",
                "outputs": "outputs.json"
            }"#,
        ).unwrap();
        std::fs::write(dir_a.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
        std::fs::write(
            dir_a.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();
        std::fs::write(dir_a.join("outputs.json"), r#"{}"#).unwrap();

        // 2. Corrupt plugin B (invalid manifest JSON)
        let dir_b = plugins_root.join("plugin-b");
        std::fs::create_dir_all(&dir_b).unwrap();
        std::fs::write(dir_b.join("manifest.json"), "{ broken").unwrap();

        // 3. Plugin C (missing subcontracts)
        let dir_c = plugins_root.join("plugin-c");
        std::fs::create_dir_all(&dir_c).unwrap();
        std::fs::write(
            dir_c.join("manifest.json"),
            r#"{
                "manifest_version": 1,
                "metadata": {"id": "plugin-c", "name": "Plugin C", "version": "1.0.0", "description": "C"},
                "runtime": {"type": "python", "entrypoint": "main.py"},
                "parameters": "missing_params.json",
                "inputs": "inputs.json",
                "outputs": "outputs.json"
            }"#,
        ).unwrap();

        // 4. Valid plugin D
        let dir_d = plugins_root.join("plugin-d");
        std::fs::create_dir_all(&dir_d).unwrap();
        std::fs::write(
            dir_d.join("manifest.json"),
            r#"{
                "manifest_version": 1,
                "metadata": {"id": "plugin-d", "name": "Plugin D", "version": "2.0.0", "description": "D"},
                "runtime": {"type": "binary", "entrypoint": "detector"},
                "parameters": "parameters.json",
                "inputs": "inputs.json",
                "outputs": "outputs.json"
            }"#,
        ).unwrap();
        std::fs::write(dir_d.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
        std::fs::write(
            dir_d.join("inputs.json"),
            r#"{"granularity": "batch", "supported_mime_types": ["image/tiff"]}"#,
        )
        .unwrap();
        std::fs::write(dir_d.join("outputs.json"), r#"{}"#).unwrap();

        let discovered = discover_plugins(plugins_root).expect("discover_plugins should succeed");
        assert_eq!(
            discovered.len(),
            2,
            "Fault-tolerant discovery must skip corrupted plugins"
        );
        let ids: Vec<&str> = discovered
            .iter()
            .map(|b| b.manifest.metadata.id.as_str())
            .collect();
        assert_eq!(ids, vec!["plugin-a", "plugin-d"]);
    }

    #[test]
    fn test_discover_plugins_nonexistent_dir() {
        let fake_path = Path::new("/path/that/does/not/exist/definitely_not");
        let res = discover_plugins(fake_path);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_check_plugin_health_internal_missing_plugin() {
        let app_state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(PluginRegistry::new())),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: None,
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(std::path::PathBuf::from("/tmp")),
        };

        let err = check_plugin_health_internal("missing-plugin", &app_state)
            .await
            .unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_execute_healthcheck_python_healthy() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();
        let script_path = plugin_dir.join("main.py");
        std::fs::write(
            &script_path,
            r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "healthy", "python_version": "3.10"}))
    sys.exit(0)
sys.exit(1)
"#,
        )
        .unwrap();

        let mut bundle = mock_bundle("test-health");
        bundle.install_path = plugin_dir.to_path_buf();
        bundle.manifest.runtime.entrypoint = "main.py".to_string();

        let status = execute_healthcheck(&bundle).await;
        assert_eq!(status.status, "healthy");
        assert_eq!(status.plugin_id, "test-health");
        assert!(status.details.is_some());
        assert!(status.error.is_none());
    }

    #[tokio::test]
    async fn test_execute_healthcheck_python_unhealthy() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();
        let script_path = plugin_dir.join("main.py");
        std::fs::write(
            &script_path,
            r#"
import sys
if "--healthcheck" in sys.argv:
    sys.stderr.write("Missing OpenCV dependency\n")
    sys.exit(1)
sys.exit(1)
"#,
        )
        .unwrap();

        let mut bundle = mock_bundle("test-unhealthy");
        bundle.install_path = plugin_dir.to_path_buf();
        bundle.manifest.runtime.entrypoint = "main.py".to_string();

        let status = execute_healthcheck(&bundle).await;
        assert_eq!(status.status, "unhealthy");
        assert!(status.error.is_some());
        assert!(status.error.unwrap().contains("Missing OpenCV"));
    }

    #[tokio::test]
    async fn test_execute_healthcheck_missing_entrypoint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut bundle = mock_bundle("test-missing-entry");
        bundle.install_path = temp_dir.path().to_path_buf();
        bundle.manifest.runtime.entrypoint = "nonexistent.py".to_string();

        let status = execute_healthcheck(&bundle).await;
        assert_eq!(status.status, "unhealthy");
        assert!(status.error.is_some());
    }

    #[tokio::test]
    async fn test_check_plugin_health_updates_in_memory_bundle_status() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();
        let script_path = plugin_dir.join("main.py");
        std::fs::write(
            &script_path,
            r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "healthy"}))
    sys.exit(0)
"#,
        )
        .unwrap();

        let mut bundle = mock_bundle("test-update-health");
        bundle.install_path = plugin_dir.to_path_buf();
        bundle.manifest.runtime.entrypoint = "main.py".to_string();
        bundle.health_status = "unknown".to_string();

        let mut registry = PluginRegistry::new();
        registry.register(bundle);

        let app_state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(registry)),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: None,
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(std::path::PathBuf::from("/tmp")),
        };

        let res = check_plugin_health_internal("test-update-health", &app_state).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().status, "healthy");

        let reg = app_state.plugins.read().await;
        assert_eq!(
            reg.plugins.get("test-update-health").unwrap().health_status,
            "healthy"
        );
    }

    #[tokio::test]
    async fn test_execute_healthcheck_parses_json_error_on_success_status() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();
        let script_path = plugin_dir.join("main.py");
        std::fs::write(
            &script_path,
            r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "unhealthy", "error": "Model weights missing"}))
    sys.exit(0)
"#,
        )
        .unwrap();

        let mut bundle = mock_bundle("test-unhealthy-zero-exit");
        bundle.install_path = plugin_dir.to_path_buf();
        bundle.manifest.runtime.entrypoint = "main.py".to_string();

        let status = execute_healthcheck(&bundle).await;
        assert_eq!(status.status, "unhealthy");
        assert_eq!(status.error.as_deref(), Some("Model weights missing"));
    }

    #[tokio::test]
    async fn test_check_plugin_health_flushes_to_state_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();
        let script_path = plugin_dir.join("main.py");
        std::fs::write(
            &script_path,
            r#"
import sys, json
if "--healthcheck" in sys.argv:
    print(json.dumps({"status": "healthy"}))
    sys.exit(0)
"#,
        )
        .unwrap();

        let mut bundle = mock_bundle("test-health-persist");
        bundle.install_path = plugin_dir.to_path_buf();
        bundle.manifest.runtime.entrypoint = "main.py".to_string();

        let mut registry = PluginRegistry::new();
        registry.register(bundle);

        let state_path = temp_dir.path().join("plugin_state.json");
        let app_state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(registry)),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: Some(state_path.clone()),
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(std::path::PathBuf::from("/tmp")),
        };

        let res = check_plugin_health_internal("test-health-persist", &app_state).await;
        assert!(res.is_ok());

        assert!(
            state_path.exists(),
            "State file must exist after health check"
        );
        let store = crate::plugin_manager::lifecycle::load_plugin_state(&state_path).unwrap();
        let record = store
            .plugins
            .get("test-health-persist")
            .expect("record should exist");
        let hc = record
            .last_healthcheck
            .as_ref()
            .expect("last_healthcheck should be set");
        assert_eq!(hc.status, "healthy");
    }

    #[test]
    fn test_validate_subcontracts_enforces_parameters_schema() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();

        // Parameter missing required 'type' and 'default'
        std::fs::write(
            plugin_dir.join("parameters.json"),
            r#"{"parameters": [{"name": "thresh", "label": "Threshold"}]}"#,
        )
        .unwrap();
        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();
        std::fs::write(plugin_dir.join("outputs.json"), r#"{}"#).unwrap();

        let manifest = PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: crate::plugin_manager::models::PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: None,
        };

        let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
        assert!(matches!(err, PluginError::SubcontractInvalid { .. }));
    }

    #[test]
    fn test_validate_subcontracts_enforces_inputs_granularity_and_mime() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path();

        std::fs::write(plugin_dir.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
        // Invalid granularity
        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "invalid_granularity", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();
        std::fs::write(plugin_dir.join("outputs.json"), r#"{}"#).unwrap();

        let manifest = PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: crate::plugin_manager::models::PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: None,
        };

        let err = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
        assert!(matches!(err, PluginError::SubcontractInvalid { .. }));

        // Invalid MIME type
        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["text/plain"]}"#,
        )
        .unwrap();
        let err_mime = validate_subcontracts(plugin_dir, &manifest).unwrap_err();
        assert!(matches!(err_mime, PluginError::SubcontractInvalid { .. }));
    }

    #[test]
    fn test_rescan_plugins_discovers_new_plugin_on_disk() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugins_root = temp_dir.path();

        // 1. Initially create plugin A
        let dir_a = plugins_root.join("plugin-a");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::write(
            dir_a.join("manifest.json"),
            r#"{
                "manifest_version": 1,
                "metadata": {"id": "plugin-a", "name": "Plugin A", "version": "1.0.0", "description": "A"},
                "runtime": {"type": "python", "entrypoint": "main.py"},
                "parameters": "parameters.json",
                "inputs": "inputs.json",
                "outputs": "outputs.json"
            }"#,
        )
        .unwrap();
        std::fs::write(dir_a.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
        std::fs::write(
            dir_a.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();
        std::fs::write(dir_a.join("outputs.json"), r#"{}"#).unwrap();

        let state_path = plugins_root.join("plugin_state.json");
        let app_state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::init_plugin_registry_from_dir(plugins_root),
            )),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: Some(state_path),
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(temp_dir.path().join("library")),
        };

        // Initially only plugin A
        let initial_reg = rescan_plugins_internal(&app_state);
        assert_eq!(initial_reg.plugins.len(), 1);
        assert!(initial_reg.plugins.contains_key("plugin-a"));

        // 2. Add plugin B to disk while app is running
        let dir_b = plugins_root.join("plugin-b");
        std::fs::create_dir_all(&dir_b).unwrap();
        std::fs::write(
            dir_b.join("manifest.json"),
            r#"{
                "manifest_version": 1,
                "metadata": {"id": "plugin-b", "name": "Plugin B", "version": "1.0.0", "description": "B"},
                "runtime": {"type": "python", "entrypoint": "main.py"},
                "parameters": "parameters.json",
                "inputs": "inputs.json",
                "outputs": "outputs.json"
            }"#,
        )
        .unwrap();
        std::fs::write(dir_b.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
        std::fs::write(
            dir_b.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();
        std::fs::write(dir_b.join("outputs.json"), r#"{}"#).unwrap();

        // 3. Rescan discovers plugin B
        let rescanned = rescan_plugins_internal(&app_state);
        assert_eq!(rescanned.plugins.len(), 2);
        assert!(rescanned.plugins.contains_key("plugin-a"));
        assert!(rescanned.plugins.contains_key("plugin-b"));
    }
}
