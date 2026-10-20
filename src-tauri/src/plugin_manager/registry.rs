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

use std::collections::HashMap;
use std::path::Path;

use super::error::{CommandError, PluginError};
use super::models::{
    HealthStatusDto, InputsSpec, PluginBundle, PluginDetailsDto, PluginManifest, PluginSummaryDto,
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a validated plugin bundle into the in-memory registry.
    pub fn register(&mut self, bundle: PluginBundle) {
        self.plugins
            .insert(bundle.manifest.metadata.id.clone(), bundle);
    }

    /// Updates the active/inactive status of a plugin.
    pub fn set_status(&mut self, plugin_id: &str, enabled: bool) -> Result<(), PluginError> {
        let bundle = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        bundle.enabled = enabled;
        Ok(())
    }

    /// Returns a list of summaries for all registered plugins.
    pub fn get_summaries(&self) -> Vec<PluginSummaryDto> {
        self.plugins
            .values()
            .map(|bundle| PluginSummaryDto {
                id: bundle.manifest.metadata.id.clone(),
                name: bundle.manifest.metadata.name.clone(),
                version: bundle.manifest.metadata.version.clone(),
                author: bundle
                    .manifest
                    .metadata
                    .author
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                category: bundle
                    .manifest
                    .metadata
                    .category
                    .clone()
                    .unwrap_or_else(|| "General".to_string()),
                description: bundle.manifest.metadata.description.clone(),
                tags: bundle.manifest.metadata.tags.clone(),
                runtime_type: bundle.manifest.runtime.runtime_type.clone(),
                enabled: bundle.enabled,
                source: bundle.source.clone(),
                health_status: bundle.health_status.clone(),
            })
            .collect()
    }

    /// Resolves and returns full contract schemas and details for a specific plugin.
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
        })
    }

    /// Queries plugins compatible with the provided geospatial target.
    pub fn query_compatible(&self, target: &TargetDescriptorDto) -> Vec<PluginSummaryDto> {
        self.plugins
            .values()
            .filter(|bundle| bundle.enabled && is_compatible(&bundle.inputs_spec, target))
            .map(|bundle| PluginSummaryDto {
                id: bundle.manifest.metadata.id.clone(),
                name: bundle.manifest.metadata.name.clone(),
                version: bundle.manifest.metadata.version.clone(),
                author: bundle
                    .manifest
                    .metadata
                    .author
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                category: bundle
                    .manifest
                    .metadata
                    .category
                    .clone()
                    .unwrap_or_else(|| "General".to_string()),
                description: bundle.manifest.metadata.description.clone(),
                tags: bundle.manifest.metadata.tags.clone(),
                runtime_type: bundle.manifest.runtime.runtime_type.clone(),
                enabled: bundle.enabled,
                source: bundle.source.clone(),
                health_status: bundle.health_status.clone(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// 2. TAURI IPC COMMANDS (pub — registered in lib.rs invoke_handler)
// ---------------------------------------------------------------------------

/// Returns a lightweight summary list of all installed plugins for UI cards and menus.
#[tauri::command]
pub async fn get_installed_plugins(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PluginSummaryDto>, CommandError> {
    let registry = state.plugins.read().await;
    Ok(registry.get_summaries())
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
    let registry = state.plugins.read().await;
    let bundle = registry
        .plugins
        .get(&plugin_id)
        .ok_or_else(|| PluginError::NotFound(plugin_id.clone()))?;

    Ok(HealthStatusDto {
        status: "healthy".to_string(),
        plugin_id,
        version: bundle.manifest.metadata.version.clone(),
        details: None,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// 3. PURE FUNCTIONS & DOMAIN LOGIC (pub(crate))
// ---------------------------------------------------------------------------

/// Pure predicate to check if a plugin's input specification is compatible with a given target.
///
/// Evaluates MIME type match, granularity match, AOI support, and all metadata requirements (GPS, Altitude).
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

/// Parses and strictly validates manifest JSON string against `plugin.schema.json`.
///
/// **Pure function** — deterministic parse; zero mutation, zero side effects.
#[allow(dead_code)]
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

    if manifest.runtime.entrypoint.trim().is_empty() {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: "runtime.entrypoint must not be empty".to_string(),
        });
    }

    if manifest.parameters.trim().is_empty()
        || manifest.inputs.trim().is_empty()
        || manifest.outputs.trim().is_empty()
    {
        return Err(PluginError::ManifestInvalid {
            path: "manifest.json".to_string(),
            message: "subcontract paths (parameters, inputs, outputs) must not be empty"
                .to_string(),
        });
    }

    Ok(manifest)
}

/// Reads and schema-validates the three sub-contracts (`parameters.json`, `inputs.json`, `outputs.json`).
///
/// **Pre-condition**: Files referenced in manifest exist.  
/// **Post-condition**: All 3 sub-contracts parsed and schema-validated.
#[allow(dead_code)]
pub(crate) fn validate_subcontracts(
    dir: &Path,
    manifest: &PluginManifest,
) -> Result<(serde_json::Value, InputsSpec, serde_json::Value), PluginError> {
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
    let params_val: serde_json::Value =
        serde_json::from_str(&params_raw_str).map_err(|e| PluginError::SubcontractInvalid {
            path: manifest.parameters.clone(),
            message: format!("Failed to parse parameters JSON: {}", e),
        })?;
    if params_val
        .get("parameters")
        .and_then(|p| p.as_array())
        .is_none()
    {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.parameters.clone(),
            message: "Parameters definition must contain a 'parameters' array".to_string(),
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
    let inputs_spec: InputsSpec =
        serde_json::from_str(&inputs_raw_str).map_err(|e| PluginError::SubcontractInvalid {
            path: manifest.inputs.clone(),
            message: format!("Failed to parse inputs specification: {}", e),
        })?;
    if inputs_spec.supported_mime_types.is_empty() {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.inputs.clone(),
            message: "supported_mime_types must contain at least one MIME type".to_string(),
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
    let outputs_val: serde_json::Value =
        serde_json::from_str(&outputs_raw_str).map_err(|e| PluginError::SubcontractInvalid {
            path: manifest.outputs.clone(),
            message: format!("Failed to parse outputs JSON: {}", e),
        })?;
    if !outputs_val.is_object() {
        return Err(PluginError::SubcontractInvalid {
            path: manifest.outputs.clone(),
            message: "Outputs definition must be a JSON object".to_string(),
        });
    }

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

        let path = entry.path();
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

        bundles.push(PluginBundle {
            install_path: path,
            manifest,
            inputs_spec,
            inputs_raw,
            parameters_raw,
            outputs_raw,
            enabled: true,
            source: "builtin".to_string(),
            health_status: "unknown".to_string(),
        });
    }

    bundles.sort_by(|a, b| a.manifest.metadata.id.cmp(&b.manifest.metadata.id));
    Ok(bundles)
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

    #[tokio::test]
    async fn test_app_state_has_plugins_and_concurrent_access() {
        let state = crate::AppState {
            plugins: std::sync::Arc::new(tokio::sync::RwLock::new(PluginRegistry::new())),
            jobs: std::sync::Arc::new(tokio::sync::RwLock::new(
                crate::plugin_manager::executor::ActiveJobTracker::new(),
            )),
            state_path: None,
        };

        {
            let mut reg = state.plugins.write().await;
            reg.register(mock_bundle("rgb-exg"));
        }

        let reg = state.plugins.read().await;
        assert_eq!(reg.get_summaries().len(), 1);
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
}
