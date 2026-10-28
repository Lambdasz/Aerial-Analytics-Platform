//! # Domain 2: Lifecycle & State Persistence
//!
//! **Role**: Plugin Lifecycle & Package Manager (Lifecycle & State)
//! **Deliverable**: `plugin_manager/lifecycle.rs`
//!
//! Responsible for:
//! - M2.4: Safe ZIP archive installation with Zip-Slip path traversal defense
//! - M2.5: Enable/disable plugin status toggling
//! - Uninstallation and removal of user-installed plugins
//! - Atomic persistence of mutable state (`plugin_state.json`)
//!
//! Refer to: `src-tauri/src/plugin_manager/DOCS/02_DOMAIN_2_LIFECYCLE_STATE.md`

use std::path::{Path, PathBuf};

use super::error::{CommandError, PluginError};
use super::models::{PluginStateStore, PluginSummaryDto};

// ---------------------------------------------------------------------------
// 1. TAURI IPC COMMANDS (pub — registered in lib.rs invoke_handler)
// ---------------------------------------------------------------------------

/// Installs a new plugin from a user-provided `.zip` archive file.
///
/// **Pre-condition**: `archive_path` points to a valid `.zip` file on disk.  
/// **Post-condition**: Safely extracted into `plugins/<id>/`, validated, and registered in state.
#[tauri::command]
pub async fn install_plugin(
    archive_path: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<PluginSummaryDto, CommandError> {
    install_plugin_internal(Path::new(&archive_path), &state)
        .await
        .map_err(Into::into)
}

/// Permanently uninstalls an existing user-installed plugin.
///
/// **Pre-condition**: `plugin_id` exists; source is `user` (not `builtin`); plugin is not running.  
/// **Post-condition**: Plugin folder deleted; entry purged from `plugin_state.json`.
#[tauri::command]
pub async fn remove_plugin(
    plugin_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    remove_plugin_internal(&plugin_id, &state)
        .await
        .map_err(Into::into)
}

/// Toggles active/inactive status of a plugin and flushes changes to `plugin_state.json`.
///
/// Provides the administrative enable/disable lifecycle control exposed to the frontend via Tauri IPC (`invoke("set_plugin_status", { pluginId, enabled })`).
/// Invoked from Module 11 (Extension Manager) when a user activates or deactivates a plugin card or table row,
/// preventing disabled plugins from being selected in analysis workflows or executed by the job supervisor.
///
/// # Arguments
/// * `_plugin_id` - Unique identifier string of the target plugin to toggle.
/// * `_enabled` - Target status boolean (`true` to enable, `false` to disable).
/// * `_state` - Injected Tauri application state containing plugin registry and state stores.
///
/// # Pre-condition
/// - `_plugin_id` is registered in memory, and the OS AppData state directory is accessible for atomic file persistence.
///
/// # Post-condition
/// - Updates the in-memory `enabled` flag of the specified plugin and atomically flushes the updated `PluginStateStore` to `plugin_state.json` via a temporary file write and atomic rename.
///
/// # Errors
/// - Returns `CommandError` with error code `PLUGIN_NOT_FOUND` if `_plugin_id` is not found in the registry or state, or `STATE_ERROR` if serializing and atomically persisting to `plugin_state.json` fails.
///
/// # Panics
/// - This function does not panic.
#[tauri::command]
pub async fn set_plugin_status(
    plugin_id: String,
    enabled: bool,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    set_plugin_status_internal(&plugin_id, enabled, &state)
        .await
        .map_err(Into::into)
}

/// Internal helper for toggling plugin status and flushing changes to plugin_state.json.
pub(crate) async fn set_plugin_status_internal(
    plugin_id: &str,
    enabled: bool,
    app_state: &crate::AppState,
) -> Result<(), PluginError> {
    let mut registry = app_state.plugins.write().await;
    let prev_enabled = registry
        .plugins
        .get(plugin_id)
        .map(|b| b.enabled)
        .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

    registry.set_status(plugin_id, enabled)?;

    let state_path = app_state
        .state_path
        .clone()
        .unwrap_or_else(default_state_path);
    let mut store = load_plugin_state(&state_path).unwrap_or_default();
    store.updated_at = chrono::Utc::now().to_rfc3339();

    let record = store
        .plugins
        .entry(plugin_id.to_string())
        .or_insert_with(|| {
            let bundle = registry.plugins.get(plugin_id);
            super::models::PluginStateRecord {
                version: bundle
                    .map(|b| b.manifest.metadata.version.clone())
                    .unwrap_or_else(|| "1.0.0".to_string()),
                source: if bundle.map(|b| b.source.as_str()) == Some("user") {
                    super::models::PluginSource::User
                } else {
                    super::models::PluginSource::Builtin
                },
                enabled,
                installed_at: chrono::Utc::now().to_rfc3339(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({}),
                stats: Default::default(),
            }
        });
    record.enabled = enabled;

    if let Err(err) = save_plugin_state(&state_path, &store) {
        let _ = registry.set_status(plugin_id, prev_enabled);
        return Err(err);
    }

    Ok(())
}

/// Pure helper to validate plugin identifier format matching `^[a-z0-9-_]+$`.
pub(crate) fn is_valid_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Applies persisted state records to an in-memory plugin registry.
pub(crate) fn apply_state_to_registry(
    registry: &mut super::registry::PluginRegistry,
    store: &PluginStateStore,
) {
    for (id, record) in &store.plugins {
        if let Some(bundle) = registry.plugins.get_mut(id) {
            bundle.enabled = record.enabled;
            bundle.source = match record.source {
                super::models::PluginSource::Builtin => "builtin".to_string(),
                super::models::PluginSource::User => "user".to_string(),
            };
            if let Some(ref hc) = record.last_healthcheck {
                bundle.health_status = hc.status.clone();
            }
        }
    }
}

/// Fallback default path for plugin_state.json if none is specified in AppState.
pub(crate) fn default_state_path() -> PathBuf {
    let candidate_dirs = [
        PathBuf::from("plugins"),
        PathBuf::from("../plugins"),
        PathBuf::from("../../plugins"),
    ];

    for dir in &candidate_dirs {
        if dir.is_dir() {
            return dir.join("plugin_state.json");
        }
    }

    PathBuf::from("plugins").join("plugin_state.json")
}

/// Updates and persists custom parameter overrides configured by the user.
///
/// **Pre-condition**: `plugin_id` exists; parameters conform to plugin's `parameters.json`.  
/// **Post-condition**: Overrides saved in state and persisted to disk.
#[tauri::command]
pub async fn update_plugin_config(
    plugin_id: String,
    custom_params: serde_json::Value,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    update_plugin_config_internal(&plugin_id, custom_params, &state)
        .await
        .map_err(Into::into)
}

/// Internal helper for installing a plugin from a .zip archive.
pub(crate) async fn install_plugin_internal(
    archive_path: &Path,
    app_state: &crate::AppState,
) -> Result<PluginSummaryDto, PluginError> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| PluginError::ArchiveError(format!("Cannot open archive file: {}", e)))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| PluginError::ArchiveError(format!("Invalid zip archive: {}", e)))?;

    let state_path = app_state
        .state_path
        .clone()
        .unwrap_or_else(default_state_path);
    let plugins_dir = match state_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("plugins"),
    };
    std::fs::create_dir_all(&plugins_dir)
        .map_err(|e| PluginError::Io(format!("Cannot create plugins directory: {}", e)))?;

    // Step 1: Extract into a temporary staging folder within plugins_dir (Zero disk remnants on failure)
    let temp_dir = tempfile::Builder::new()
        .prefix("staging_plugin_")
        .tempdir_in(&plugins_dir)
        .map_err(|e| PluginError::Io(format!("Cannot create staging directory: {}", e)))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PluginError::ArchiveError(e.to_string()))?;
        verify_and_extract_entry(&mut entry, temp_dir.path())?;
    }

    // Step 2: Locate plugin root (root or single top-level directory containing manifest.json)
    let staging_path = temp_dir.path();
    let plugin_source_dir = if staging_path.join("manifest.json").is_file() {
        staging_path.to_path_buf()
    } else {
        let mut candidates = Vec::new();
        if let Ok(entries) = std::fs::read_dir(staging_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("manifest.json").is_file() {
                    candidates.push(p);
                }
            }
        }
        if candidates.is_empty() {
            return Err(PluginError::ManifestInvalid {
                path: "manifest.json".to_string(),
                message: "manifest.json not found in archive root or subdirectory".to_string(),
            });
        }
        if candidates.len() > 1 {
            return Err(PluginError::ManifestInvalid {
                path: "manifest.json".to_string(),
                message: format!(
                    "Ambiguous archive: multiple plugin directories with manifest.json found ({})",
                    candidates.len()
                ),
            });
        }
        candidates.remove(0)
    };

    // Step 3: Parse and validate manifest
    let manifest_str =
        std::fs::read_to_string(plugin_source_dir.join("manifest.json")).map_err(|e| {
            PluginError::ManifestInvalid {
                path: "manifest.json".to_string(),
                message: e.to_string(),
            }
        })?;
    let manifest = super::registry::parse_and_validate_manifest(&manifest_str)?;
    let plugin_id = &manifest.metadata.id;

    // Step 4: Verify metadata.id is valid, does not overwrite builtin, and is not currently executing a job
    if !is_valid_plugin_id(plugin_id) {
        return Err(PluginError::SecurityViolation(format!(
            "Invalid plugin id '{}': must match ^[a-z0-9-_]+$",
            plugin_id
        )));
    }

    {
        let tracker = app_state.jobs.read().await;
        for job in tracker.jobs.values() {
            if job.plugin_id == *plugin_id
                && matches!(
                    job.status,
                    super::executor::JobStatusDto::Queued
                        | super::executor::JobStatusDto::Running { .. }
                )
            {
                return Err(PluginError::ExecutionError(format!(
                    "Cannot overwrite plugin '{}' because it is currently executing a job",
                    plugin_id
                )));
            }
        }
    }

    {
        let registry = app_state.plugins.read().await;
        if let Some(existing) = registry.plugins.get(plugin_id) {
            if existing.source == "builtin" {
                return Err(PluginError::SecurityViolation(format!(
                    "Cannot overwrite builtin plugin '{}'",
                    plugin_id
                )));
            }
        }
    }
    let mut store = load_plugin_state(&state_path).unwrap_or_default();
    if let Some(rec) = store.plugins.get(plugin_id) {
        if rec.source == super::models::PluginSource::Builtin {
            return Err(PluginError::SecurityViolation(format!(
                "Cannot overwrite builtin plugin '{}'",
                plugin_id
            )));
        }
    }

    // Step 5: Validate sub-contracts (parameters.json, inputs.json, outputs.json)
    let (parameters_raw, inputs_spec, outputs_raw) =
        super::registry::validate_subcontracts(&plugin_source_dir, &manifest)?;

    let inputs_path = plugin_source_dir.join(&manifest.inputs);
    let inputs_raw = std::fs::read_to_string(&inputs_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::to_value(&inputs_spec).unwrap_or_default());

    // Step 6: Atomic commit to plugins/<plugin_id>
    let target_dir = plugins_dir.join(plugin_id);
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir).map_err(|e| {
            PluginError::Io(format!(
                "Failed to clean up existing plugin directory: {}",
                e
            ))
        })?;
    }

    if plugin_source_dir == staging_path {
        let staging_path_buf = temp_dir.path().to_path_buf();
        move_dir_all(&staging_path_buf, &target_dir)?;
    } else {
        move_dir_all(&plugin_source_dir, &target_dir)?;
    }

    // Step 7: In-memory registration
    let bundle = super::models::PluginBundle {
        install_path: target_dir.clone(),
        manifest: manifest.clone(),
        inputs_spec,
        inputs_raw,
        parameters_raw,
        outputs_raw,
        enabled: true,
        source: "user".to_string(),
        health_status: "unknown".to_string(),
    };
    let summary = bundle.to_summary();

    let mut registry = app_state.plugins.write().await;
    registry.register(bundle);
    drop(registry);

    // Step 8: Update and atomically persist plugin_state.json
    store.updated_at = chrono::Utc::now().to_rfc3339();
    let record = store.plugins.entry(plugin_id.clone()).or_insert_with(|| {
        super::models::PluginStateRecord {
            version: manifest.metadata.version.clone(),
            source: super::models::PluginSource::User,
            enabled: true,
            installed_at: chrono::Utc::now().to_rfc3339(),
            last_healthcheck: None,
            custom_parameters: serde_json::json!({}),
            stats: Default::default(),
        }
    });
    record.version = manifest.metadata.version.clone();
    record.source = super::models::PluginSource::User;
    record.enabled = true;

    if let Err(err) = save_plugin_state(&state_path, &store) {
        // Rollback filesystem commit and in-memory registration on persistence failure
        let _ = std::fs::remove_dir_all(&target_dir);
        let mut reg = app_state.plugins.write().await;
        reg.plugins.remove(plugin_id);
        return Err(err);
    }

    Ok(summary)
}

/// Internal helper for removing a user plugin and purging its state.
pub(crate) async fn remove_plugin_internal(
    plugin_id: &str,
    app_state: &crate::AppState,
) -> Result<(), PluginError> {
    if !is_valid_plugin_id(plugin_id) {
        return Err(PluginError::NotFound(plugin_id.to_string()));
    }

    // 1. Verify plugin is not currently running a job
    {
        let tracker = app_state.jobs.read().await;
        for job in tracker.jobs.values() {
            if job.plugin_id == plugin_id
                && matches!(
                    job.status,
                    super::executor::JobStatusDto::Queued
                        | super::executor::JobStatusDto::Running { .. }
                )
            {
                return Err(PluginError::ExecutionError(format!(
                    "Cannot remove plugin '{}' because it is currently executing a job",
                    plugin_id
                )));
            }
        }
    }

    let state_path = app_state
        .state_path
        .clone()
        .unwrap_or_else(default_state_path);
    let mut store = load_plugin_state(&state_path).unwrap_or_default();

    let mut registry = app_state.plugins.write().await;
    let bundle_opt = registry.plugins.get(plugin_id);
    let store_rec_opt = store.plugins.get(plugin_id);

    if bundle_opt.is_none() && store_rec_opt.is_none() {
        return Err(PluginError::NotFound(plugin_id.to_string()));
    }

    let is_builtin = bundle_opt
        .map(|b| b.source.as_str() == "builtin")
        .unwrap_or(false)
        || store_rec_opt
            .map(|r| r.source == super::models::PluginSource::Builtin)
            .unwrap_or(false);

    if is_builtin {
        return Err(PluginError::SecurityViolation(format!(
            "Cannot remove builtin plugin '{}'",
            plugin_id
        )));
    }

    // 2. Remove physical directory from filesystem
    if let Some(bundle) = bundle_opt {
        if bundle.install_path.exists() {
            std::fs::remove_dir_all(&bundle.install_path).map_err(|e| {
                PluginError::Io(format!("Failed to delete plugin directory: {}", e))
            })?;
        }
    } else {
        let plugins_dir = match state_path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("plugins"),
        };
        let default_dir = plugins_dir.join(plugin_id);
        if default_dir.exists() {
            let _ = std::fs::remove_dir_all(default_dir);
        }
    }

    // 3. Remove from in-memory registry
    registry.plugins.remove(plugin_id);
    drop(registry);

    // 4. Purge from plugin_state.json and flush
    store.plugins.remove(plugin_id);
    store.updated_at = chrono::Utc::now().to_rfc3339();
    save_plugin_state(&state_path, &store)?;

    Ok(())
}

/// Validates custom parameter values against constraints declared in parameters.json.
pub(crate) fn validate_custom_parameters(
    parameters_raw: &serde_json::Value,
    custom_params: &serde_json::Value,
) -> Result<(), PluginError> {
    let custom_obj = custom_params
        .as_object()
        .ok_or_else(|| PluginError::SubcontractInvalid {
            path: "parameters.json".to_string(),
            message: "Custom parameters must be a JSON object".to_string(),
        })?;

    let empty_vec = vec![];
    let defined_params = parameters_raw
        .get("parameters")
        .and_then(|p| p.as_array())
        .unwrap_or(&empty_vec);

    for (key, val) in custom_obj {
        let param_def = defined_params
            .iter()
            .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(key))
            .ok_or_else(|| PluginError::SubcontractInvalid {
                path: "parameters.json".to_string(),
                message: format!("Unknown parameter '{}'", key),
            })?;

        let param_type = param_def.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match param_type {
            "boolean" => {
                if !val.is_boolean() {
                    return Err(PluginError::SubcontractInvalid {
                        path: "parameters.json".to_string(),
                        message: format!("Parameter '{}' must be a boolean", key),
                    });
                }
            }
            "string" => {
                if !val.is_string() {
                    return Err(PluginError::SubcontractInvalid {
                        path: "parameters.json".to_string(),
                        message: format!("Parameter '{}' must be a string", key),
                    });
                }
            }
            "select" => {
                let str_val = val
                    .as_str()
                    .ok_or_else(|| PluginError::SubcontractInvalid {
                        path: "parameters.json".to_string(),
                        message: format!(
                            "Parameter '{}' must be a string matching one of the options",
                            key
                        ),
                    })?;
                let options = param_def
                    .get("options")
                    .and_then(|o| o.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| item.as_str())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !options.contains(&str_val) {
                    return Err(PluginError::SubcontractInvalid {
                        path: "parameters.json".to_string(),
                        message: format!(
                            "Parameter '{}' value '{}' is not one of the allowed options: {:?}",
                            key, str_val, options
                        ),
                    });
                }
            }
            "number" => {
                let num = val
                    .as_f64()
                    .ok_or_else(|| PluginError::SubcontractInvalid {
                        path: "parameters.json".to_string(),
                        message: format!("Parameter '{}' must be a number", key),
                    })?;
                if let Some(min) = param_def.get("min").and_then(|m| m.as_f64()) {
                    if num < min {
                        return Err(PluginError::SubcontractInvalid {
                            path: "parameters.json".to_string(),
                            message: format!(
                                "Parameter '{}' value {} is below minimum {}",
                                key, num, min
                            ),
                        });
                    }
                }
                if let Some(max) = param_def.get("max").and_then(|m| m.as_f64()) {
                    if num > max {
                        return Err(PluginError::SubcontractInvalid {
                            path: "parameters.json".to_string(),
                            message: format!(
                                "Parameter '{}' value {} exceeds maximum {}",
                                key, num, max
                            ),
                        });
                    }
                }
            }
            other => {
                return Err(PluginError::SubcontractInvalid {
                    path: "parameters.json".to_string(),
                    message: format!(
                        "Unsupported parameter type '{}' for parameter '{}'",
                        other, key
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Internal helper for validating and saving custom parameter overrides.
pub(crate) async fn update_plugin_config_internal(
    plugin_id: &str,
    custom_params: serde_json::Value,
    app_state: &crate::AppState,
) -> Result<(), PluginError> {
    let registry = app_state.plugins.read().await;
    let bundle = registry
        .plugins
        .get(plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

    validate_custom_parameters(&bundle.parameters_raw, &custom_params)?;

    let state_path = app_state
        .state_path
        .clone()
        .unwrap_or_else(default_state_path);
    let mut store = load_plugin_state(&state_path).unwrap_or_default();
    store.updated_at = chrono::Utc::now().to_rfc3339();

    let record = store
        .plugins
        .entry(plugin_id.to_string())
        .or_insert_with(|| super::models::PluginStateRecord {
            version: bundle.manifest.metadata.version.clone(),
            source: if bundle.source == "user" {
                super::models::PluginSource::User
            } else {
                super::models::PluginSource::Builtin
            },
            enabled: bundle.enabled,
            installed_at: chrono::Utc::now().to_rfc3339(),
            last_healthcheck: None,
            custom_parameters: serde_json::json!({}),
            stats: Default::default(),
        });

    record.custom_parameters = custom_params;
    save_plugin_state(&state_path, &store)?;

    Ok(())
}

fn move_dir_all(from: &Path, to: &Path) -> Result<(), PluginError> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PluginError::Io(format!(
                "Failed to create parent directory {:?}: {}",
                parent, e
            ))
        })?;
    }

    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }

    copy_dir_recursive(from, to)?;
    let _ = std::fs::remove_dir_all(from);
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), PluginError> {
    std::fs::create_dir_all(dst)
        .map_err(|e| PluginError::Io(format!("Failed to create directory {:?}: {}", dst, e)))?;

    for entry in std::fs::read_dir(src).map_err(|e| PluginError::Io(e.to_string()))? {
        let entry = entry.map_err(|e| PluginError::Io(e.to_string()))?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else {
            std::fs::copy(&entry_path, &target_path).map_err(|e| {
                PluginError::Io(format!(
                    "Failed to copy file {:?} to {:?}: {}",
                    entry_path, target_path, e
                ))
            })?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. STATE PERSISTENCE & SECURITY HELPERS (pub(crate))
// ---------------------------------------------------------------------------

/// Reads and parses `plugin_state.json` from disk.
///
/// Returns empty default store if the state file does not yet exist.
pub(crate) fn load_plugin_state(state_path: &Path) -> Result<PluginStateStore, PluginError> {
    if !state_path.exists() {
        return Ok(PluginStateStore::default());
    }

    let raw = std::fs::read_to_string(state_path).map_err(|e| PluginError::Io(e.to_string()))?;
    let store: PluginStateStore = serde_json::from_str(&raw).map_err(|e| {
        PluginError::StateError(format!(
            "Failed to parse plugin state from {:?}: {}",
            state_path, e
        ))
    })?;

    Ok(store)
}

/// Atomically persists current plugin states to `plugin_state.json`.
///
/// Writes to a temporary file (`.tmp`) first, then performs an atomic rename.
pub(crate) fn save_plugin_state(
    state_path: &Path,
    state: &PluginStateStore,
) -> Result<(), PluginError> {
    use std::io::Write;
    let parent = state_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| PluginError::Io(e.to_string()))?;

    let temp_file_path = state_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
    let serialized = serde_json::to_string_pretty(state)
        .map_err(|e| PluginError::Serialization(e.to_string()))?;

    let mut file =
        std::fs::File::create(&temp_file_path).map_err(|e| PluginError::Io(e.to_string()))?;
    file.write_all(serialized.as_bytes())
        .map_err(|e| PluginError::Io(e.to_string()))?;
    file.sync_all()
        .map_err(|e| PluginError::Io(e.to_string()))?;

    std::fs::rename(&temp_file_path, state_path).map_err(|e| PluginError::Io(e.to_string()))?;
    Ok(())
}

/// Verifies a ZIP entry path against Zip-Slip path traversal vulnerability.
///
/// Enforces enclosed names and verifies the target path remains within `dest_dir`.
#[allow(dead_code)]
pub(crate) fn verify_and_extract_entry(
    entry: &mut zip::read::ZipFile,
    dest_dir: &Path,
) -> Result<PathBuf, PluginError> {
    if let Some(mode) = entry.unix_mode() {
        if (mode & 0o170000) == 0o120000 {
            return Err(PluginError::SecurityViolation(format!(
                "Symlinks are prohibited in plugin archives: {}",
                entry.name()
            )));
        }
    }

    let enclosed = entry.enclosed_name().ok_or_else(|| {
        PluginError::SecurityViolation(format!("Zip-Slip attempt: {}", entry.name()))
    })?;

    let target_path = dest_dir.join(enclosed);

    if !target_path.starts_with(dest_dir) {
        return Err(PluginError::SecurityViolation(format!(
            "Path escapes sandbox: {}",
            entry.name()
        )));
    }

    if entry.is_dir() || entry.name().ends_with('/') {
        std::fs::create_dir_all(&target_path).map_err(|e| {
            PluginError::Io(format!("Failed to create dir {:?}: {}", target_path, e))
        })?;
    } else {
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PluginError::Io(format!("Failed to create parent dir {:?}: {}", parent, e))
            })?;
        }
        let mut outfile = std::fs::File::create(&target_path).map_err(|e| {
            PluginError::Io(format!("Failed to create file {:?}: {}", target_path, e))
        })?;
        std::io::copy(entry, &mut outfile).map_err(|e| {
            PluginError::Io(format!("Failed to write file {:?}: {}", target_path, e))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = std::fs::set_permissions(
                    &target_path,
                    std::fs::Permissions::from_mode(mode & 0o777),
                );
            }
        }
    }

    Ok(target_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::PluginSource;
    use std::io::{Cursor, Write};
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn test_verify_and_extract_entry_safe() {
        let mut buffer = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
            writer
                .start_file("valid/file.txt", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"safe payload").unwrap();
            writer.finish().unwrap();
        }
        let dir = tempdir().unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
        let mut entry = archive.by_index(0).unwrap();
        let res = verify_and_extract_entry(&mut entry, dir.path());
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), dir.path().join("valid/file.txt"));
    }

    #[test]
    fn test_verify_and_extract_entry_zip_slip() {
        let mut buffer = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
            writer
                .start_file("../evil.sh", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"evil payload").unwrap();
            writer.finish().unwrap();
        }
        let dir = tempdir().unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
        let mut entry = archive.by_index(0).unwrap();
        let res = verify_and_extract_entry(&mut entry, dir.path());
        assert!(res.is_err());
        match res.unwrap_err() {
            PluginError::SecurityViolation(msg) => {
                assert!(msg.contains("Zip-Slip") || msg.contains("escapes"));
            }
            other => panic!("Expected SecurityViolation, got {:?}", other),
        }
    }

    #[test]
    fn test_load_plugin_state_nonexistent_returns_default() {
        let temp_dir = tempdir().unwrap();
        let missing_path = temp_dir.path().join("plugin_state.json");
        let store = load_plugin_state(&missing_path).expect("should return default on missing");
        assert_eq!(store.schema_version, 1);
        assert!(store.plugins.is_empty());
    }

    #[test]
    fn test_save_plugin_state_atomic_and_load_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("subdir").join("plugin_state.json");

        let mut store = PluginStateStore::default();
        store.plugins.insert(
            "rgb-exg".to_string(),
            crate::plugin_manager::models::PluginStateRecord {
                version: "1.0.0".to_string(),
                source: crate::plugin_manager::models::PluginSource::Builtin,
                enabled: true,
                installed_at: "2026-09-15T14:32:00Z".to_string(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({"threshold": 0.42}),
                stats: Default::default(),
            },
        );

        save_plugin_state(&state_path, &store).expect("save_plugin_state should succeed");
        assert!(
            state_path.exists(),
            "State file should exist after atomic save"
        );

        let loaded = load_plugin_state(&state_path).expect("load_plugin_state should succeed");
        assert_eq!(loaded.schema_version, store.schema_version);
        assert!(loaded.plugins.contains_key("rgb-exg"));
        assert!(loaded.plugins.get("rgb-exg").unwrap().enabled);
        assert_eq!(
            loaded.plugins.get("rgb-exg").unwrap().custom_parameters["threshold"],
            0.42
        );
    }

    #[test]
    fn test_load_plugin_state_malformed_json_returns_state_error() {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("plugin_state.json");
        std::fs::write(&state_path, "{ broken json").unwrap();

        let err = load_plugin_state(&state_path).unwrap_err();
        assert!(matches!(err, PluginError::StateError(_)));
    }

    #[tokio::test]
    async fn test_set_plugin_status_updates_registry_and_flushes_state_file() {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("plugin_state.json");

        let mut registry = crate::plugin_manager::registry::PluginRegistry::new();
        let bundle = crate::plugin_manager::models::PluginBundle {
            install_path: temp_dir.path().to_path_buf(),
            manifest: crate::plugin_manager::models::PluginManifest {
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
            },
            inputs_spec: crate::plugin_manager::models::InputsSpec {
                granularity: "single_image".to_string(),
                supported_mime_types: vec!["image/jpeg".to_string()],
                requires_metadata: vec![],
                optional_metadata: vec![],
                supports_aoi: false,
                aoi_format: None,
            },
            inputs_raw: serde_json::json!({}),
            parameters_raw: serde_json::json!({}),
            outputs_raw: serde_json::json!({}),
            enabled: true,
            source: "builtin".to_string(),
            health_status: "unknown".to_string(),
        };
        registry.register(bundle);

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

        let res = set_plugin_status_internal("test-plugin", false, &app_state).await;
        assert!(res.is_ok());

        // In-memory check
        let reg = app_state.plugins.read().await;
        assert!(!reg.get_details("test-plugin").unwrap().enabled);
        drop(reg);

        // On-disk check
        assert!(
            state_path.exists(),
            "State file must exist after set_plugin_status"
        );
        let store = load_plugin_state(&state_path).unwrap();
        assert!(!store.plugins.get("test-plugin").unwrap().enabled);

        // Re-enable
        let res2 = set_plugin_status_internal("test-plugin", true, &app_state).await;
        assert!(res2.is_ok());

        let reg2 = app_state.plugins.read().await;
        assert!(reg2.get_details("test-plugin").unwrap().enabled);
        drop(reg2);

        let store2 = load_plugin_state(&state_path).unwrap();
        assert!(store2.plugins.get("test-plugin").unwrap().enabled);
    }

    #[tokio::test]
    async fn test_set_plugin_status_missing_plugin_returns_not_found() {
        let app_state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::registry::PluginRegistry::new(),
            )),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: None,
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(std::path::PathBuf::from("/tmp")),
        };

        let res = set_plugin_status_internal("missing-plugin", false, &app_state).await;
        assert!(matches!(res, Err(PluginError::NotFound(_))));
    }

    fn create_test_app_state(temp_dir: &Path) -> crate::AppState {
        let state_path = temp_dir.join("plugin_state.json");
        crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::registry::PluginRegistry::new(),
            )),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: Some(state_path),
            db: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open_in_memory().unwrap(),
            )),
            library_root: std::sync::Arc::new(temp_dir.join("library")),
        }
    }

    fn create_valid_zip_archive(zip_path: &Path, plugin_id: &str) {
        let file = std::fs::File::create(zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);

        let manifest = format!(
            r#"{{
            "manifest_version": 1,
            "metadata": {{
                "id": "{plugin_id}",
                "name": "Test User Plugin",
                "version": "1.0.0",
                "description": "User test plugin",
                "category": "vegetation",
                "tags": ["user", "test"]
            }},
            "runtime": {{
                "type": "python",
                "entrypoint": "main.py"
            }},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }}"#
        );

        let parameters = r#"{
            "parameters": [
                {
                    "name": "threshold",
                    "label": "Threshold",
                    "type": "number",
                    "default": 0.5,
                    "min": 0.0,
                    "max": 1.0
                },
                {
                    "name": "mode",
                    "label": "Mode",
                    "type": "select",
                    "options": ["FAST", "ACCURATE"],
                    "default": "FAST"
                }
            ]
        }"#;

        let inputs = r#"{
            "granularity": "single_image",
            "supported_mime_types": ["image/jpeg"]
        }"#;

        let outputs = r#"{
            "metrics": {},
            "artifacts": {}
        }"#;

        writer
            .start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(manifest.as_bytes()).unwrap();

        writer
            .start_file("parameters.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(parameters.as_bytes()).unwrap();

        writer
            .start_file("inputs.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(inputs.as_bytes()).unwrap();

        writer
            .start_file("outputs.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(outputs.as_bytes()).unwrap();

        writer
            .start_file("main.py", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"# entrypoint\nprint('hello')\n").unwrap();

        writer.finish().unwrap();
    }

    #[tokio::test]
    async fn test_install_plugin_success() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let zip_path = temp_dir.path().join("user-plugin.zip");
        create_valid_zip_archive(&zip_path, "user-test-plugin");

        let summary = install_plugin_internal(&zip_path, &app_state)
            .await
            .expect("installation should succeed");
        assert_eq!(summary.id, "user-test-plugin");
        assert_eq!(summary.source, "user");
        assert!(summary.enabled);

        // Registry verification
        let registry = app_state.plugins.read().await;
        let details = registry.get_details("user-test-plugin").unwrap();
        assert!(details.enabled);
        drop(registry);

        // Filesystem verification
        let target_dir = temp_dir.path().join("user-test-plugin");
        assert!(target_dir.join("manifest.json").exists());
        assert!(target_dir.join("main.py").exists());

        // State persistence verification
        let state_path = temp_dir.path().join("plugin_state.json");
        let store = load_plugin_state(&state_path).unwrap();
        let rec = store.plugins.get("user-test-plugin").unwrap();
        assert_eq!(rec.source, PluginSource::User);
        assert!(rec.enabled);
    }

    #[tokio::test]
    async fn test_install_plugin_zip_slip_rejected() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let zip_path = temp_dir.path().join("malicious.zip");

        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            writer
                .start_file("../evil.sh", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"malicious").unwrap();
            writer.finish().unwrap();
        }

        let res = install_plugin_internal(&zip_path, &app_state).await;
        assert!(res.is_err());
        match res.unwrap_err() {
            PluginError::SecurityViolation(msg) => {
                assert!(msg.contains("Zip-Slip") || msg.contains("escapes"));
            }
            other => panic!("Expected SecurityViolation, got {:?}", other),
        }

        // Verify zero disk remnants
        assert!(!temp_dir.path().join("evil.sh").exists());
    }

    #[tokio::test]
    async fn test_install_plugin_cannot_overwrite_builtin() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());

        // Pre-register builtin plugin
        {
            let mut reg = app_state.plugins.write().await;
            let bundle = crate::plugin_manager::models::PluginBundle {
                install_path: temp_dir.path().join("builtin-plugin"),
                manifest: crate::plugin_manager::models::PluginManifest {
                    schema: None,
                    manifest_version: 1,
                    metadata: crate::plugin_manager::models::PluginMetadata {
                        id: "builtin-plugin".to_string(),
                        name: "Builtin".to_string(),
                        version: "1.0.0".to_string(),
                        author: None,
                        license: None,
                        description: "Builtin".to_string(),
                        category: Some("vegetation".to_string()),
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
                },
                inputs_spec: crate::plugin_manager::models::InputsSpec {
                    granularity: "single_image".to_string(),
                    supported_mime_types: vec!["image/jpeg".to_string()],
                    requires_metadata: vec![],
                    optional_metadata: vec![],
                    supports_aoi: false,
                    aoi_format: None,
                },
                inputs_raw: serde_json::json!({}),
                parameters_raw: serde_json::json!({}),
                outputs_raw: serde_json::json!({}),
                enabled: true,
                source: "builtin".to_string(),
                health_status: "unknown".to_string(),
            };
            reg.register(bundle);
        }

        let zip_path = temp_dir.path().join("overwrite.zip");
        create_valid_zip_archive(&zip_path, "builtin-plugin");

        let res = install_plugin_internal(&zip_path, &app_state).await;
        assert!(matches!(res, Err(PluginError::SecurityViolation(_))));
    }

    #[tokio::test]
    async fn test_install_plugin_corrupt_archive_no_remnants() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let bad_zip = temp_dir.path().join("corrupt.zip");
        std::fs::write(&bad_zip, b"this is not a zip file").unwrap();

        let res = install_plugin_internal(&bad_zip, &app_state).await;
        assert!(matches!(res, Err(PluginError::ArchiveError(_))));
    }

    #[tokio::test]
    async fn test_remove_user_plugin_success() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let zip_path = temp_dir.path().join("plugin.zip");
        create_valid_zip_archive(&zip_path, "removable-plugin");

        // Install first
        let _ = install_plugin_internal(&zip_path, &app_state)
            .await
            .unwrap();
        let target_dir = temp_dir.path().join("removable-plugin");
        assert!(target_dir.exists());

        // Remove
        let remove_res = remove_plugin_internal("removable-plugin", &app_state).await;
        assert!(remove_res.is_ok());

        // Assert directory deleted
        assert!(!target_dir.exists());

        // Assert removed from registry
        let reg = app_state.plugins.read().await;
        assert!(!reg.plugins.contains_key("removable-plugin"));
        drop(reg);

        // Assert purged from state store
        let state_path = temp_dir.path().join("plugin_state.json");
        let store = load_plugin_state(&state_path).unwrap();
        assert!(!store.plugins.contains_key("removable-plugin"));
    }

    #[tokio::test]
    async fn test_remove_builtin_plugin_rejected() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());

        // Register builtin
        {
            let mut reg = app_state.plugins.write().await;
            let bundle = crate::plugin_manager::models::PluginBundle {
                install_path: temp_dir.path().join("core-builtin"),
                manifest: crate::plugin_manager::models::PluginManifest {
                    schema: None,
                    manifest_version: 1,
                    metadata: crate::plugin_manager::models::PluginMetadata {
                        id: "core-builtin".to_string(),
                        name: "Core Builtin".to_string(),
                        version: "1.0.0".to_string(),
                        author: None,
                        license: None,
                        description: "Core Builtin".to_string(),
                        category: Some("vegetation".to_string()),
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
                },
                inputs_spec: crate::plugin_manager::models::InputsSpec {
                    granularity: "single_image".to_string(),
                    supported_mime_types: vec!["image/jpeg".to_string()],
                    requires_metadata: vec![],
                    optional_metadata: vec![],
                    supports_aoi: false,
                    aoi_format: None,
                },
                inputs_raw: serde_json::json!({}),
                parameters_raw: serde_json::json!({}),
                outputs_raw: serde_json::json!({}),
                enabled: true,
                source: "builtin".to_string(),
                health_status: "unknown".to_string(),
            };
            reg.register(bundle);
        }

        let res = remove_plugin_internal("core-builtin", &app_state).await;
        assert!(matches!(res, Err(PluginError::SecurityViolation(_))));

        // Verify still in registry
        let reg = app_state.plugins.read().await;
        assert!(reg.plugins.contains_key("core-builtin"));
    }

    #[tokio::test]
    async fn test_remove_plugin_running_job_rejected() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());

        // Register user plugin
        {
            let mut reg = app_state.plugins.write().await;
            let bundle = crate::plugin_manager::models::PluginBundle {
                install_path: temp_dir.path().join("busy-user-plugin"),
                manifest: crate::plugin_manager::models::PluginManifest {
                    schema: None,
                    manifest_version: 1,
                    metadata: crate::plugin_manager::models::PluginMetadata {
                        id: "busy-user-plugin".to_string(),
                        name: "Busy".to_string(),
                        version: "1.0.0".to_string(),
                        author: None,
                        license: None,
                        description: "Busy".to_string(),
                        category: Some("vegetation".to_string()),
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
                },
                inputs_spec: crate::plugin_manager::models::InputsSpec {
                    granularity: "single_image".to_string(),
                    supported_mime_types: vec!["image/jpeg".to_string()],
                    requires_metadata: vec![],
                    optional_metadata: vec![],
                    supports_aoi: false,
                    aoi_format: None,
                },
                inputs_raw: serde_json::json!({}),
                parameters_raw: serde_json::json!({}),
                outputs_raw: serde_json::json!({}),
                enabled: true,
                source: "user".to_string(),
                health_status: "unknown".to_string(),
            };
            reg.register(bundle);
        }

        // Add running job to active tracker
        {
            let mut tracker = app_state.jobs.write().await;
            tracker.jobs.insert(
                "job-123".to_string(),
                crate::plugin_manager::executor::JobRecord {
                    job_id: "job-123".to_string(),
                    plugin_id: "busy-user-plugin".to_string(),
                    status: crate::plugin_manager::executor::JobStatusDto::Running {
                        progress_pct: 50,
                        stage: "processing".to_string(),
                    },
                    result: None,
                    started_at: "2026-10-25T12:00:00Z".to_string(),
                    abort_handle: None,
                    child_pid: None,
                    output_dir: None,
                },
            );
        }

        let res = remove_plugin_internal("busy-user-plugin", &app_state).await;
        assert!(matches!(res, Err(PluginError::ExecutionError(_))));
    }

    #[tokio::test]
    async fn test_remove_nonexistent_plugin_returns_not_found() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let res = remove_plugin_internal("ghost-plugin", &app_state).await;
        assert!(matches!(res, Err(PluginError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_update_plugin_config_success() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());

        // Register plugin with parameters
        {
            let mut reg = app_state.plugins.write().await;
            let bundle = crate::plugin_manager::models::PluginBundle {
                install_path: temp_dir.path().join("config-plugin"),
                manifest: crate::plugin_manager::models::PluginManifest {
                    schema: None,
                    manifest_version: 1,
                    metadata: crate::plugin_manager::models::PluginMetadata {
                        id: "config-plugin".to_string(),
                        name: "Config Plugin".to_string(),
                        version: "1.0.0".to_string(),
                        author: None,
                        license: None,
                        description: "Config Plugin".to_string(),
                        category: Some("vegetation".to_string()),
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
                },
                inputs_spec: crate::plugin_manager::models::InputsSpec {
                    granularity: "single_image".to_string(),
                    supported_mime_types: vec!["image/jpeg".to_string()],
                    requires_metadata: vec![],
                    optional_metadata: vec![],
                    supports_aoi: false,
                    aoi_format: None,
                },
                inputs_raw: serde_json::json!({}),
                parameters_raw: serde_json::json!({
                    "parameters": [
                        {
                            "name": "threshold",
                            "label": "Threshold",
                            "type": "number",
                            "default": 0.5,
                            "min": 0.0,
                            "max": 1.0
                        },
                        {
                            "name": "mode",
                            "label": "Mode",
                            "type": "select",
                            "options": ["FAST", "ACCURATE"],
                            "default": "FAST"
                        }
                    ]
                }),
                outputs_raw: serde_json::json!({}),
                enabled: true,
                source: "user".to_string(),
                health_status: "unknown".to_string(),
            };
            reg.register(bundle);
        }

        let custom_params = serde_json::json!({
            "threshold": 0.85,
            "mode": "ACCURATE"
        });

        let res =
            update_plugin_config_internal("config-plugin", custom_params.clone(), &app_state).await;
        assert!(res.is_ok());

        // Check state file on disk
        let state_path = temp_dir.path().join("plugin_state.json");
        let store = load_plugin_state(&state_path).unwrap();
        let rec = store.plugins.get("config-plugin").unwrap();
        assert_eq!(rec.custom_parameters["threshold"], 0.85);
        assert_eq!(rec.custom_parameters["mode"], "ACCURATE");
    }

    #[tokio::test]
    async fn test_update_plugin_config_missing_plugin_returns_not_found() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let res = update_plugin_config_internal(
            "missing-plugin",
            serde_json::json!({"foo": "bar"}),
            &app_state,
        )
        .await;
        assert!(matches!(res, Err(PluginError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_update_plugin_config_invalid_parameter_rejected() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());

        // Register plugin
        {
            let mut reg = app_state.plugins.write().await;
            let bundle = crate::plugin_manager::models::PluginBundle {
                install_path: temp_dir.path().join("strict-plugin"),
                manifest: crate::plugin_manager::models::PluginManifest {
                    schema: None,
                    manifest_version: 1,
                    metadata: crate::plugin_manager::models::PluginMetadata {
                        id: "strict-plugin".to_string(),
                        name: "Strict".to_string(),
                        version: "1.0.0".to_string(),
                        author: None,
                        license: None,
                        description: "Strict".to_string(),
                        category: Some("vegetation".to_string()),
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
                },
                inputs_spec: crate::plugin_manager::models::InputsSpec {
                    granularity: "single_image".to_string(),
                    supported_mime_types: vec!["image/jpeg".to_string()],
                    requires_metadata: vec![],
                    optional_metadata: vec![],
                    supports_aoi: false,
                    aoi_format: None,
                },
                inputs_raw: serde_json::json!({}),
                parameters_raw: serde_json::json!({
                    "parameters": [
                        {
                            "name": "threshold",
                            "label": "Threshold",
                            "type": "number",
                            "default": 0.5,
                            "min": 0.0,
                            "max": 1.0
                        },
                        {
                            "name": "mode",
                            "label": "Mode",
                            "type": "select",
                            "options": ["FAST", "ACCURATE"],
                            "default": "FAST"
                        }
                    ]
                }),
                outputs_raw: serde_json::json!({}),
                enabled: true,
                source: "user".to_string(),
                health_status: "unknown".to_string(),
            };
            reg.register(bundle);
        }

        // 1. Unknown param
        let res1 = update_plugin_config_internal(
            "strict-plugin",
            serde_json::json!({"unknown_param": 10}),
            &app_state,
        )
        .await;
        assert!(matches!(res1, Err(PluginError::SubcontractInvalid { .. })));

        // 2. Number exceeds max
        let res2 = update_plugin_config_internal(
            "strict-plugin",
            serde_json::json!({"threshold": 1.5}),
            &app_state,
        )
        .await;
        assert!(matches!(res2, Err(PluginError::SubcontractInvalid { .. })));

        // 3. Number below min
        let res3 = update_plugin_config_internal(
            "strict-plugin",
            serde_json::json!({"threshold": -0.1}),
            &app_state,
        )
        .await;
        assert!(matches!(res3, Err(PluginError::SubcontractInvalid { .. })));

        // 4. Invalid select option
        let res4 = update_plugin_config_internal(
            "strict-plugin",
            serde_json::json!({"mode": "ULTRA_FAST"}),
            &app_state,
        )
        .await;
        assert!(matches!(res4, Err(PluginError::SubcontractInvalid { .. })));

        // 5. Wrong type (string for number)
        let res5 = update_plugin_config_internal(
            "strict-plugin",
            serde_json::json!({"threshold": "not_a_number"}),
            &app_state,
        )
        .await;
        assert!(matches!(res5, Err(PluginError::SubcontractInvalid { .. })));
    }

    #[tokio::test]
    async fn test_install_plugin_cannot_overwrite_running_job() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let zip_path = temp_dir.path().join("busy.zip");
        create_valid_zip_archive(&zip_path, "busy-user-plugin");

        // Install first
        install_plugin_internal(&zip_path, &app_state)
            .await
            .unwrap();

        // Mark running job
        {
            let mut tracker = app_state.jobs.write().await;
            tracker.jobs.insert(
                "job-123".to_string(),
                crate::plugin_manager::executor::JobRecord {
                    job_id: "job-123".to_string(),
                    plugin_id: "busy-user-plugin".to_string(),
                    status: crate::plugin_manager::executor::JobStatusDto::Running {
                        progress_pct: 50,
                        stage: "processing".to_string(),
                    },
                    result: None,
                    started_at: "2026-10-25T12:00:00Z".to_string(),
                    abort_handle: None,
                    child_pid: None,
                    output_dir: None,
                },
            );
        }

        // Attempt reinstall while running
        let res = install_plugin_internal(&zip_path, &app_state).await;
        assert!(
            matches!(res, Err(PluginError::ExecutionError(_))),
            "Expected ExecutionError when overwriting active running job, got {:?}",
            res
        );
    }

    #[test]
    fn test_verify_and_extract_entry_rejects_symlinks() {
        let mut buffer = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
            writer
                .add_symlink(
                    "malicious_symlink",
                    "/etc/passwd",
                    SimpleFileOptions::default(),
                )
                .unwrap();
            writer.finish().unwrap();
        }
        let dir = tempdir().unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
        let mut entry = archive.by_index(0).unwrap();
        let res = verify_and_extract_entry(&mut entry, dir.path());
        assert!(res.is_err(), "Symlinks in zip must be rejected");
        assert!(matches!(
            res.unwrap_err(),
            PluginError::SecurityViolation(_)
        ));
    }

    #[test]
    #[cfg(unix)]
    fn test_verify_and_extract_entry_preserves_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let mut buffer = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
            let options = SimpleFileOptions::default().unix_permissions(0o755);
            writer.start_file("bin/run.sh", options).unwrap();
            writer.write_all(b"#!/bin/sh\necho ok\n").unwrap();
            writer.finish().unwrap();
        }
        let dir = tempdir().unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
        let mut entry = archive.by_index(0).unwrap();
        let extracted = verify_and_extract_entry(&mut entry, dir.path()).unwrap();
        let meta = std::fs::metadata(&extracted).unwrap();
        assert_eq!(
            meta.permissions().mode() & 0o111,
            0o111,
            "Executable bits must be preserved"
        );
    }

    #[tokio::test]
    async fn test_remove_plugin_invalid_id_path_traversal_rejected() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let res = remove_plugin_internal("../../etc/passwd", &app_state).await;
        assert!(matches!(res, Err(PluginError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_install_plugin_ambiguous_multiple_manifests_rejected() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());
        let zip_path = temp_dir.path().join("ambiguous.zip");

        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);

            writer
                .start_file("plugin-a/manifest.json", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{\"manifest_version\": 1}").unwrap();

            writer
                .start_file("plugin-b/manifest.json", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{\"manifest_version\": 1}").unwrap();

            writer.finish().unwrap();
        }

        let res = install_plugin_internal(&zip_path, &app_state).await;
        assert!(matches!(res, Err(PluginError::ManifestInvalid { .. })));
    }

    #[tokio::test]
    async fn test_install_plugin_state_save_failure_rolls_back_filesystem_and_registry() {
        let temp_dir = tempdir().unwrap();
        let app_state = create_test_app_state(temp_dir.path());

        // Make state_path a directory so rename(&temp_file_path, state_path) fails with EISDIR
        let state_path = temp_dir.path().join("plugin_state.json");
        std::fs::create_dir_all(&state_path).unwrap();

        let zip_path = temp_dir.path().join("rollback.zip");
        create_valid_zip_archive(&zip_path, "rollback-plugin");

        let res = install_plugin_internal(&zip_path, &app_state).await;
        assert!(
            res.is_err(),
            "Installation must fail when state saving fails"
        );

        // Verify rollback: target directory must NOT exist on disk
        let target_dir = temp_dir.path().join("rollback-plugin");
        assert!(
            !target_dir.exists(),
            "Target directory must be deleted/rolled back on state save failure"
        );

        // Verify rollback: in-memory registry must NOT contain the plugin
        let reg = app_state.plugins.read().await;
        assert!(
            !reg.plugins.contains_key("rollback-plugin"),
            "Registry must not retain plugin on state save failure"
        );
    }
}
