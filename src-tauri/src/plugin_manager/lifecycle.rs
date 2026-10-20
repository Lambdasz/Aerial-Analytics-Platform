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
    _archive_path: String,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<PluginSummaryDto, CommandError> {
    todo!("install_plugin: safe zip extraction, schema validation, state registration")
}

/// Permanently uninstalls an existing user-installed plugin.
///
/// **Pre-condition**: `plugin_id` exists; source is `user` (not `builtin`); plugin is not running.  
/// **Post-condition**: Plugin folder deleted; entry purged from `plugin_state.json`.
#[tauri::command]
pub async fn remove_plugin(
    _plugin_id: String,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    todo!("remove_plugin: delete plugin directory and remove state record")
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

    save_plugin_state(&state_path, &store)
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
    _plugin_id: String,
    _custom_params: serde_json::Value,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    todo!("update_plugin_config: validate and save custom parameter overrides")
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
    let enclosed = entry.enclosed_name().ok_or_else(|| {
        PluginError::SecurityViolation(format!("Zip-Slip attempt: {}", entry.name()))
    })?;

    let target_path = dest_dir.join(enclosed);

    if target_path.starts_with(dest_dir) {
        Ok(target_path)
    } else {
        Err(PluginError::SecurityViolation(format!(
            "Path escapes sandbox: {}",
            entry.name()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };

        let res = set_plugin_status_internal("missing-plugin", false, &app_state).await;
        assert!(matches!(res, Err(PluginError::NotFound(_))));
    }
}
