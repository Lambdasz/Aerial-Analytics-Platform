//! # Domain 2 (Pure Core): Lifecycle Validation, State Transitions & Planners
//!
//! **Role**: Pure functional core for plugin ID validation, staged archive
//! layout resolution, installation precondition checks, parameter override
//! validation, immutable `PluginStateStore`/`PluginRegistry` transitions, and
//! removal planning.
//!
//! ## Purity Boundary
//!
//! This module is strictly pure. It MUST NOT import or invoke any side-effectful
//! primitives (`std::fs`, `std::io`, `zip`, `tempfile`, `tokio`, `rusqlite`,
//! `chrono::Utc::now()`, `uuid::Uuid::new_v4()`, `crate::AppState`, or `eprintln!`).
//! All filesystem I/O, archive extraction, clock reads, and Tauri state
//! synchronization live in [`super::lifecycle`].

use std::path::{Path, PathBuf};

use super::error::PluginError;
use super::models::{
    InputsSpec, PluginBundle, PluginManifest, PluginSource, PluginStateRecord, PluginStateStore,
    PluginStats,
};
use super::registry_core::PluginRegistry;

// ---------------------------------------------------------------------------
// 1. PURE VALIDATION & ASSEMBLY FUNCTIONS
// ---------------------------------------------------------------------------

/// Pure predicate validating plugin identifier format against `^[a-z0-9-_]+$`.
///
/// # Purity
/// Pure deterministic predicate over `&str`; zero I/O, zero allocations, zero side effects.
pub(crate) fn is_valid_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Resolves the plugin root directory from staging inspection results.
///
/// `has_root_manifest` indicates whether `manifest.json` exists directly in
/// `staging_root`; `candidates` contains top-level subdirectories containing
/// `manifest.json`.
///
/// # Purity
/// Pure deterministic path selection; performs no filesystem I/O or mutation.
pub(crate) fn resolve_staged_plugin_root(
    staging_root: &Path,
    has_root_manifest: bool,
    candidates: Vec<PathBuf>,
) -> Result<PathBuf, PluginError> {
    if has_root_manifest {
        return Ok(staging_root.to_path_buf());
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
    Ok(candidates
        .into_iter()
        .next()
        .expect("candidates verified to have length 1"))
}

/// Validates whether a plugin can be installed based on ID format, active job
/// status, and existing source in the registry or state store.
///
/// # Purity
/// Pure deterministic validation over borrowed arguments; zero I/O, zero side effects.
pub(crate) fn validate_installation_preconditions(
    plugin_id: &str,
    is_job_active: bool,
    existing_registry_source: Option<&str>,
    existing_state_source: Option<&PluginSource>,
) -> Result<(), PluginError> {
    if !is_valid_plugin_id(plugin_id) {
        return Err(PluginError::SecurityViolation(format!(
            "Invalid plugin id '{}': must match ^[a-z0-9-_]+$",
            plugin_id
        )));
    }

    if is_job_active {
        return Err(PluginError::ExecutionError(format!(
            "Cannot overwrite plugin '{}' because it is currently executing a job",
            plugin_id
        )));
    }

    if let Some("builtin") = existing_registry_source {
        return Err(PluginError::SecurityViolation(format!(
            "Cannot overwrite builtin plugin '{}'",
            plugin_id
        )));
    }

    if let Some(PluginSource::Builtin) = existing_state_source {
        return Err(PluginError::SecurityViolation(format!(
            "Cannot overwrite builtin plugin '{}'",
            plugin_id
        )));
    }

    Ok(())
}

/// Assembles a user-installed [`PluginBundle`] from validated components.
///
/// # Purity
/// Pure deterministic constructor; zero I/O, zero side effects.
pub(crate) fn assemble_user_plugin_bundle(
    target_dir: PathBuf,
    manifest: PluginManifest,
    inputs_spec: InputsSpec,
    inputs_raw: serde_json::Value,
    parameters_raw: serde_json::Value,
    outputs_raw: serde_json::Value,
) -> PluginBundle {
    PluginBundle {
        install_path: target_dir,
        manifest,
        inputs_spec,
        inputs_raw,
        parameters_raw,
        outputs_raw,
        enabled: true,
        source: "user".to_string(),
        health_status: "unknown".to_string(),
        custom_parameters: serde_json::json!({}),
    }
}

/// Validates custom parameter overrides against constraints declared in `parameters.json`.
///
/// Evaluates each `(key, value)` override lazily via iterator combinators
/// without allocating intermediate collections or mutating inputs.
///
/// # Purity
/// Pure sequential validation over immutable JSON references; zero I/O, zero mutation.
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

    let defined_params = parameters_raw
        .get("parameters")
        .and_then(|p| p.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    custom_obj.iter().try_for_each(|(key, val)| {
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
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// 2. IMMUTABLE STATE TRANSITIONS & PLANNERS
// ---------------------------------------------------------------------------

/// Returns a new [`PluginStateStore`] with the plugin's enabled status updated.
///
/// Inserts a new [`PluginStateRecord`] when `plugin_id` is absent, or updates
/// `enabled` on the existing record while preserving `installed_at`, `version`,
/// `source`, `custom_parameters`, and `stats`.
///
/// # Purity
/// Pure immutable state transition; clones `&PluginStateStore` without mutating the
/// caller's value, performs no I/O, and reads no system clock (`now_iso` is explicit).
pub(crate) fn with_status_update(
    store: &PluginStateStore,
    plugin_id: &str,
    enabled: bool,
    version: &str,
    source: PluginSource,
    now_iso: &str,
) -> PluginStateStore {
    let mut next_store = store.clone();
    next_store.updated_at = now_iso.to_string();
    let record = next_store
        .plugins
        .entry(plugin_id.to_string())
        .or_insert_with(|| PluginStateRecord {
            version: version.to_string(),
            source,
            enabled,
            installed_at: now_iso.to_string(),
            last_healthcheck: None,
            custom_parameters: serde_json::json!({}),
            stats: PluginStats::default(),
        });
    record.enabled = enabled;
    next_store
}

/// Returns a new [`PluginStateStore`] recording a user plugin installation or reinstall.
///
/// Inserts a new [`PluginStateRecord`] when `plugin_id` is absent, or updates
/// `version`, `source = User`, and `enabled = true` while preserving existing
/// `installed_at`, `last_healthcheck`, `custom_parameters`, and `stats`.
///
/// # Purity
/// Pure immutable state transition; clones `&PluginStateStore` without mutating the
/// caller's value, performs no I/O, and reads no system clock (`now_iso` is explicit).
pub(crate) fn with_install_update(
    store: &PluginStateStore,
    plugin_id: &str,
    version: &str,
    now_iso: &str,
) -> PluginStateStore {
    let mut next_store = store.clone();
    next_store.updated_at = now_iso.to_string();
    let record = next_store
        .plugins
        .entry(plugin_id.to_string())
        .or_insert_with(|| PluginStateRecord {
            version: version.to_string(),
            source: PluginSource::User,
            enabled: true,
            installed_at: now_iso.to_string(),
            last_healthcheck: None,
            custom_parameters: serde_json::json!({}),
            stats: PluginStats::default(),
        });
    record.version = version.to_string();
    record.source = PluginSource::User;
    record.enabled = true;
    next_store
}

/// Borrowed input fields for [`with_config_update`].
#[derive(Debug, Clone)]
pub(crate) struct ConfigUpdateInput<'a> {
    pub plugin_id: &'a str,
    pub version: &'a str,
    pub source: PluginSource,
    pub enabled: bool,
    pub custom_params: &'a serde_json::Value,
}

/// Returns a new [`PluginStateStore`] with updated `custom_parameters` for a plugin.
///
/// Inserts a new [`PluginStateRecord`] when `plugin_id` is absent, or updates
/// `custom_parameters` on the existing record while preserving its remaining fields.
///
/// # Purity
/// Pure immutable state transition; clones `&PluginStateStore` without mutating the
/// caller's value, performs no I/O, and reads no system clock (`now_iso` is explicit).
pub(crate) fn with_config_update(
    store: &PluginStateStore,
    input: &ConfigUpdateInput<'_>,
    now_iso: &str,
) -> PluginStateStore {
    let mut next_store = store.clone();
    next_store.updated_at = now_iso.to_string();
    let record = next_store
        .plugins
        .entry(input.plugin_id.to_string())
        .or_insert_with(|| PluginStateRecord {
            version: input.version.to_string(),
            source: input.source.clone(),
            enabled: input.enabled,
            installed_at: now_iso.to_string(),
            last_healthcheck: None,
            custom_parameters: serde_json::json!({}),
            stats: PluginStats::default(),
        });
    record.custom_parameters = input.custom_params.clone();
    next_store
}

/// Returns a new [`PluginStateStore`] with `plugin_id` removed and `updated_at` set.
///
/// # Purity
/// Pure immutable state transition; clones `&PluginStateStore` without mutating the
/// caller's value, performs no I/O, and reads no system clock (`now_iso` is explicit).
pub(crate) fn with_removed_plugin(
    store: &PluginStateStore,
    plugin_id: &str,
    now_iso: &str,
) -> PluginStateStore {
    let mut next_store = store.clone();
    next_store.plugins.remove(plugin_id);
    next_store.updated_at = now_iso.to_string();
    next_store
}

/// Returns a new [`PluginRegistry`] with persisted state records applied to matching bundles.
///
/// Updates `enabled`, `source`, `health_status` (when `last_healthcheck` is present),
/// and `custom_parameters` for every plugin present in both `registry` and `store`.
///
/// # Purity
/// Pure immutable projection; clones `&PluginRegistry` without mutating the caller's
/// value and performs no I/O.
pub(crate) fn with_applied_state(
    registry: &PluginRegistry,
    store: &PluginStateStore,
) -> PluginRegistry {
    let mut next_registry = registry.clone();
    for (id, record) in &store.plugins {
        if let Some(bundle) = next_registry.plugins.get_mut(id) {
            bundle.enabled = record.enabled;
            bundle.source = match record.source {
                PluginSource::Builtin => "builtin".to_string(),
                PluginSource::User => "user".to_string(),
            };
            if let Some(ref hc) = record.last_healthcheck {
                bundle.health_status = hc.status.clone();
            }
            bundle.custom_parameters = record.custom_parameters.clone();
        }
    }
    next_registry
}

/// Borrowed input fields for [`plan_plugin_removal`].
#[derive(Debug, Clone)]
pub(crate) struct RemovalPlanInput<'a> {
    pub plugin_id: &'a str,
    pub is_job_active: bool,
    pub plugins_dir: &'a Path,
    pub now_iso: &'a str,
}

/// Pure execution plan produced by [`plan_plugin_removal`] for removing a user plugin.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RemovalPlan {
    pub install_dir_to_remove: PathBuf,
    pub updated_store: PluginStateStore,
}

/// Validates plugin removal preconditions and builds an immutable [`RemovalPlan`].
///
/// Checks ID format (`is_valid_plugin_id`), active job guard, presence in either
/// `registry` or `store`, builtin protection, resolves the installation directory
/// (`bundle.install_path` if registered, otherwise `plugins_dir.join(plugin_id)`),
/// and computes `with_removed_plugin(store, plugin_id, now_iso)`.
///
/// # Purity
/// Pure planner over immutable references; performs no filesystem deletion, state
/// mutation, or clock access.
pub(crate) fn plan_plugin_removal(
    registry: &PluginRegistry,
    store: &PluginStateStore,
    input: &RemovalPlanInput<'_>,
) -> Result<RemovalPlan, PluginError> {
    if !is_valid_plugin_id(input.plugin_id) {
        return Err(PluginError::NotFound(input.plugin_id.to_string()));
    }

    if input.is_job_active {
        return Err(PluginError::ExecutionError(format!(
            "Cannot remove plugin '{}' because it is currently executing a job",
            input.plugin_id
        )));
    }

    let bundle_opt = registry.plugins.get(input.plugin_id);
    let store_rec_opt = store.plugins.get(input.plugin_id);

    if bundle_opt.is_none() && store_rec_opt.is_none() {
        return Err(PluginError::NotFound(input.plugin_id.to_string()));
    }

    let is_builtin = bundle_opt
        .map(|b| b.source.as_str() == "builtin")
        .unwrap_or(false)
        || store_rec_opt
            .map(|r| r.source == PluginSource::Builtin)
            .unwrap_or(false);

    if is_builtin {
        return Err(PluginError::SecurityViolation(format!(
            "Cannot remove builtin plugin '{}'",
            input.plugin_id
        )));
    }

    let install_dir_to_remove = bundle_opt
        .map(|b| b.install_path.clone())
        .unwrap_or_else(|| input.plugins_dir.join(input.plugin_id));

    let updated_store = with_removed_plugin(store, input.plugin_id, input.now_iso);

    Ok(RemovalPlan {
        install_dir_to_remove,
        updated_store,
    })
}

// ---------------------------------------------------------------------------
// 3. PURE UNIT TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::{
        CachedHealthcheck, PluginMetadata, PluginRuntime, PluginStateRecord,
    };
    use std::collections::HashMap;

    fn empty_store() -> PluginStateStore {
        PluginStateStore {
            schema_version: 1,
            updated_at: "2026-11-01T00:00:00Z".to_string(),
            plugins: HashMap::new(),
        }
    }

    fn make_manifest(id: &str, version: &str) -> PluginManifest {
        PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: PluginMetadata {
                id: id.to_string(),
                name: format!("Plugin {id}"),
                version: version.to_string(),
                author: None,
                license: None,
                description: "Test plugin".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: PluginRuntime {
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
        }
    }

    fn make_inputs_spec() -> InputsSpec {
        InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string()],
            requires_metadata: vec![],
            optional_metadata: vec![],
            supports_aoi: false,
            aoi_format: None,
        }
    }

    fn make_bundle(id: &str, source: &str, install_path: PathBuf) -> PluginBundle {
        PluginBundle {
            install_path,
            manifest: make_manifest(id, "1.0.0"),
            inputs_spec: make_inputs_spec(),
            inputs_raw: serde_json::json!({}),
            parameters_raw: serde_json::json!({}),
            outputs_raw: serde_json::json!({}),
            enabled: true,
            source: source.to_string(),
            health_status: "unknown".to_string(),
            custom_parameters: serde_json::json!({}),
        }
    }

    #[test]
    fn test_resolve_staged_plugin_root_pure() {
        let root = PathBuf::from("/staging");

        // 1. Root manifest exists
        let res = resolve_staged_plugin_root(&root, true, vec![]);
        assert_eq!(res.unwrap(), root);

        // 2. Single subdirectory candidate
        let sub = PathBuf::from("/staging/my-plugin");
        let res = resolve_staged_plugin_root(&root, false, vec![sub.clone()]);
        assert_eq!(res.unwrap(), sub);

        // 3. No candidates
        let res = resolve_staged_plugin_root(&root, false, vec![]);
        assert!(matches!(res, Err(PluginError::ManifestInvalid { .. })));

        // 4. Ambiguous candidates (>1)
        let sub2 = PathBuf::from("/staging/other-plugin");
        let res = resolve_staged_plugin_root(&root, false, vec![sub, sub2]);
        assert!(matches!(res, Err(PluginError::ManifestInvalid { .. })));
    }

    #[test]
    fn test_validate_installation_preconditions_pure() {
        // 1. Invalid ID
        let res = validate_installation_preconditions("INVALID_ID", false, None, None);
        assert!(matches!(res, Err(PluginError::SecurityViolation(_))));

        // 2. Active job running
        let res = validate_installation_preconditions("valid-id", true, None, None);
        assert!(matches!(res, Err(PluginError::ExecutionError(_))));

        // 3. Cannot overwrite builtin in registry
        let res = validate_installation_preconditions("valid-id", false, Some("builtin"), None);
        assert!(matches!(res, Err(PluginError::SecurityViolation(_))));

        // 4. Cannot overwrite builtin in state store
        let res = validate_installation_preconditions(
            "valid-id",
            false,
            None,
            Some(&PluginSource::Builtin),
        );
        assert!(matches!(res, Err(PluginError::SecurityViolation(_))));

        // 5. Valid installation (new user plugin or user plugin overwrite)
        let res = validate_installation_preconditions(
            "valid-id",
            false,
            Some("user"),
            Some(&PluginSource::User),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_assemble_user_plugin_bundle_pure() {
        let manifest = make_manifest("user-test", "1.0.0");
        let inputs_spec = make_inputs_spec();

        let bundle = assemble_user_plugin_bundle(
            PathBuf::from("/plugins/user-test"),
            manifest,
            inputs_spec,
            serde_json::json!({}),
            serde_json::json!({"parameters": []}),
            serde_json::json!({}),
        );

        assert_eq!(bundle.manifest.metadata.id, "user-test");
        assert_eq!(bundle.source, "user");
        assert!(bundle.enabled);
        assert_eq!(bundle.install_path, PathBuf::from("/plugins/user-test"));
    }

    #[test]
    fn test_with_install_update_pure() {
        let original = empty_store();
        let now_iso = "2026-11-02T12:00:00Z";

        let next = with_install_update(&original, "my-plugin", "1.2.3", now_iso);

        assert!(original.plugins.is_empty());
        assert_eq!(original.updated_at, "2026-11-01T00:00:00Z");

        assert_eq!(next.updated_at, now_iso);
        let record = next.plugins.get("my-plugin").expect("record must exist");
        assert_eq!(record.version, "1.2.3");
        assert_eq!(record.source, PluginSource::User);
        assert!(record.enabled);
        assert_eq!(record.installed_at, now_iso);

        // Reinstall updates version and preserves installed_at + custom_parameters
        let mut with_custom = next.clone();
        with_custom
            .plugins
            .get_mut("my-plugin")
            .unwrap()
            .custom_parameters = serde_json::json!({"threshold": 0.7});
        let reinstall_iso = "2026-11-05T09:00:00Z";
        let reinstalled = with_install_update(&with_custom, "my-plugin", "2.0.0", reinstall_iso);

        let rec2 = reinstalled.plugins.get("my-plugin").unwrap();
        assert_eq!(rec2.version, "2.0.0");
        assert_eq!(rec2.installed_at, now_iso);
        assert_eq!(rec2.custom_parameters["threshold"], 0.7);
        assert_eq!(
            with_custom.plugins.get("my-plugin").unwrap().version,
            "1.2.3"
        );
    }

    #[test]
    fn test_with_status_update_insert_new_record_pure() {
        let original = empty_store();
        let now_iso = "2026-11-03T01:00:00Z";

        let next = with_status_update(
            &original,
            "new-plugin",
            false,
            "2.0.0",
            PluginSource::Builtin,
            now_iso,
        );

        assert!(original.plugins.is_empty());
        assert_eq!(next.updated_at, now_iso);
        let record = next.plugins.get("new-plugin").expect("record must exist");
        assert_eq!(record.version, "2.0.0");
        assert_eq!(record.source, PluginSource::Builtin);
        assert!(!record.enabled);
        assert_eq!(record.installed_at, now_iso);
    }

    #[test]
    fn test_with_status_update_update_existing_record_pure() {
        let initial_iso = "2026-11-01T00:00:00Z";
        let installed = with_install_update(&empty_store(), "toggle-plugin", "1.0.0", initial_iso);
        assert!(installed.plugins.get("toggle-plugin").unwrap().enabled);

        let update_iso = "2026-11-03T01:30:00Z";
        let toggled = with_status_update(
            &installed,
            "toggle-plugin",
            false,
            "1.0.0",
            PluginSource::User,
            update_iso,
        );

        // Original remains enabled
        assert!(installed.plugins.get("toggle-plugin").unwrap().enabled);

        assert_eq!(toggled.updated_at, update_iso);
        let record = toggled
            .plugins
            .get("toggle-plugin")
            .expect("record must exist");
        assert_eq!(record.version, "1.0.0");
        assert_eq!(record.source, PluginSource::User);
        assert!(!record.enabled);
        assert_eq!(record.installed_at, initial_iso);
    }

    #[test]
    fn test_validate_custom_parameters_parallel_valid_and_invalid() {
        let schema = serde_json::json!({
            "parameters": [
                { "name": "enabled_flag", "type": "boolean" },
                { "name": "label", "type": "string" },
                { "name": "mode", "type": "select", "options": ["FAST", "ACCURATE"] },
                { "name": "threshold", "type": "number", "min": 0.0, "max": 1.0 },
                { "name": "broken_type", "type": "matrix" }
            ]
        });

        // Valid multi-entry object (exercised across rayon parallel iteration)
        let valid = serde_json::json!({
            "enabled_flag": true,
            "label": "field-a",
            "mode": "ACCURATE",
            "threshold": 0.75
        });
        assert!(validate_custom_parameters(&schema, &valid).is_ok());
        assert!(validate_custom_parameters(&schema, &serde_json::json!({})).is_ok());

        // Non-object root
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!(["not-an-object"])),
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Unknown parameter
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "unknown": 1 })),
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid boolean
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "enabled_flag": "yes" })),
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid string
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "label": 42 })),
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid select non-string & out-of-options
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "mode": 123 })),
            Err(PluginError::SubcontractInvalid { .. })
        ));
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "mode": "TURBO" })),
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid number type & bounds
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "threshold": "high" })),
            Err(PluginError::SubcontractInvalid { .. })
        ));
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "threshold": -0.01 })),
            Err(PluginError::SubcontractInvalid { .. })
        ));
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "threshold": 1.01 })),
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Unsupported schema parameter type
        assert!(matches!(
            validate_custom_parameters(&schema, &serde_json::json!({ "broken_type": "x" })),
            Err(PluginError::SubcontractInvalid { .. })
        ));
    }

    #[test]
    fn test_with_config_update_inserts_updates_and_preserves_original() {
        let original = empty_store();
        let params_v1 = serde_json::json!({ "threshold": 0.4 });
        let t1 = "2026-11-04T10:00:00Z";

        let created = with_config_update(
            &original,
            &ConfigUpdateInput {
                plugin_id: "cfg-plugin",
                version: "1.0.0",
                source: PluginSource::User,
                enabled: true,
                custom_params: &params_v1,
            },
            t1,
        );

        assert!(original.plugins.is_empty());
        assert_eq!(created.updated_at, t1);
        let rec1 = created.plugins.get("cfg-plugin").unwrap();
        assert_eq!(rec1.custom_parameters, params_v1);
        assert_eq!(rec1.installed_at, t1);
        assert_eq!(rec1.source, PluginSource::User);

        let params_v2 = serde_json::json!({ "threshold": 0.9 });
        let t2 = "2026-11-04T11:00:00Z";
        let updated = with_config_update(
            &created,
            &ConfigUpdateInput {
                plugin_id: "cfg-plugin",
                version: "2.0.0",
                source: PluginSource::Builtin,
                enabled: false,
                custom_params: &params_v2,
            },
            t2,
        );

        assert_eq!(
            created.plugins.get("cfg-plugin").unwrap().custom_parameters,
            params_v1
        );
        let rec2 = updated.plugins.get("cfg-plugin").unwrap();
        assert_eq!(rec2.custom_parameters, params_v2);
        assert_eq!(rec2.installed_at, t1);
        assert_eq!(rec2.version, "1.0.0");
        assert_eq!(updated.updated_at, t2);
    }

    #[test]
    fn test_with_removed_plugin_pure() {
        let store =
            with_install_update(&empty_store(), "to-remove", "1.0.0", "2026-11-01T00:00:00Z");
        let store = with_install_update(&store, "to-keep", "1.0.0", "2026-11-01T00:00:00Z");

        let t_rm = "2026-11-06T08:00:00Z";
        let after = with_removed_plugin(&store, "to-remove", t_rm);

        assert!(store.plugins.contains_key("to-remove"));
        assert!(!after.plugins.contains_key("to-remove"));
        assert!(after.plugins.contains_key("to-keep"));
        assert_eq!(after.updated_at, t_rm);

        // Removing an absent plugin still returns a fresh store with updated timestamp
        let t_absent = "2026-11-06T09:00:00Z";
        let after_absent = with_removed_plugin(&after, "missing", t_absent);
        assert_eq!(after_absent.updated_at, t_absent);
        assert!(after_absent.plugins.contains_key("to-keep"));
    }

    #[test]
    fn test_with_applied_state_pure() {
        let mut registry = PluginRegistry::new();
        registry.register(make_bundle(
            "alpha",
            "builtin",
            PathBuf::from("/plugins/alpha"),
        ));
        registry.register(make_bundle("beta", "user", PathBuf::from("/plugins/beta")));

        let mut store = empty_store();
        store.plugins.insert(
            "alpha".to_string(),
            PluginStateRecord {
                version: "1.0.0".to_string(),
                source: PluginSource::User,
                enabled: false,
                installed_at: "2026-11-01T00:00:00Z".to_string(),
                last_healthcheck: Some(CachedHealthcheck {
                    status: "healthy".to_string(),
                    checked_at: "2026-11-02T00:00:00Z".to_string(),
                    message: None,
                }),
                custom_parameters: serde_json::json!({"threshold": 0.33}),
                stats: PluginStats::default(),
            },
        );
        store.plugins.insert(
            "orphan".to_string(),
            PluginStateRecord {
                version: "1.0.0".to_string(),
                source: PluginSource::User,
                enabled: true,
                installed_at: "2026-11-01T00:00:00Z".to_string(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({}),
                stats: PluginStats::default(),
            },
        );

        let applied = with_applied_state(&registry, &store);

        // Original registry is unmutated
        let orig_alpha = registry.plugins.get("alpha").unwrap();
        assert!(orig_alpha.enabled);
        assert_eq!(orig_alpha.source, "builtin");
        assert_eq!(orig_alpha.health_status, "unknown");

        // Applied registry reflects persisted record
        let next_alpha = applied.plugins.get("alpha").unwrap();
        assert!(!next_alpha.enabled);
        assert_eq!(next_alpha.source, "user");
        assert_eq!(next_alpha.health_status, "healthy");
        assert_eq!(next_alpha.custom_parameters["threshold"], 0.33);

        // Unmatched bundle stays intact; orphan state entry is ignored
        let next_beta = applied.plugins.get("beta").unwrap();
        assert!(next_beta.enabled);
        assert!(!applied.plugins.contains_key("orphan"));
    }

    #[test]
    fn test_plan_plugin_removal_error_and_success_paths() {
        let plugins_dir = PathBuf::from("/app/plugins");
        let now_iso = "2026-11-07T12:00:00Z";

        let mut registry = PluginRegistry::new();
        registry.register(make_bundle(
            "builtin-reg",
            "builtin",
            plugins_dir.join("builtin-reg"),
        ));
        registry.register(make_bundle(
            "user-reg",
            "user",
            PathBuf::from("/custom/path/user-reg"),
        ));

        let mut store = empty_store();
        store.plugins.insert(
            "builtin-store".to_string(),
            PluginStateRecord {
                version: "1.0.0".to_string(),
                source: PluginSource::Builtin,
                enabled: true,
                installed_at: "2026-11-01T00:00:00Z".to_string(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({}),
                stats: PluginStats::default(),
            },
        );
        store.plugins.insert(
            "user-reg".to_string(),
            PluginStateRecord {
                version: "1.0.0".to_string(),
                source: PluginSource::User,
                enabled: true,
                installed_at: "2026-11-01T00:00:00Z".to_string(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({}),
                stats: PluginStats::default(),
            },
        );
        store.plugins.insert(
            "user-store-only".to_string(),
            PluginStateRecord {
                version: "1.0.0".to_string(),
                source: PluginSource::User,
                enabled: true,
                installed_at: "2026-11-01T00:00:00Z".to_string(),
                last_healthcheck: None,
                custom_parameters: serde_json::json!({}),
                stats: PluginStats::default(),
            },
        );

        // 1. Invalid ID -> NotFound
        let err_invalid = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "../../etc/passwd",
                is_job_active: false,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .unwrap_err();
        assert!(matches!(err_invalid, PluginError::NotFound(_)));

        // 2. Active job -> ExecutionError
        let err_busy = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "user-reg",
                is_job_active: true,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .unwrap_err();
        assert!(matches!(err_busy, PluginError::ExecutionError(_)));

        // 3. Not found in both registry and store -> NotFound
        let err_missing = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "ghost-plugin",
                is_job_active: false,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .unwrap_err();
        assert!(matches!(err_missing, PluginError::NotFound(_)));

        // 4. Builtin in registry -> SecurityViolation
        let err_builtin_reg = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "builtin-reg",
                is_job_active: false,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .unwrap_err();
        assert!(matches!(err_builtin_reg, PluginError::SecurityViolation(_)));

        // 5. Builtin in store only -> SecurityViolation
        let err_builtin_store = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "builtin-store",
                is_job_active: false,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .unwrap_err();
        assert!(matches!(
            err_builtin_store,
            PluginError::SecurityViolation(_)
        ));

        // 6. Success when registered -> uses bundle.install_path
        let plan_reg = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "user-reg",
                is_job_active: false,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .expect("registered user plugin should plan removal");
        assert_eq!(
            plan_reg.install_dir_to_remove,
            PathBuf::from("/custom/path/user-reg")
        );
        assert!(!plan_reg.updated_store.plugins.contains_key("user-reg"));
        assert_eq!(plan_reg.updated_store.updated_at, now_iso);
        assert!(store.plugins.contains_key("user-reg"));

        // 7. Success when store-only -> falls back to plugins_dir.join(plugin_id)
        let plan_store = plan_plugin_removal(
            &registry,
            &store,
            &RemovalPlanInput {
                plugin_id: "user-store-only",
                is_job_active: false,
                plugins_dir: &plugins_dir,
                now_iso,
            },
        )
        .expect("store-only user plugin should plan removal");
        assert_eq!(
            plan_store.install_dir_to_remove,
            plugins_dir.join("user-store-only")
        );
        assert!(!plan_store
            .updated_store
            .plugins
            .contains_key("user-store-only"));
    }
}
