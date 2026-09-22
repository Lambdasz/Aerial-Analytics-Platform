# Domain 1: Discovery, Registry & Health Check (Rust Core)

> **Module 2: Plugin Architecture & Extension Manager**  
> **Parent Guide**: [README.md](README.md)

---

## 1. Domain Purpose & Overview

Domain 1 is responsible for discovering plugins from the filesystem, validating them against standard JSON schemas, indexing valid plugins in memory, querying plugin compatibility for other modules, and performing pre-flight environment/dependency health checks.

---

## 2. Function Specification Matrix

| Function Name                     | Scope             | Input Parameters                                          | Output Type                                                   | Purity / Category            | Pre-condition                                     | Post-condition                                                                            | Description                                                                 |
| :-------------------------------- | :---------------- | :-------------------------------------------------------- | :------------------------------------------------------------ | :--------------------------- | :------------------------------------------------ | :---------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------- |
| **`discover_plugins`**            | `pub(crate)`      | `plugins_dir: &Path`                                      | `Result<Vec<PluginBundle>, PluginError>`                      | **I/O (Read FS)**            | `plugins_dir` exists.                             | Scans subdirectories; loads manifests & subcontracts. Corrupt plugins logged and skipped. | Scans `plugins/` directory and builds in-memory plugin registry.            |
| **`parse_and_validate_manifest`** | `pub(crate)`      | `raw_json: &str`                                          | `Result<PluginManifest, PluginError>`                         | **Pure Function**            | Valid UTF-8 JSON string.                          | Deterministic parse; zero mutation, zero side effects.                                    | Validates syntax and types strictly against `plugin.schema.json`.           |
| **`validate_subcontracts`**       | `pub(crate)`      | `dir: &Path, manifest: &PluginManifest`                   | `Result<(ParametersDef, InputsDef, OutputsDef), PluginError>` | **I/O + Pure Validation**    | Files referenced in manifest exist.               | All 3 subcontracts are parsed and schema-validated.                                       | Ingests and validates `parameters.json`, `inputs.json`, and `outputs.json`. |
| **`get_installed_plugins`**       | `pub (IPC Tauri)` | `state: State<'_, AppState>`                              | `Result<Vec<PluginSummaryDto>, String>`                       | **I/O (Read Memory)**        | Registry initialized in `AppState`.               | Returns immutable snapshot of installed plugins with active `enabled` states.             | Tauri command for UI cards and action menus (Modules 1, 3, 11).             |
| **`get_plugin_details`**          | `pub (IPC Tauri)` | `plugin_id: String, state: State<'_, AppState>`           | `Result<PluginDetailsDto, String>`                            | **I/O (Read Memory)**        | `plugin_id` exists in registry.                   | Returns fully resolved bundle (Manifest + Parameters + Inputs + Outputs).                 | Delivers complete schema definitions for dynamic forms and layer hints.     |
| **`query_compatible_plugins`**    | `pub (IPC Tauri)` | `target: TargetDescriptorDto, state: State<'_, AppState>` | `Result<Vec<PluginSummaryDto>, String>`                       | **Pure Logic (over Memory)** | Target defines MIME, granularity, metadata flags. | Filters registry using pure predicate `is_compatible(inputs_spec, target)`.               | Returns only plugins capable of processing the given image or AOI.          |
| **`check_plugin_health`**         | `pub (IPC Tauri)` | `plugin_id: String, state: State<'_, AppState>`           | `Result<HealthStatusDto, String>`                             | **I/O (Subprocess Spawn)**   | Plugin has valid entrypoint.                      | Runs `<entrypoint> --healthcheck` with 10s timeout; parses stdout/stderr JSON.            | Verifies runtime dependencies, Python environment, or binary liveness.      |

---

## 3. Data Models & Rust DTOs

```rust
use serde::{Deserialize, Serialize};

/// Lightweight summary for UI lists and dropdowns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginSummaryDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub category: String,
    pub description: String,
    pub tags: Vec<String>,
    pub runtime_type: String, // "python" | "binary" | "wasm"
    pub enabled: bool,
}

/// Target descriptor for compatibility filtering (Modules 1, 3, 10 -> Module 2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetDescriptorDto {
    pub granularity: String, // "single_image", "image_pair", "batch"
    pub mime_type: String,   // "image/jpeg", "image/png", "image/tiff"
    pub has_gps: bool,
    pub has_altitude: bool,
    pub has_aoi: bool,
}

/// Full resolved bundle returned to Frontend and Consumer Modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetailsDto {
    pub manifest: PluginManifest,
    pub parameters: serde_json::Value, // parameters.json content
    pub inputs: serde_json::Value,     // inputs.json content
    pub outputs: serde_json::Value,   // outputs.json content
    pub install_path: String,
    pub enabled: bool,
}

/// Healthcheck response emitted by plugin entrypoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatusDto {
    pub status: String, // "healthy" | "unhealthy"
    pub plugin_id: String,
    pub version: String,
    pub details: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Domain Error Enum (Unified, zero panics)
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum PluginError {
    #[error("Plugin '{0}' not found in registry")]
    NotFound(String),

    #[error("Invalid manifest at '{path}': {message}")]
    ManifestInvalid { path: String, message: String },

    #[error("Sub-contract missing or invalid at '{path}': {message}")]
    SubcontractInvalid { path: String, message: String },

    #[error("Healthcheck execution timed out after {0} seconds")]
    HealthcheckTimeout(u64),

    #[error("Healthcheck process failed: {0}")]
    HealthcheckFailed(String),

    #[error("I/O error: {0}")]
    Io(String),
}
```

---

## 4. Functional Programming Invariants

### 4.1 Pure Compatibility Filter Logic

When Module 1 or Module 3 queries for available plugins, filtering is computed with zero side-effects via a pure predicate function:

```rust
pub fn is_compatible(spec: &InputsSpec, target: &TargetDescriptorDto) -> bool {
    let mime_match = spec.supported_mime_types.iter().any(|m| m == &target.mime_type);
    let granularity_match = spec.granularity == target.granularity;
    let aoi_valid = !target.has_aoi || spec.supports_aoi;
    let gps_valid = !spec.requires_metadata.contains(&"gps".to_string()) || target.has_gps;

    mime_match && granularity_match && aoi_valid && gps_valid
}
```

### 4.2 Fault-Tolerant Directory Discovery

An unreadable directory or an invalid manifest in one plugin folder **MUST NOT** abort discovery of other plugins:

```rust
pub fn scan_plugins(plugins_dir: &Path) -> Result<Vec<PluginBundle>, PluginError> {
    let entries = std::fs::read_dir(plugins_dir).map_err(|e| PluginError::Io(e.to_string()))?;

    let valid_plugins: Vec<PluginBundle> = entries
        .filter_map(|res| res.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| match load_plugin_bundle(&entry.path()) {
            Ok(bundle) => Some(bundle),
            Err(err) => {
                eprintln!("Warning: skipping corrupted plugin at {:?}: {}", entry.path(), err);
                None
            }
        })
        .collect();

    Ok(valid_plugins)
}
```

---

## 5. Healthcheck Execution Protocol

Before executing heavy jobs or displaying active badges, the supervisor runs:

```bash
<plugin_entrypoint> --healthcheck
```

- **Timeout**: Wrapped in `tokio::time::timeout(Duration::from_secs(10), ...)`.
- **Parsing**:
  - Code `0`: Parse stdout JSON (`{"status": "healthy", ...}`).
  - Non-zero: Parse stderr JSON or format exit code into diagnostic error.
- Never panics or blocks the Tauri async runtime.
