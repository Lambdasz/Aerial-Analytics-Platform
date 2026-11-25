//! # Domain 1 (Pure Core): Registry State, Compatibility & Schema Validation
//!
//! **Role**: Pure functional core for plugin indexing, compatibility queries,
//! manifest/subcontract parsing & validation, and bundle assembly.
//!
//! ## Purity Boundary
//!
//! This module is strictly pure. It MUST NOT import or invoke any side-effectful
//! primitives (`std::fs`, `tokio`, `rusqlite`, `chrono::Utc::now()`,
//! `uuid::Uuid::new_v4()`, `crate::AppState`, or `eprintln!`). All disk I/O,
//! subprocess health checks, and Tauri state synchronization live in
//! [`super::registry`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use super::error::PluginError;
use super::models::{
    InputsSpec, PluginBundle, PluginDetailsDto, PluginManifest, PluginSummaryDto,
    TargetDescriptorDto,
};

// ---------------------------------------------------------------------------
// 1. IN-MEMORY PLUGIN REGISTRY
// ---------------------------------------------------------------------------

/// In-memory storage for discovered and validated plugin bundles.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    pub plugins: HashMap<String, PluginBundle>,
}

impl PluginRegistry {
    /// Creates an empty [`PluginRegistry`].
    ///
    /// # Purity
    /// Pure constructor; allocates no external resources and performs no I/O.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a validated plugin bundle into the in-memory registry.
    ///
    /// # Purity
    /// Pure in-memory state transition on `&mut self`; performs no I/O or external side effects.
    pub fn register(&mut self, bundle: PluginBundle) {
        self.plugins
            .insert(bundle.manifest.metadata.id.clone(), bundle);
    }

    /// Updates the active/inactive status of a plugin.
    ///
    /// # Purity
    /// Pure in-memory state transition on `&mut self`; performs no I/O or persistence.
    pub fn set_status(&mut self, plugin_id: &str, enabled: bool) -> Result<(), PluginError> {
        let bundle = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        bundle.enabled = enabled;
        Ok(())
    }

    /// Returns a list of summaries for all registered plugins.
    ///
    /// # Purity
    /// Pure read-only parallel projection over `self.plugins` via `rayon`; no I/O or mutation.
    pub fn get_summaries(&self) -> Vec<PluginSummaryDto> {
        self.plugins
            .par_iter()
            .map(|(_, bundle)| bundle.to_summary())
            .collect()
    }

    /// Resolves and returns full contract schemas and details for a specific plugin.
    ///
    /// # Purity
    /// Pure read-only lookup and DTO projection; performs no I/O or mutation.
    pub fn get_details(&self, plugin_id: &str) -> Option<PluginDetailsDto> {
        self.plugins.get(plugin_id).map(|bundle| PluginDetailsDto {
            manifest: bundle.manifest.clone(),
            parameters: bundle.parameters_raw.clone(),
            inputs: if bundle.inputs_raw.is_null() {
                serde_json::to_value(&bundle.inputs_spec).unwrap_or_default()
            } else {
                bundle.inputs_raw.clone()
            },
            outputs: bundle.outputs_raw.clone(),
            install_path: bundle.install_path.to_string_lossy().into_owned(),
            enabled: bundle.enabled,
            custom_parameters: bundle.custom_parameters.clone(),
        })
    }

    /// Queries plugins compatible with the provided geospatial target.
    ///
    /// # Purity
    /// Pure read-only parallel filter/map over `self.plugins` via `rayon`; no I/O or mutation.
    pub fn query_compatible(&self, target: &TargetDescriptorDto) -> Vec<PluginSummaryDto> {
        self.plugins
            .par_iter()
            .filter(|(_, bundle)| bundle.enabled && is_compatible(&bundle.inputs_spec, target))
            .map(|(_, bundle)| bundle.to_summary())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// 2. PURE FUNCTIONS & DOMAIN LOGIC
// ---------------------------------------------------------------------------

/// Pure predicate to check if a plugin's input specification is compatible with a given target.
///
/// Evaluates MIME type match, granularity match, AOI support, and all metadata requirements (GPS, Altitude).
///
/// # Purity
/// Pure deterministic predicate over immutable references; zero I/O, zero side effects.
pub fn is_compatible(spec: &InputsSpec, target: &TargetDescriptorDto) -> bool {
    let mime_match = spec
        .supported_mime_types
        .iter()
        .any(|m| m.eq_ignore_ascii_case(&target.mime_type));
    let granularity_match = spec.granularity.eq_ignore_ascii_case(&target.granularity);
    let aoi_valid = !target.has_aoi || spec.supports_aoi;
    let metadata_valid = spec.requires_metadata.iter().all(|req| {
        if req.eq_ignore_ascii_case("gps") {
            target.has_gps
        } else if req.eq_ignore_ascii_case("altitude") {
            target.has_altitude
        } else if req.eq_ignore_ascii_case("resolution") {
            target.has_resolution
        } else {
            false
        }
    });

    mime_match && granularity_match && aoi_valid && metadata_valid
}

/// Checks whether a path string is a safe relative path that does not escape its parent sandbox.
///
/// # Purity
/// Pure lexical path inspection; performs no filesystem stat or canonicalization I/O.
pub(crate) fn is_safe_relative_path(p: &str) -> bool {
    let path = Path::new(p);
    if path.is_absolute() {
        return false;
    }
    !path.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::Prefix(_)
        )
    })
}

/// Parses and strictly validates manifest JSON string against `plugin.schema.json`.
///
/// # Purity
/// Pure deterministic parser and validator; zero mutation, zero I/O, zero side effects.
pub(crate) fn parse_and_validate_manifest(raw_json: &str) -> Result<PluginManifest, PluginError> {
    let manifest: PluginManifest =
        serde_json::from_str(raw_json).map_err(|e| PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: e.to_string(),
        })?;

    if manifest.manifest_version < 1 {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: "manifest_version must be >= 1".to_string(),
        });
    }

    let id = &manifest.metadata.id;
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: format!("Plugin id '{}' must match ^[a-z0-9-_]+$", id),
        });
    }

    if manifest.metadata.name.trim().is_empty() {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: "metadata.name must not be empty".to_string(),
        });
    }

    let version = &manifest.metadata.version;
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3
        || !parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: format!(
                "Version '{}' does not conform to semver (\\d+\\.\\d+\\.\\d+)",
                version
            ),
        });
    }

    const VALID_CATEGORIES: &[&str] = &[
        "vegetation",
        "tree_detection",
        "tree_counting",
        "land_cover",
        "temporal",
        "elevation",
        "analytics",
        "utility",
        "general",
    ];
    if let Some(ref cat) = manifest.metadata.category {
        if !VALID_CATEGORIES.contains(&cat.as_str()) {
            return Err(PluginError::ManifestInvalid {
                path: "manifest.json".to_string(),
                message: format!(
                    "Invalid category '{}', must be one of: {:?}",
                    cat, VALID_CATEGORIES
                ),
            });
        }
    }

    let rt = manifest.runtime.runtime_type.as_str();
    if rt != "python" && rt != "binary" && rt != "wasm" {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: format!(
                "Unsupported runtime type '{}', must be 'python', 'binary', or 'wasm'",
                rt
            ),
        });
    }

    if manifest.runtime.entrypoint.trim().is_empty()
        || !is_safe_relative_path(&manifest.runtime.entrypoint)
    {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: format!(
                "runtime.entrypoint '{}' must be a safe non-empty relative path without parent traversal",
                manifest.runtime.entrypoint
            ),
        });
    }

    for (name, path_str) in [
        ("parameters", &manifest.parameters),
        ("inputs", &manifest.inputs),
        ("outputs", &manifest.outputs),
    ] {
        if path_str.trim().is_empty() || !is_safe_relative_path(path_str) {
            return Err(PluginError::ManifestInvalid {
                path: "manifest.json".to_string(),
                message: format!(
                    "Subcontract '{}' path '{}' must be a safe non-empty relative path without parent traversal",
                    name, path_str
                ),
            });
        }
    }

    Ok(manifest)
}

/// Parses and schema-validates parameters JSON from raw string content.
///
/// **Pre-condition**: `raw_json` is a valid UTF-8 JSON string.
/// **Post-condition**: Deterministically validates that `parameters` array exists,
/// each parameter satisfies identifier naming (`^[a-z0-9_]+$`), label, valid type, and default value.
///
/// # Purity
/// Pure deterministic parser and validator; zero I/O, zero mutation.
pub(crate) fn parse_and_validate_parameters(
    raw_json: &str,
    path_label: &str,
) -> Result<serde_json::Value, PluginError> {
    let params_val: serde_json::Value =
        serde_json::from_str(raw_json).map_err(|e| PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: format!("Failed to parse parameters JSON: {}", e),
        })?;

    let params_arr = params_val
        .get("parameters")
        .and_then(|p| p.as_array())
        .ok_or_else(|| PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: "Parameters definition must contain a 'parameters' array".to_string(),
        })?;

    const VALID_PARAM_TYPES: &[&str] = &["number", "boolean", "select", "string"];
    for (idx, p) in params_arr.iter().enumerate() {
        let p_obj = p
            .as_object()
            .ok_or_else(|| PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!("Parameter at index {} must be an object", idx),
            })?;

        let name = p_obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!("Parameter at index {} missing required 'name'", idx),
            }
        })?;

        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!("Parameter '{}' must match pattern ^[a-z0-9_]+$", name),
            });
        }

        if p_obj
            .get("label")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!("Parameter '{}' missing required non-empty 'label'", name),
            });
        }

        let p_type = p_obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!("Parameter '{}' missing required 'type'", name),
            }
        })?;

        if !VALID_PARAM_TYPES.contains(&p_type) {
            return Err(PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!(
                    "Parameter '{}' has invalid type '{}', expected one of: {:?}",
                    name, p_type, VALID_PARAM_TYPES
                ),
            });
        }

        if !p_obj.contains_key("default") {
            return Err(PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!("Parameter '{}' missing required 'default' value", name),
            });
        }
    }

    Ok(params_val)
}

/// Parses and schema-validates inputs JSON from raw string content.
///
/// **Pre-condition**: `raw_json` is a valid UTF-8 JSON string.
/// **Post-condition**: Deterministically validates that `supported_mime_types` is non-empty,
/// granularity is supported, and all MIME types are valid aerial imagery formats.
///
/// # Purity
/// Pure deterministic parser and validator; zero I/O, zero mutation.
pub(crate) fn parse_and_validate_inputs(
    raw_json: &str,
    path_label: &str,
) -> Result<InputsSpec, PluginError> {
    let inputs_spec: InputsSpec =
        serde_json::from_str(raw_json).map_err(|e| PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: format!("Failed to parse inputs specification: {}", e),
        })?;

    if inputs_spec.supported_mime_types.is_empty() {
        return Err(PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: "supported_mime_types must contain at least one MIME type".to_string(),
        });
    }

    const VALID_GRANULARITIES: &[&str] = &[
        "single_image",
        "image_pair",
        "batch",
        "session",
        "flight_session",
        "multi_image",
    ];
    if !VALID_GRANULARITIES.contains(&inputs_spec.granularity.as_str()) {
        return Err(PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: format!(
                "Invalid granularity '{}', expected one of: {:?}",
                inputs_spec.granularity, VALID_GRANULARITIES
            ),
        });
    }

    const VALID_MIME_TYPES: &[&str] = &[
        "image/jpeg",
        "image/png",
        "image/tiff",
        "application/x-geotiff",
    ];
    for mime in &inputs_spec.supported_mime_types {
        if !VALID_MIME_TYPES.contains(&mime.to_ascii_lowercase().as_str()) {
            return Err(PluginError::SubcontractInvalid {
                path: path_label.to_string(),
                message: format!(
                    "Unsupported MIME type '{}', expected one of: {:?}",
                    mime, VALID_MIME_TYPES
                ),
            });
        }
    }

    Ok(inputs_spec)
}

/// Parses and schema-validates outputs JSON from raw string content.
///
/// **Pre-condition**: `raw_json` is a valid UTF-8 JSON string.
/// **Post-condition**: Deterministically validates that output is a JSON object and any declared metrics
/// contain valid keys and supported data types.
///
/// # Purity
/// Pure deterministic parser and validator; zero I/O, zero mutation.
pub(crate) fn parse_and_validate_outputs(
    raw_json: &str,
    path_label: &str,
) -> Result<serde_json::Value, PluginError> {
    let outputs_val: serde_json::Value =
        serde_json::from_str(raw_json).map_err(|e| PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: format!("Failed to parse outputs JSON: {}", e),
        })?;
    if !outputs_val.is_object() {
        return Err(PluginError::SubcontractInvalid {
            path: path_label.to_string(),
            message: "Outputs definition must be a JSON object".to_string(),
        });
    }

    if let Some(metrics_arr) = outputs_val.get("metrics").and_then(|m| m.as_array()) {
        const VALID_METRIC_TYPES: &[&str] = &["float", "integer", "string", "boolean"];
        for (idx, m) in metrics_arr.iter().enumerate() {
            let m_obj = m
                .as_object()
                .ok_or_else(|| PluginError::SubcontractInvalid {
                    path: path_label.to_string(),
                    message: format!("Metric at index {} must be an object", idx),
                })?;
            let key = m_obj.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
                PluginError::SubcontractInvalid {
                    path: path_label.to_string(),
                    message: format!("Metric at index {} missing required 'key'", idx),
                }
            })?;
            let m_type = m_obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
                PluginError::SubcontractInvalid {
                    path: path_label.to_string(),
                    message: format!("Metric '{}' missing required 'type'", key),
                }
            })?;
            if !VALID_METRIC_TYPES.contains(&m_type) {
                return Err(PluginError::SubcontractInvalid {
                    path: path_label.to_string(),
                    message: format!(
                        "Metric '{}' has invalid type '{}', expected one of: {:?}",
                        key, m_type, VALID_METRIC_TYPES
                    ),
                });
            }
        }
    }

    Ok(outputs_val)
}

/// Assembles a `PluginBundle` from validated components.
///
/// # Purity
/// Pure deterministic constructor; zero I/O, zero side effects.
pub(crate) fn assemble_plugin_bundle(
    install_path: PathBuf,
    manifest: PluginManifest,
    inputs_spec: InputsSpec,
    inputs_raw: serde_json::Value,
    parameters_raw: serde_json::Value,
    outputs_raw: serde_json::Value,
) -> PluginBundle {
    PluginBundle {
        install_path,
        manifest,
        inputs_spec,
        inputs_raw,
        parameters_raw,
        outputs_raw,
        enabled: true,
        source: "builtin".to_string(),
        health_status: "unknown".to_string(),
        custom_parameters: serde_json::json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::{InputsSpec, TargetDescriptorDto};
    use std::path::PathBuf;

    fn default_inputs_spec() -> InputsSpec {
        InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string(), "image/tiff".to_string()],
            requires_metadata: Vec::new(),
            optional_metadata: Vec::new(),
            supports_aoi: true,
            aoi_format: None,
        }
    }

    fn default_target_descriptor() -> TargetDescriptorDto {
        TargetDescriptorDto {
            granularity: "single_image".to_string(),
            mime_type: "image/jpeg".to_string(),
            has_gps: true,
            has_altitude: true,
            has_aoi: false,
            has_resolution: true,
        }
    }

    #[test]
    fn test_is_compatible_mime_case_insensitive() {
        let spec = default_inputs_spec();
        let mut target = default_target_descriptor();
        target.mime_type = "IMAGE/JPEG".to_string();

        assert!(
            is_compatible(&spec, &target),
            "MIME matching must be case-insensitive"
        );
    }

    #[test]
    fn test_is_compatible_mime_mismatch() {
        let spec = default_inputs_spec();
        let mut target = default_target_descriptor();
        target.mime_type = "image/png".to_string();

        assert!(!is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_granularity_case_insensitive() {
        let spec = default_inputs_spec();
        let mut target = default_target_descriptor();
        target.granularity = "SINGLE_IMAGE".to_string();

        assert!(is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_granularity_mismatch() {
        let spec = default_inputs_spec();
        let mut target = default_target_descriptor();
        target.granularity = "batch".to_string();

        assert!(!is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_aoi_support() {
        let mut spec = default_inputs_spec();
        spec.supports_aoi = false;

        let mut target = default_target_descriptor();
        target.has_aoi = true;

        assert!(
            !is_compatible(&spec, &target),
            "Target with AOI must be rejected if plugin does not support AOI"
        );

        target.has_aoi = false;
        assert!(is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_gps_metadata_enforced() {
        let mut spec = default_inputs_spec();
        spec.requires_metadata = vec!["gps".to_string()];

        let mut target = default_target_descriptor();
        target.has_gps = false;
        assert!(
            !is_compatible(&spec, &target),
            "Plugin requiring GPS must reject target without GPS"
        );

        target.has_gps = true;
        assert!(is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_altitude_metadata_enforced() {
        let mut spec = default_inputs_spec();
        spec.requires_metadata = vec!["altitude".to_string()];

        let mut target = default_target_descriptor();
        target.has_altitude = false;
        assert!(
            !is_compatible(&spec, &target),
            "Plugin requiring altitude must reject target without altitude"
        );

        target.has_altitude = true;
        assert!(is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_combined_metadata_requirements() {
        let mut spec = default_inputs_spec();
        spec.requires_metadata = vec!["GPS".to_string(), "ALTITUDE".to_string()];

        let mut target = default_target_descriptor();
        target.has_gps = true;
        target.has_altitude = false;
        assert!(!is_compatible(&spec, &target));

        target.has_altitude = true;
        assert!(is_compatible(&spec, &target));
    }

    #[test]
    fn test_is_compatible_unsupported_metadata_rejected() {
        let mut spec = default_inputs_spec();
        spec.requires_metadata = vec!["hyperspectral".to_string()];

        let target = default_target_descriptor();
        assert!(
            !is_compatible(&spec, &target),
            "Unknown or unsupported metadata requirements must be rejected"
        );
    }

    #[test]
    fn test_is_compatible_resolution_metadata_enforced() {
        let mut spec = default_inputs_spec();
        spec.requires_metadata = vec!["resolution".to_string()];

        let mut target = default_target_descriptor();
        target.has_resolution = false;
        assert!(
            !is_compatible(&spec, &target),
            "Plugin requiring resolution must reject target without resolution"
        );

        target.has_resolution = true;
        assert!(
            is_compatible(&spec, &target),
            "Plugin requiring resolution must accept target with resolution"
        );
    }

    #[test]
    fn test_is_compatible_real_plugins_compatible() {
        // Real plugins in plugins/ directory require ["resolution"] or ["resolution", "gps"]
        let veg_spec = InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec![
                "image/jpeg".to_string(),
                "image/png".to_string(),
                "image/tiff".to_string(),
            ],
            requires_metadata: vec!["resolution".to_string()],
            optional_metadata: vec![
                "gps".to_string(),
                "altitude".to_string(),
                "timestamp".to_string(),
            ],
            supports_aoi: true,
            aoi_format: Some("geojson_polygon".to_string()),
        };

        let mut target = default_target_descriptor();
        target.has_resolution = true;
        target.has_gps = true;
        target.has_altitude = true;
        target.mime_type = "image/jpeg".to_string();

        assert!(
            is_compatible(&veg_spec, &target),
            "Standard drone image target must be compatible with vegetation detection plugin"
        );

        let landcover_spec = InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec![
                "image/jpeg".to_string(),
                "image/png".to_string(),
                "image/tiff".to_string(),
            ],
            requires_metadata: vec!["resolution".to_string(), "gps".to_string()],
            optional_metadata: vec!["altitude".to_string(), "timestamp".to_string()],
            supports_aoi: true,
            aoi_format: Some("geojson_polygon".to_string()),
        };

        assert!(
            is_compatible(&landcover_spec, &target),
            "Target with resolution and GPS must be compatible with landcover classification plugin"
        );
    }

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
            inputs_spec: default_inputs_spec(),
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

    #[test]
    fn test_plugin_registry_register_and_get_summaries() {
        let mut registry = PluginRegistry::new();
        registry.register(mock_bundle("rgb-exg"));

        let summaries = registry.get_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "rgb-exg");
        assert_eq!(summaries[0].runtime_type, "python");
    }

    #[test]
    fn test_plugin_registry_get_details_found_and_not_found() {
        let mut registry = PluginRegistry::new();
        registry.register(mock_bundle("rgb-exg"));

        let details = registry.get_details("rgb-exg");
        assert!(details.is_some());
        assert_eq!(details.unwrap().manifest.metadata.id, "rgb-exg");

        let missing = registry.get_details("non-existent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_plugin_registry_query_compatible() {
        let mut registry = PluginRegistry::new();
        let mut bundle = mock_bundle("rgb-exg");
        bundle.inputs_spec.supported_mime_types = vec!["image/jpeg".to_string()];
        registry.register(bundle);

        let mut target = default_target_descriptor();
        target.mime_type = "image/jpeg".to_string();
        let matches = registry.query_compatible(&target);
        assert_eq!(matches.len(), 1);

        target.mime_type = "image/png".to_string();
        let no_matches = registry.query_compatible(&target);
        assert_eq!(no_matches.len(), 0);
    }

    #[test]
    fn test_inputs_spec_serialization_omits_none_aoi_format() {
        let spec = InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string()],
            requires_metadata: vec!["resolution".to_string()],
            optional_metadata: vec![],
            supports_aoi: false,
            aoi_format: None,
        };

        let val = serde_json::to_value(&spec).expect("serialize InputsSpec");
        assert!(
            val.get("aoi_format").is_none(),
            "None aoi_format must not be serialized as null (must be omitted according to inputs.schema.json)"
        );
    }

    #[test]
    fn test_plugin_details_preserves_raw_inputs_without_schema_violation() {
        let mut registry = PluginRegistry::new();
        let mut bundle = mock_bundle("test-plugin");
        bundle.inputs_raw = serde_json::json!({
            "granularity": "single_image",
            "supported_mime_types": ["image/jpeg"]
        });
        registry.register(bundle);

        let details = registry.get_details("test-plugin").expect("get_details");
        assert_eq!(
            details.inputs.get("granularity").and_then(|v| v.as_str()),
            Some("single_image")
        );
        assert!(
            details.inputs.get("aoi_format").is_none(),
            "inputs in details must not contain null aoi_format"
        );
    }

    #[test]
    fn test_plugin_registry_set_status_updates_enabled() {
        let mut registry = PluginRegistry::new();
        registry.register(mock_bundle("rgb-exg"));

        assert!(registry.get_details("rgb-exg").unwrap().enabled);
        assert!(registry.get_summaries()[0].enabled);

        let res = registry.set_status("rgb-exg", false);
        assert!(res.is_ok());

        assert!(!registry.get_details("rgb-exg").unwrap().enabled);
        assert!(!registry.get_summaries()[0].enabled);
    }

    #[test]
    fn test_plugin_registry_set_status_returns_not_found_for_missing_plugin() {
        let mut registry = PluginRegistry::new();
        let res = registry.set_status("unknown-id", false);
        assert!(matches!(res, Err(PluginError::NotFound(_))));
    }

    #[test]
    fn test_query_compatible_excludes_disabled_plugins() {
        let mut registry = PluginRegistry::new();
        let mut bundle = mock_bundle("rgb-exg");
        bundle.inputs_spec.supported_mime_types = vec!["image/jpeg".to_string()];
        registry.register(bundle);

        let mut target = default_target_descriptor();
        target.mime_type = "image/jpeg".to_string();

        assert_eq!(registry.query_compatible(&target).len(), 1);

        registry.set_status("rgb-exg", false).unwrap();
        assert_eq!(
            registry.query_compatible(&target).len(),
            0,
            "Disabled plugins must be excluded from compatibility query results"
        );

        registry.set_status("rgb-exg", true).unwrap();
        assert_eq!(registry.query_compatible(&target).len(), 1);
    }

    #[test]
    fn test_parse_and_validate_manifest_valid() {
        let raw = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "rgb-exg-test",
                "name": "RGB ExG Test",
                "version": "1.2.3",
                "description": "Test plugin description",
                "category": "vegetation",
                "tags": ["rgb", "vegetation"]
            },
            "runtime": {
                "type": "python",
                "entrypoint": "main.py"
            },
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;

        let manifest = parse_and_validate_manifest(raw).expect("manifest should be valid");
        assert_eq!(manifest.metadata.id, "rgb-exg-test");
        assert_eq!(manifest.metadata.version, "1.2.3");
        assert_eq!(manifest.runtime.runtime_type, "python");
    }

    #[test]
    fn test_parse_and_validate_manifest_invalid_json() {
        let raw = r#"{ "manifest_version": 1, broken json"#;
        let err = parse_and_validate_manifest(raw).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));
    }

    #[test]
    fn test_parse_and_validate_manifest_invalid_semver() {
        let raw = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "rgb-exg-test",
                "name": "RGB ExG Test",
                "version": "v1.2",
                "description": "Test plugin description"
            },
            "runtime": {
                "type": "python",
                "entrypoint": "main.py"
            },
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;

        let err = parse_and_validate_manifest(raw).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));
    }

    #[test]
    fn test_parse_and_validate_manifest_invalid_id() {
        let raw = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "RGB Invalid ID",
                "name": "RGB ExG Test",
                "version": "1.0.0",
                "description": "Test plugin description"
            },
            "runtime": {
                "type": "python",
                "entrypoint": "main.py"
            },
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;

        let err = parse_and_validate_manifest(raw).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));
    }

    #[test]
    fn test_parse_and_validate_manifest_invalid_runtime_type() {
        let raw = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "rgb-test",
                "name": "RGB Test",
                "version": "1.0.0",
                "description": "Test"
            },
            "runtime": {
                "type": "docker",
                "entrypoint": "entrypoint.sh"
            },
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;

        let err = parse_and_validate_manifest(raw).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));
    }

    #[test]
    fn test_parse_and_validate_manifest_rejects_path_traversal() {
        let raw = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "rgb-traversal",
                "name": "Traversal Test",
                "version": "1.0.0",
                "description": "Test"
            },
            "runtime": {
                "type": "python",
                "entrypoint": "../outside/main.py"
            },
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;
        let err = parse_and_validate_manifest(raw).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));

        let raw_abs = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "rgb-traversal",
                "name": "Traversal Test",
                "version": "1.0.0",
                "description": "Test"
            },
            "runtime": {
                "type": "python",
                "entrypoint": "main.py"
            },
            "parameters": "/etc/shadow",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;
        let err = parse_and_validate_manifest(raw_abs).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));
    }

    #[test]
    fn test_parse_and_validate_manifest_rejects_invalid_category() {
        let raw = r#"{
            "manifest_version": 1,
            "metadata": {
                "id": "rgb-cat-test",
                "name": "Category Test",
                "version": "1.0.0",
                "description": "Test",
                "category": "crypto_mining"
            },
            "runtime": {
                "type": "python",
                "entrypoint": "main.py"
            },
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }"#;
        let err = parse_and_validate_manifest(raw).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid { .. }));
    }

    #[test]
    fn test_parse_and_validate_parameters_pure() {
        let valid_json = r#"{
            "parameters": [
                {"name": "threshold_val", "label": "Threshold", "type": "number", "default": 0.25},
                {"name": "apply_blur", "label": "Blur", "type": "boolean", "default": false}
            ]
        }"#;
        let res = parse_and_validate_parameters(valid_json, "parameters.json");
        assert!(res.is_ok());

        // Invalid JSON
        let err_syntax = parse_and_validate_parameters("invalid json", "parameters.json");
        assert!(matches!(
            err_syntax,
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Missing parameters array
        let err_no_arr = parse_and_validate_parameters(r#"{"params": []}"#, "parameters.json");
        assert!(matches!(
            err_no_arr,
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid name format (uppercase or hyphen not allowed)
        let err_bad_name = parse_and_validate_parameters(
            r#"{"parameters": [{"name": "bad-name", "label": "L", "type": "string", "default": ""}]}"#,
            "parameters.json",
        );
        assert!(matches!(
            err_bad_name,
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Unknown type
        let err_bad_type = parse_and_validate_parameters(
            r#"{"parameters": [{"name": "param", "label": "L", "type": "matrix", "default": ""}]}"#,
            "parameters.json",
        );
        assert!(matches!(
            err_bad_type,
            Err(PluginError::SubcontractInvalid { .. })
        ));
    }

    #[test]
    fn test_parse_and_validate_inputs_pure() {
        let valid_json = r#"{
            "granularity": "single_image",
            "supported_mime_types": ["image/jpeg", "image/png"]
        }"#;
        let res = parse_and_validate_inputs(valid_json, "inputs.json");
        assert!(res.is_ok());
        let spec = res.unwrap();
        assert_eq!(spec.granularity, "single_image");
        assert_eq!(spec.supported_mime_types.len(), 2);

        // Empty supported MIME types
        let err_empty = parse_and_validate_inputs(
            r#"{"granularity": "single_image", "supported_mime_types": []}"#,
            "inputs.json",
        );
        assert!(matches!(
            err_empty,
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid granularity
        let err_gran = parse_and_validate_inputs(
            r#"{"granularity": "world_scale", "supported_mime_types": ["image/jpeg"]}"#,
            "inputs.json",
        );
        assert!(matches!(
            err_gran,
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Unsupported MIME type
        let err_mime = parse_and_validate_inputs(
            r#"{"granularity": "single_image", "supported_mime_types": ["video/mp4"]}"#,
            "inputs.json",
        );
        assert!(matches!(
            err_mime,
            Err(PluginError::SubcontractInvalid { .. })
        ));
    }

    #[test]
    fn test_parse_and_validate_outputs_pure() {
        let valid_json = r#"{
            "metrics": [
                {"key": "green_pct", "type": "float"},
                {"key": "tree_count", "type": "integer"}
            ],
            "artifacts": []
        }"#;
        let res = parse_and_validate_outputs(valid_json, "outputs.json");
        assert!(res.is_ok());

        // Not a JSON object
        let err_arr = parse_and_validate_outputs(r#"["not", "an", "object"]"#, "outputs.json");
        assert!(matches!(
            err_arr,
            Err(PluginError::SubcontractInvalid { .. })
        ));

        // Invalid metric type
        let err_metric_type = parse_and_validate_outputs(
            r#"{"metrics": [{"key": "score", "type": "complex_number"}]}"#,
            "outputs.json",
        );
        assert!(matches!(
            err_metric_type,
            Err(PluginError::SubcontractInvalid { .. })
        ));
    }

    #[test]
    fn test_assemble_plugin_bundle_pure() {
        let manifest = PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: crate::plugin_manager::models::PluginMetadata {
                id: "pure-test".to_string(),
                name: "Pure Test".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test description".to_string(),
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

        let inputs_spec = InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string()],
            requires_metadata: vec![],
            optional_metadata: vec![],
            supports_aoi: false,
            aoi_format: None,
        };

        let bundle = assemble_plugin_bundle(
            PathBuf::from("/path/to/plugin"),
            manifest,
            inputs_spec,
            serde_json::json!({}),
            serde_json::json!({"parameters": []}),
            serde_json::json!({}),
        );

        assert_eq!(bundle.manifest.metadata.id, "pure-test");
        assert!(bundle.enabled);
        assert_eq!(bundle.source, "builtin");
        assert_eq!(bundle.install_path, PathBuf::from("/path/to/plugin"));
    }
}
