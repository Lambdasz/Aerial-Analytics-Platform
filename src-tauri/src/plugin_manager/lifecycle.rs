//! # Domain 2: Lifecycle & State Persistence
//!
//! **Role**: Plugin Lifecycle & Package Manager (Imperative Shell & Facade)
//! **Deliverable**: `plugin_manager/lifecycle.rs`
//!
//! Responsible for:
//! - M2.4: Safe ZIP archive installation with Zip-Slip path traversal defense
//! - M2.5: Enable/disable plugin status toggling
//! - Uninstallation and removal of user-installed plugins
//! - Atomic persistence of mutable state (`plugin_state.json`)
//!
//! ## Purity Boundary
//!
//! This module is the **Imperative Shell** and public/crate facade over
//! [`super::lifecycle_core`]. All filesystem I/O, ZIP archive extraction,
//! Tauri IPC command handling, `tokio::sync::RwLock` state synchronization,
//! `chrono::Utc::now()` clock reads, `uuid::Uuid::new_v4()` temporary naming,
//! and blocking validation dispatch live here. Pure domain logic (plugin ID
//! validation, staged root resolution, precondition checks, parallel parameter
//! validation, immutable state transitions, and removal planning) is delegated
//! to [`super::lifecycle_core`].
//!
//! Refer to: `src-tauri/src/plugin_manager/DOCS/02_DOMAIN_2_LIFECYCLE_STATE.md`

use std::path::{Path, PathBuf};

use super::error::{CommandError, PluginError};
use super::lifecycle_core::{
    assemble_user_plugin_bundle, plan_plugin_removal, resolve_staged_plugin_root,
    validate_custom_parameters, validate_installation_preconditions, with_applied_state,
    with_config_update, with_install_update, with_status_update, ConfigUpdateInput,
    RemovalPlanInput,
};
use super::models::{PluginSource, PluginStateStore, PluginSummaryDto};

// ---------------------------------------------------------------------------
// 1. TAURI IPC COMMANDS (pub — registered in lib.rs invoke_handler)
// ---------------------------------------------------------------------------

/// Installs a new plugin from a user-provided `.zip` archive file.
///
/// **Pre-condition**: `archive_path` points to a valid `.zip` file on disk.  
/// **Post-condition**: Safely extracted into `plugins/<id>/`, validated, and registered in state.
///
/// # Purity
/// Impure Tauri IPC command handler; delegates archive extraction, validation,
/// persistence, and registry registration to `install_plugin_internal`.
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
///
/// # Purity
/// Impure Tauri IPC command handler; delegates removal planning, disk cleanup,
/// persistence, and registry mutation to `remove_plugin_internal`.
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
/// * `plugin_id` - Unique identifier string of the target plugin to toggle.
/// * `enabled` - Target status boolean (`true` to enable, `false` to disable).
/// * `state` - Injected Tauri application state containing plugin registry and state stores.
///
/// # Pre-condition
/// - `plugin_id` is registered in memory, and the OS AppData state directory is accessible for atomic file persistence.
///
/// # Post-condition
/// - Updates the in-memory `enabled` flag of the specified plugin and atomically flushes the updated `PluginStateStore` to `plugin_state.json` via a temporary file write and atomic rename.
///
/// # Errors
/// - Returns `CommandError` with error code `PLUGIN_NOT_FOUND` if `plugin_id` is not found in the registry or state, or `STATE_ERROR` if serializing and atomically persisting to `plugin_state.json` fails.
///
/// # Panics
/// - This function does not panic.
///
/// # Purity
/// Impure Tauri IPC command handler; delegates state persistence and registry
/// status synchronization to `set_plugin_status_internal`.
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

/// Updates and persists custom parameter overrides configured by the user.
///
/// **Pre-condition**: `plugin_id` exists; parameters conform to plugin's `parameters.json`.  
/// **Post-condition**: Overrides saved in state and persisted to disk.
///
/// # Purity
/// Impure Tauri IPC command handler; delegates off-thread parameter validation,
/// persistence, and registry mutation to `update_plugin_config_internal`.
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

// ---------------------------------------------------------------------------
// 2. IMPERATIVE SHELL WORKFLOWS & REGISTRY COMPATIBILITY
// ---------------------------------------------------------------------------

/// Internal helper for toggling plugin status and flushing changes to `plugin_state.json`.
///
/// Reads a short `(version, source)` snapshot under `plugins.read().await`, computes
/// the next state via [`with_status_update`], persists to disk first, and only then
/// mutates the in-memory registry under `plugins.write().await` so persistence
/// failures require no in-memory rollback.
///
/// # Purity
/// Imperative shell: performs async `RwLock` synchronization, clock reads, and
/// atomic disk persistence around pure [`with_status_update`].
pub(crate) async fn set_plugin_status_internal(
    plugin_id: &str,
    enabled: bool,
    app_state: &crate::AppState,
) -> Result<(), PluginError> {
    let (version, source) = {
        let registry = app_state.plugins.read().await;
        let bundle = registry
            .plugins
            .get(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        (
            bundle.manifest.metadata.version.clone(),
            plugin_source_from_label(&bundle.source),
        )
    };

    let state_path = state_path_or_default(app_state);
    let store = load_plugin_state(&state_path).unwrap_or_default();
    let now_iso = chrono::Utc::now().to_rfc3339();
    let next_store = with_status_update(&store, plugin_id, enabled, &version, source, &now_iso);

    save_plugin_state(&state_path, &next_store)?;

    let mut registry = app_state.plugins.write().await;
    registry.set_status(plugin_id, enabled)?;

    Ok(())
}

/// Compatibility wrapper delegating to [`with_applied_state`].
///
/// # Purity
/// Impure in-place mutation facade over pure [`with_applied_state`].
pub(crate) fn apply_state_to_registry(
    registry: &mut super::registry::PluginRegistry,
    store: &PluginStateStore,
) {
    *registry = with_applied_state(registry, store);
}

/// Fallback default path for `plugin_state.json` if none is specified in `AppState`.
///
/// # Purity
/// Impure filesystem probe (`Path::is_dir`) over candidate plugin directories.
pub(crate) fn default_state_path() -> PathBuf {
    let candidate_dirs = [
        PathBuf::from("plugins"),
        PathBuf::from("../plugins"),
        PathBuf::from("../../plugins"),
    ];

    candidate_dirs
        .iter()
        .find(|dir| dir.is_dir())
        .map(|dir| dir.join("plugin_state.json"))
        .unwrap_or_else(|| PathBuf::from("plugins").join("plugin_state.json"))
}

/// Resolves the configured `plugin_state.json` path or falls back to [`default_state_path`].
///
/// # Purity
/// Impure when `app_state.state_path` is `None` (delegates to [`default_state_path`]).
fn state_path_or_default(app_state: &crate::AppState) -> PathBuf {
    app_state
        .state_path
        .clone()
        .unwrap_or_else(default_state_path)
}

/// Derives the plugins root directory from a `plugin_state.json` path.
///
/// # Purity
/// Pure deterministic path projection over `&Path`.
fn plugins_dir_from_state_path(state_path: &Path) -> PathBuf {
    match state_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("plugins"),
    }
}

/// Maps a bundle source label (`"user"` vs `"builtin"`) to [`PluginSource`].
///
/// # Purity
/// Pure deterministic mapping over `&str`.
fn plugin_source_from_label(source: &str) -> PluginSource {
    if source == "user" {
        PluginSource::User
    } else {
        PluginSource::Builtin
    }
}

/// Checks whether `plugin_id` currently has a queued or running job in `app_state.jobs`.
///
/// # Purity
/// Imperative shell: acquires async read lock on `app_state.jobs`.
async fn is_plugin_job_active(app_state: &crate::AppState, plugin_id: &str) -> bool {
    let tracker = app_state.jobs.read().await;
    tracker.jobs.values().any(|job| {
        job.plugin_id == plugin_id
            && matches!(
                job.status,
                super::executor::JobStatusDto::Queued
                    | super::executor::JobStatusDto::Running { .. }
            )
    })
}

/// Internal helper for installing a plugin from a `.zip` archive.
///
/// Performs archive extraction, staging scan, manifest/subcontract validation,
/// filesystem commit, and `plugin_state.json` persistence before taking a short
/// write lock to register the assembled [`PluginBundle`](super::models::PluginBundle) in memory. If state
/// persistence fails, removes `target_dir` before returning the error.
///
/// # Purity
/// Imperative shell: performs ZIP/filesystem I/O, clock reads, async `RwLock`
/// synchronization, and error rollback around pure helpers in [`super::lifecycle_core`]
/// and [`super::registry`].
pub(crate) async fn install_plugin_internal(
    archive_path: &Path,
    app_state: &crate::AppState,
) -> Result<PluginSummaryDto, PluginError> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| PluginError::ArchiveError(format!("Cannot open archive file: {}", e)))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| PluginError::ArchiveError(format!("Invalid zip archive: {}", e)))?;

    let state_path = state_path_or_default(app_state);
    let plugins_dir = plugins_dir_from_state_path(&state_path);
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
    let has_root_manifest = staging_path.join("manifest.json").is_file();
    let candidates = if has_root_manifest {
        Vec::new()
    } else {
        std::fs::read_dir(staging_path)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| p.is_dir() && p.join("manifest.json").is_file())
            .collect()
    };
    let plugin_source_dir =
        resolve_staged_plugin_root(staging_path, has_root_manifest, candidates)?;

    // Step 3: Parse and validate manifest
    let manifest_str =
        std::fs::read_to_string(plugin_source_dir.join("manifest.json")).map_err(|e| {
            PluginError::ManifestInvalid {
                path: "manifest.json".to_string(),
                message: e.to_string(),
            }
        })?;
    let manifest = super::registry::parse_and_validate_manifest(&manifest_str)?;
    let plugin_id = manifest.metadata.id.clone();
    let plugin_version = manifest.metadata.version.clone();

    // Step 4: Verify metadata.id is valid, does not overwrite builtin, and is not currently executing a job
    let is_job_active = is_plugin_job_active(app_state, &plugin_id).await;

    let existing_reg_source = {
        let registry = app_state.plugins.read().await;
        registry.plugins.get(&plugin_id).map(|b| b.source.clone())
    };

    let store = load_plugin_state(&state_path).unwrap_or_default();
    let existing_state_source = store.plugins.get(&plugin_id).map(|r| r.source.clone());

    validate_installation_preconditions(
        &plugin_id,
        is_job_active,
        existing_reg_source.as_deref(),
        existing_state_source.as_ref(),
    )?;

    // Step 5: Validate sub-contracts (parameters.json, inputs.json, outputs.json)
    let (parameters_raw, inputs_spec, outputs_raw) =
        super::registry::validate_subcontracts(&plugin_source_dir, &manifest)?;

    let inputs_path = plugin_source_dir.join(&manifest.inputs);
    let inputs_raw = std::fs::read_to_string(&inputs_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::to_value(&inputs_spec).unwrap_or_default());

    // Step 6: Atomic commit to plugins/<plugin_id>
    let target_dir = plugins_dir.join(&plugin_id);
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

    // Step 7: Assemble bundle and persist plugin_state.json before registering in memory
    let bundle = assemble_user_plugin_bundle(
        target_dir.clone(),
        manifest,
        inputs_spec,
        inputs_raw,
        parameters_raw,
        outputs_raw,
    );
    let summary = bundle.to_summary();

    let now_iso = chrono::Utc::now().to_rfc3339();
    let next_store = with_install_update(&store, &plugin_id, &plugin_version, &now_iso);

    if let Err(err) = save_plugin_state(&state_path, &next_store) {
        // Rollback filesystem commit on persistence failure; registry was not mutated yet
        let _ = std::fs::remove_dir_all(&target_dir);
        return Err(err);
    }

    // Step 8: Short write lock to register the bundle only after state persistence succeeds
    let mut registry = app_state.plugins.write().await;
    registry.register(bundle);

    Ok(summary)
}

/// Internal helper for removing a user plugin and purging its state.
///
/// Snapshots active-job status and builds a [`RemovalPlan`](super::lifecycle_core::RemovalPlan) under short read locks,
/// deletes the physical plugin directory and persists the updated state without
/// holding registry locks, then removes the entry under a short write lock.
///
/// # Purity
/// Imperative shell: performs async `RwLock` synchronization, clock reads,
/// filesystem directory deletion, and atomic state persistence around pure
/// [`plan_plugin_removal`].
pub(crate) async fn remove_plugin_internal(
    plugin_id: &str,
    app_state: &crate::AppState,
) -> Result<(), PluginError> {
    let is_job_active = is_plugin_job_active(app_state, plugin_id).await;

    let state_path = state_path_or_default(app_state);
    let store = load_plugin_state(&state_path).unwrap_or_default();
    let plugins_dir = plugins_dir_from_state_path(&state_path);
    let now_iso = chrono::Utc::now().to_rfc3339();

    let (has_bundle, plan) = {
        let registry = app_state.plugins.read().await;
        let has_bundle = registry.plugins.contains_key(plugin_id);
        let input = RemovalPlanInput {
            plugin_id,
            is_job_active,
            plugins_dir: &plugins_dir,
            now_iso: &now_iso,
        };
        let plan = plan_plugin_removal(&registry, &store, &input)?;
        (has_bundle, plan)
    };

    if plan.install_dir_to_remove.exists() {
        if has_bundle {
            std::fs::remove_dir_all(&plan.install_dir_to_remove).map_err(|e| {
                PluginError::Io(format!("Failed to delete plugin directory: {}", e))
            })?;
        } else {
            let _ = std::fs::remove_dir_all(&plan.install_dir_to_remove);
        }
    }

    save_plugin_state(&state_path, &plan.updated_store)?;

    let mut registry = app_state.plugins.write().await;
    registry.plugins.remove(plugin_id);

    Ok(())
}

/// Internal helper for validating and saving custom parameter overrides.
///
/// Snapshots plugin metadata under a short read lock, drops the lock, runs
/// [`validate_custom_parameters`] (`rayon::par_iter()`) inside
/// `tokio::task::spawn_blocking`, persists the new state computed by
/// [`with_config_update`], and finally updates the in-memory bundle under a
/// short write lock.
///
/// # Purity
/// Imperative shell: performs async `RwLock` synchronization, blocking worker
/// dispatch, clock reads, and atomic state persistence around pure
/// [`validate_custom_parameters`] and [`with_config_update`].
pub(crate) async fn update_plugin_config_internal(
    plugin_id: &str,
    custom_params: serde_json::Value,
    app_state: &crate::AppState,
) -> Result<(), PluginError> {
    let (version, source, enabled, parameters_raw) = {
        let registry = app_state.plugins.read().await;
        let bundle = registry
            .plugins
            .get(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        (
            bundle.manifest.metadata.version.clone(),
            plugin_source_from_label(&bundle.source),
            bundle.enabled,
            bundle.parameters_raw.clone(),
        )
    };

    let params_clone = custom_params.clone();
    tokio::task::spawn_blocking(move || validate_custom_parameters(&parameters_raw, &params_clone))
        .await
        .map_err(|e| {
            PluginError::StateError(format!("Parameter validation task failed: {}", e))
        })??;

    let state_path = state_path_or_default(app_state);
    let store = load_plugin_state(&state_path).unwrap_or_default();
    let now_iso = chrono::Utc::now().to_rfc3339();
    let input = ConfigUpdateInput {
        plugin_id,
        version: &version,
        source,
        enabled,
        custom_params: &custom_params,
    };
    let next_store = with_config_update(&store, &input, &now_iso);

    save_plugin_state(&state_path, &next_store)?;

    let mut registry = app_state.plugins.write().await;
    if let Some(bundle_mut) = registry.plugins.get_mut(plugin_id) {
        bundle_mut.custom_parameters = custom_params;
    }

    Ok(())
}

/// Moves a directory tree to `to`, falling back to recursive copy when rename fails across filesystems.
///
/// # Purity
/// Impure filesystem mutation helper.
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

/// Recursively copies files and subdirectories from `src` to `dst`.
///
/// # Purity
/// Impure filesystem I/O helper.
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
// 3. STATE PERSISTENCE & SECURITY HELPERS (pub(crate))
// ---------------------------------------------------------------------------

/// Reads and parses `plugin_state.json` from disk.
///
/// Returns empty default store if the state file does not yet exist.
///
/// # Purity
/// Impure filesystem read and JSON deserialization helper.
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
///
/// # Purity
/// Impure filesystem write, `fsync`, UUID generation, and atomic rename helper.
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
///
/// # Purity
/// Impure archive extraction and filesystem write/permission helper.
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
#[path = "lifecycle_tests.rs"]
mod tests;
