use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Plugin Manifest Definition (Conforms to schemas/plugin.schema.json)
// ---------------------------------------------------------------------------

/// Metadata describing plugin identity, author, version, and categorization.
///
/// Corresponds to the `metadata` block in `manifest.json` conforming to `plugin.schema.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub min_core_version: Option<String>,
}

/// Runtime execution environment configuration for the plugin.
///
/// Declares whether the plugin executes via Python virtualenv, native compiled binary, or WebAssembly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginRuntime {
    #[serde(rename = "type")]
    pub runtime_type: String, // "python" | "binary" | "wasm"
    pub entrypoint: String,
    #[serde(default)]
    pub min_python_version: Option<String>,
    #[serde(default)]
    pub dependencies_file: Option<String>,
    #[serde(default)]
    pub healthcheck_flag: Option<String>,
}

/// Hardware GPU acceleration configuration and requirements for the plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginGpuConfig {
    #[serde(default = "default_gpu_support")]
    pub support: String, // "none" | "optional" | "required"
    #[serde(default)]
    pub driver: Option<String>,
}

fn default_gpu_support() -> String {
    "none".to_string()
}

/// Subprocess execution constraints including timeouts and concurrency limits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginExecutionConfig {
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_provides_output_dir")]
    pub provides_output_dir_arg: bool,
    #[serde(default = "default_concurrency")]
    pub concurrency: String, // "sequential" | "parallel"
    #[serde(default)]
    pub gpu: Option<PluginGpuConfig>,
}

fn default_timeout() -> u64 {
    60
}

fn default_provides_output_dir() -> bool {
    true
}

fn default_concurrency() -> String {
    "sequential".to_string()
}

/// Canonical plugin manifest model strictly validated against `plugin.schema.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    #[serde(default, rename = "$schema")]
    pub schema: Option<String>,
    pub manifest_version: u32,
    pub metadata: PluginMetadata,
    pub runtime: PluginRuntime,
    pub parameters: String, // relative path to parameters.json
    pub inputs: String,     // relative path to inputs.json
    pub outputs: String,    // relative path to outputs.json
    #[serde(default)]
    pub execution: Option<PluginExecutionConfig>,
}

// ---------------------------------------------------------------------------
// Inputs Specification (Conforms to schemas/inputs.schema.json)
// ---------------------------------------------------------------------------

/// Input data constraints and metadata requirements validated against `inputs.schema.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputsSpec {
    pub granularity: String,
    pub supported_mime_types: Vec<String>,
    #[serde(default)]
    pub requires_metadata: Vec<String>,
    #[serde(default)]
    pub optional_metadata: Vec<String>,
    #[serde(default)]
    pub supports_aoi: bool,
    #[serde(default)]
    pub aoi_format: Option<String>,
}

// ---------------------------------------------------------------------------
// Plugin State Persistence Models (plugin_state.json)
// ---------------------------------------------------------------------------

/// Origin source classification of an installed plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginSource {
    /// Shipped with core platform (cannot be uninstalled).
    Builtin,
    /// Uploaded as a `.zip` archive by the user.
    User,
}

/// Cached result of the most recent pre-flight dependency health check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedHealthcheck {
    pub status: String,     // "healthy" | "unhealthy"
    pub checked_at: String, // ISO 8601
    #[serde(default)]
    pub message: Option<String>,
}

/// Cumulative execution metrics and last-run timestamp tracked per plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginStats {
    #[serde(default)]
    pub execution_count: u64,
    #[serde(default)]
    pub last_executed_at: Option<String>,
}

/// Operational state record stored per plugin in `plugin_state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginStateRecord {
    pub version: String,
    pub source: PluginSource,
    pub enabled: bool,
    pub installed_at: String,
    #[serde(default)]
    pub last_healthcheck: Option<CachedHealthcheck>,
    #[serde(default)]
    pub custom_parameters: serde_json::Value,
    #[serde(default)]
    pub stats: PluginStats,
}

/// Canonical root state store atomically persisted to `plugin_state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginStateStore {
    pub schema_version: u32,
    pub updated_at: String,
    pub plugins: HashMap<String, PluginStateRecord>,
}

impl Default for PluginStateStore {
    fn default() -> Self {
        Self {
            schema_version: 1,
            updated_at: chrono::Utc::now().to_rfc3339(),
            plugins: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// In-Memory Plugin Bundle & DTOs for IPC and Cross-Module Usage
// ---------------------------------------------------------------------------

/// In-memory bundle aggregating manifest, sub-contracts, and physical installation path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginBundle {
    pub install_path: PathBuf,
    pub manifest: PluginManifest,
    pub inputs_spec: InputsSpec,
    pub parameters_raw: serde_json::Value,
    pub outputs_raw: serde_json::Value,
}

/// Lightweight summary DTO for UI lists, dropdowns, and cards (Modules 1, 3, 11).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginSummaryDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub category: String,
    pub description: String,
    pub tags: Vec<String>,
    pub runtime_type: String,
    pub enabled: bool,
    pub source: String,
    pub health_status: String,
}

/// Target descriptor specifying image characteristics for compatibility filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetDescriptorDto {
    pub granularity: String,
    pub mime_type: String,
    pub has_gps: bool,
    pub has_altitude: bool,
    pub has_aoi: bool,
}

/// Fully resolved plugin bundle DTO delivered to frontend forms and modals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetailsDto {
    pub manifest: PluginManifest,
    pub parameters: serde_json::Value,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
    pub install_path: String,
    pub enabled: bool,
}

/// Diagnostic health status response emitted by plugin entrypoint pre-flight checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatusDto {
    pub status: String,
    pub plugin_id: String,
    pub version: String,
    pub details: Option<serde_json::Value>,
    pub error: Option<String>,
}
