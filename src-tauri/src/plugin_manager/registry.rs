//! # Domain 1: Discovery, Registry & Health Check
//!
//! **Role**: Plugin Lifecycle & Package Manager (Imperative Shell & Facade)
//! **Deliverable**: `plugin_manager/registry.rs`
//!
//! Responsible for:
//! - M2.2: Plugin Registration & indexing
//! - M2.3: Plugin Discovery (scan directory, validate manifest & sub-contracts)
//! - Querying compatible plugins based on image/AOI target
//! - Running fast environment/dependency healthcheck
//!
//! ## Purity Boundary
//!
//! This module is the **Imperative Shell** and public/crate facade over
//! [`super::registry_core`]. All filesystem I/O, Tauri IPC command handling,
//! `tokio::sync::RwLock` state synchronization, `chrono::Utc::now()` clock
//! reads, `eprintln!` diagnostics, and async subprocess health checks live here.
//! Pure domain logic (manifest/subcontract validation, compatibility predicates,
//! parallel candidate assembly, and immutable state transitions) is delegated to
//! [`super::registry_core`].
//!
//! Refer to: `src-tauri/src/plugin_manager/DOCS/01_DOMAIN_1_DISCOVERY_REGISTRY.md`

use std::path::Path;

use super::error::{CommandError, PluginError};
use super::models::{
    HealthStatusDto, InputsSpec, PluginBundle, PluginDetailsDto, PluginManifest, PluginSource,
    PluginSummaryDto, TargetDescriptorDto,
};
use super::registry_core::{
    build_discovery_output, extract_subcontract_paths, with_healthcheck_state,
    DiscoveredPluginCandidate, HealthcheckStateInput,
};
pub use super::registry_core::{is_compatible, PluginRegistry};
pub(crate) use super::registry_core::{
    is_safe_relative_path, parse_and_validate_inputs, parse_and_validate_manifest,
    parse_and_validate_outputs, parse_and_validate_parameters,
};

// ---------------------------------------------------------------------------
// 1. TAURI IPC COMMANDS (pub — registered in lib.rs invoke_handler)
// ---------------------------------------------------------------------------

/// Returns a lightweight summary list of all installed plugins for UI cards and menus.
///
/// # Purity
/// Impure Tauri IPC command handler; delegates async `AppState` access to `get_installed_plugins_internal`.
#[tauri::command]
pub async fn get_installed_plugins(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    get_installed_plugins_internal(&state)
        .await
        .map_err(Into::into)
}

/// Internal helper that snapshots the registry under a short read lock and computes
/// summaries off the Tokio async runtime thread via `spawn_blocking`.
///
/// # Purity
/// Imperative shell: acquires async read lock on `app_state.plugins` and dispatches
/// pure [`PluginRegistry::get_summaries`] work onto `tokio::task::spawn_blocking`.
pub(crate) async fn get_installed_plugins_internal(
    app_state: &crate::AppState,
) -> Result<Vec<PluginSummaryDto>, PluginError> {
    let snapshot = {
        let registry = app_state.plugins.read().await;
        registry.clone()
    };
    tokio::task::spawn_blocking(move || snapshot.get_summaries())
        .await
        .map_err(|e| PluginError::StateError(format!("Failed to compute plugin summaries: {}", e)))
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
///
/// # Purity
/// Impure Tauri IPC command handler; delegates disk rescan and registry mutation to `reload_plugins_internal`.
#[tauri::command]
pub async fn reload_plugins(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    reload_plugins_internal(&state).await.map_err(Into::into)
}

/// Internal helper that rescans plugins and computes summaries in `spawn_blocking`
/// before acquiring the registry write lock to swap the new registry in.
///
/// # Purity
/// Imperative shell: performs blocking filesystem discovery via `tokio::task::spawn_blocking`
/// and mutates `app_state.plugins` under a write lock.
pub(crate) async fn reload_plugins_internal(
    app_state: &crate::AppState,
) -> Result<Vec<PluginSummaryDto>, PluginError> {
    let state_path = app_state.state_path.clone();
    let (new_registry, summaries) = tokio::task::spawn_blocking(move || {
        let new_registry = rescan_plugins_from_state_path(state_path.as_deref());
        let summaries = new_registry.get_summaries();
        (new_registry, summaries)
    })
    .await
    .map_err(|e| PluginError::StateError(format!("Failed to reload plugins: {}", e)))?;

    {
        let mut registry = app_state.plugins.write().await;
        *registry = new_registry;
    }

    Ok(summaries)
}

/// Resolves the plugins directory from an optional `state_path` and rebuilds the registry.
///
/// # Purity
/// Impure filesystem I/O: inspects parent directory and scans plugin bundles on disk.
fn rescan_plugins_from_state_path(state_path: Option<&Path>) -> PluginRegistry {
    if let Some(state_path) = state_path {
        if let Some(parent) = state_path.parent() {
            if parent.is_dir() {
                return crate::init_plugin_registry_from_dir(parent);
            }
        }
    }
    crate::init_plugin_registry()
}

/// Internal helper to rescan the plugins directory, honoring custom `state_path` if configured.
///
/// # Purity
/// Impure test facade wrapping [`rescan_plugins_from_state_path`].
#[cfg(test)]
pub(crate) fn rescan_plugins_internal(app_state: &crate::AppState) -> PluginRegistry {
    rescan_plugins_from_state_path(app_state.state_path.as_deref())
}

/// Returns the fully resolved bundle including manifest and sub-contracts for a specific plugin.
///
/// # Purity
/// Impure Tauri IPC command handler: acquires async read lock on `state.plugins` and delegates
/// DTO projection to pure [`PluginRegistry::get_details`].
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
///
/// # Purity
/// Impure Tauri IPC command handler; delegates async `AppState` access to `query_compatible_plugins_internal`.
#[tauri::command]
pub async fn query_compatible_plugins(
    target: TargetDescriptorDto,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    query_compatible_plugins_internal(target, &state)
        .await
        .map_err(Into::into)
}

/// Internal helper that snapshots the registry under a short read lock and evaluates
/// compatible plugins off the Tokio async runtime thread via `spawn_blocking`.
///
/// # Purity
/// Imperative shell: acquires async read lock on `app_state.plugins` and dispatches
/// pure [`PluginRegistry::query_compatible`] work onto `tokio::task::spawn_blocking`.
pub(crate) async fn query_compatible_plugins_internal(
    target: TargetDescriptorDto,
    app_state: &crate::AppState,
) -> Result<Vec<PluginSummaryDto>, PluginError> {
    let snapshot = {
        let registry = app_state.plugins.read().await;
        registry.clone()
    };
    tokio::task::spawn_blocking(move || snapshot.query_compatible(&target))
        .await
        .map_err(|e| PluginError::StateError(format!("Failed to query compatible plugins: {}", e)))
}

/// Runs a fast pre-flight liveness and dependency health check on a plugin entrypoint.
///
/// # Purity
/// Impure Tauri IPC command handler; delegates subprocess check and state persistence to
/// `check_plugin_health_internal`.
#[tauri::command]
pub async fn check_plugin_health(
    plugin_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<HealthStatusDto, CommandError> {
    check_plugin_health_internal(&plugin_id, &state)
        .await
        .map_err(Into::into)
}

/// Internal helper for running health checks against a plugin entrypoint and updating in-memory
/// and persisted plugin state.
///
/// # Purity
/// Imperative shell: reads bundle snapshot under lock, runs async [`execute_healthcheck`]
/// subprocess (no `rayon`), reads `chrono::Utc::now()`, delegates pure state transition to
/// [`with_healthcheck_state`], persists `plugin_state.json`, and updates `app_state.plugins`
/// under a short write lock.
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
        let store =
            crate::plugin_manager::lifecycle::load_plugin_state(&state_path).unwrap_or_default();
        let now_iso = chrono::Utc::now().to_rfc3339();
        let source = if bundle.source == "user" {
            PluginSource::User
        } else {
            PluginSource::Builtin
        };
        let next_store = with_healthcheck_state(
            &store,
            &HealthcheckStateInput {
                plugin_id,
                bundle_version: &bundle.manifest.metadata.version,
                source,
                enabled: bundle.enabled,
                status: &result.status,
                error_msg: result.error.as_deref(),
                now_iso: &now_iso,
            },
        );
        let _ = crate::plugin_manager::lifecycle::save_plugin_state(&state_path, &next_store);
    }

    // Update in-memory health status under short write lock
    {
        let mut registry = app_state.plugins.write().await;
        if let Some(b) = registry.plugins.get_mut(plugin_id) {
            b.health_status = result.status.clone();
        }
    }

    Ok(result)
}

/// Executes the pre-flight entrypoint health check subprocess (`<entrypoint> --healthcheck`) with a 10s timeout.
///
/// # Purity
/// Impure async process I/O: probes filesystem paths, spawns child subprocesses, reads stdout/stderr
/// pipes, and enforces a Tokio timeout.
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

/// Reads a single subcontract JSON file inside `dir` after checking relative sandbox safety.
///
/// # Purity
/// Impure filesystem I/O (`Path::exists`, `std::fs::read_to_string`) guarded by pure
/// [`is_safe_relative_path`].
fn read_subcontract_file(dir: &Path, rel_path: &str, label: &str) -> Result<String, PluginError> {
    if !is_safe_relative_path(rel_path) {
        return Err(PluginError::SubcontractInvalid {
            path: rel_path.to_string(),
            message: format!("{} path escapes sandbox", label),
        });
    }
    let full_path = dir.join(rel_path);
    if !full_path.exists() {
        return Err(PluginError::SubcontractInvalid {
            path: rel_path.to_string(),
            message: format!("{} file not found at {:?}", label, full_path),
        });
    }
    std::fs::read_to_string(&full_path).map_err(|e| PluginError::SubcontractInvalid {
        path: rel_path.to_string(),
        message: e.to_string(),
    })
}

/// Reads and schema-validates the three sub-contracts (`parameters.json`, `inputs.json`, `outputs.json`).
///
/// Acts as the **Imperative Shell** around the pure functions `parse_and_validate_parameters`,
/// `parse_and_validate_inputs`, and `parse_and_validate_outputs`.
///
/// **Pre-condition**: Files referenced in manifest exist within `dir`.
/// **Post-condition**: All 3 sub-contracts parsed and schema-validated.
///
/// # Purity
/// Imperative shell: reads subcontract files from disk via [`read_subcontract_file`] and
/// delegates validation to pure parsers in [`super::registry_core`].
pub(crate) fn validate_subcontracts(
    dir: &Path,
    manifest: &PluginManifest,
) -> Result<(serde_json::Value, InputsSpec, serde_json::Value), PluginError> {
    let params_raw_str = read_subcontract_file(dir, &manifest.parameters, "Parameters")?;
    let params_val = parse_and_validate_parameters(&params_raw_str, &manifest.parameters)?;

    let inputs_raw_str = read_subcontract_file(dir, &manifest.inputs, "Inputs")?;
    let inputs_spec = parse_and_validate_inputs(&inputs_raw_str, &manifest.inputs)?;

    let outputs_raw_str = read_subcontract_file(dir, &manifest.outputs, "Outputs")?;
    let outputs_val = parse_and_validate_outputs(&outputs_raw_str, &manifest.outputs)?;

    Ok((params_val, inputs_spec, outputs_val))
}

/// Scans the `plugins/` directory and builds a vector of valid `PluginBundle`s.
///
/// Fault-tolerant: Corrupt or unreadable plugin folders are logged and skipped.
///
/// # Purity
/// Imperative shell: scans `plugins_dir`, reads manifests and subcontract files into
/// [`DiscoveredPluginCandidate`] values, delegates parallel validation and deterministic
/// sorting to pure [`build_discovery_output`], and logs warnings via `eprintln!`.
pub(crate) fn discover_plugins(plugins_dir: &Path) -> Result<Vec<PluginBundle>, PluginError> {
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(plugins_dir).map_err(|e| PluginError::Io(e.to_string()))?;
    let mut candidates = Vec::new();

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

        let manifest_raw = match std::fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "Warning: failed to read manifest at {:?}: {}",
                    manifest_path, e
                );
                continue;
            }
        };

        let (parameters_result, inputs_result, outputs_result) =
            match extract_subcontract_paths(&manifest_raw) {
                Some((params_rel, inputs_rel, outputs_rel)) => (
                    read_subcontract_file(&path, &params_rel, "Parameters")
                        .map_err(|e| e.to_string()),
                    read_subcontract_file(&path, &inputs_rel, "Inputs").map_err(|e| e.to_string()),
                    read_subcontract_file(&path, &outputs_rel, "Outputs")
                        .map_err(|e| e.to_string()),
                ),
                None => {
                    let msg = "Manifest missing subcontract paths".to_string();
                    (Err(msg.clone()), Err(msg.clone()), Err(msg))
                }
            };

        candidates.push(DiscoveredPluginCandidate {
            plugin_dir: path,
            manifest_raw,
            parameters_result,
            inputs_result,
            outputs_result,
        });
    }

    let output = build_discovery_output(candidates);
    for warning in output.warnings {
        eprintln!("{}", warning);
    }

    Ok(output.bundles)
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
