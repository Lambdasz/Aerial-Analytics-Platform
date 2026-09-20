# Domain 2: Lifecycle & State Persistence (Rust Core)
> **Module 2: Plugin Architecture & Extension Manager**  
> **Parent Guide**: [README.md](README.md)

---

## 1. Domain Purpose & Overview

Domain 2 manages the physical lifecycle of plugins (safe extraction of `.zip` packages, uninstallation) and the atomic persistence of mutable runtime states (enabled/disabled toggles, user-configured parameter overrides, and execution telemetry).

---

## 2. Function Specification Matrix

| Function Name | Scope | Input Parameters | Output Type | Purity / Category | Pre-condition | Post-condition | Description |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`install_plugin`** | `pub (IPC Tauri)` | `archive_path: String, state: State<'_, AppState>` | `Result<PluginSummaryDto, String>` | **I/O (Extract, Validate, Atomic Move)** | `archive_path` points to a valid `.zip` file. | Safe extraction into `plugins/<id>/`; validated against schemas; registered in state. Corrupt archives leave zero disk remnants. | Installs a new plugin from a `.zip` file with Zip-Slip protection and schema verification. |
| **`remove_plugin`** | `pub (IPC Tauri)` | `plugin_id: String, state: State<'_, AppState>` | `Result<(), String>` | **I/O (Delete FS & Update State)** | Plugin exists; plugin source is `user` (not `builtin`); plugin is not running. | Plugin directory deleted; entry purged from `plugin_state.json`. | Permanently uninstalls an existing plugin and cleans up its stored configuration. |
| **`load_plugin_state`** | `pub(crate)` | `state_path: &Path` | `Result<PluginStateStore, PluginError>` | **I/O (Read FS)** | `state_path` is accessible (returns empty default if file missing). | Returns strongly-typed `PluginStateStore` loaded into memory. | Reads and parses `plugin_state.json` from OS AppData directory on startup. |
| **`save_plugin_state`** | `pub(crate)` | `state_path: &Path, state: &PluginStateStore` | `Result<(), PluginError>` | **I/O (Atomic Write FS)** | Parent directory exists and is writable. | Serializes state to `.tmp` file and atomically renames to `plugin_state.json`. | Safely persists current plugin states and custom parameters to disk. |
| **`set_plugin_status`** | `pub (IPC Tauri)` | `plugin_id: String, enabled: bool, state: State<'_, AppState>` | `Result<(), String>` | **I/O (Update Memory & Write FS)** | `plugin_id` is registered in memory. | In-memory state updated; changes flushed to `plugin_state.json`. | Enables or disables a plugin for UI selection and job execution. |
| **`update_plugin_config`**| `pub (IPC Tauri)` | `plugin_id: String, custom_params: Value, state: State<'_, AppState>` | `Result<(), String>` | **Pure Validation + I/O (Write FS)** | `plugin_id` exists; parameters match `parameters.json` constraints. | Validated parameters stored in state and written to disk. | Saves user-configured default parameter overrides for a specific plugin. |

---

## 3. Canonical State Schema & Rust Models (`plugin_state.json`)

To prevent data loss and support rich metrics without crawling disk logs, `plugin_state.json` tracks operational state:

```json
{
  "schema_version": 1,
  "updated_at": "2026-09-15T15:30:00Z",
  "plugins": {
    "rgb-vegetation-exg": {
      "version": "1.0.0",
      "source": "builtin",
      "enabled": true,
      "installed_at": "2026-09-15T14:32:00Z",
      "last_healthcheck": {
        "status": "healthy",
        "checked_at": "2026-09-15T14:35:00Z",
        "message": null
      },
      "custom_parameters": {
        "index_type": "ExG",
        "threshold": 0.35,
        "export_mask": true
      },
      "stats": {
        "execution_count": 8,
        "last_executed_at": "2026-09-15T15:20:00Z"
      }
    }
  }
}
```

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginSource {
    Builtin, // Shipped with core platform (cannot be uninstalled)
    User,    // Uploaded as .zip by user
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHealthcheck {
    pub status: String, // "healthy" | "unhealthy"
    pub checked_at: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginStats {
    pub execution_count: u64,
    pub last_executed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateStore {
    pub schema_version: u32,
    pub updated_at: String,
    pub plugins: HashMap<String, PluginStateRecord>,
}
```

---

## 4. Security & Safety Protocols

### 4.1 Zip-Slip Path Traversal Defense (`installer.rs`)
When decompressing user-provided archives, all entry names are checked to ensure they cannot escape the target directory:

```rust
pub fn verify_and_extract_entry(
    entry: &mut zip::read::ZipFile,
    dest_dir: &Path,
) -> Result<PathBuf, PluginError> {
    // 1. Enforce enclosed name (rejects '../', absolute paths, and drive letters)
    let enclosed = entry
        .enclosed_name()
        .ok_or_else(|| PluginError::SecurityViolation(format!("Zip-Slip attempt: {}", entry.name())))?;

    let target_path = dest_dir.join(enclosed);

    // 2. Verify target is within destination sandbox
    if target_path.starts_with(dest_dir) {
        Ok(target_path)
    } else {
        Err(PluginError::SecurityViolation(format!("Path escapes sandbox: {}", entry.name())))
    }
}
```

### 4.2 Staging & Atomic Rollback Protocol
1. **Extract**: ZIP archive is unpacked into a temporary folder (`tempfile::tempdir_in(plugins_dir)?`).
2. **Validate**:
   - Manifest is parsed and validated against `plugin.schema.json`.
   - Subcontracts (`parameters.json`, `inputs.json`, `outputs.json`) are validated against their schemas.
   - Verified that `metadata.id` does not overwrite a `builtin` core plugin.
3. **Commit**: Folder is renamed to `plugins/<id>`.
4. **Rollback**: If validation fails at any point, the `TempDir` handle is dropped, automatically wiping all temporary files from disk.

### 4.3 Atomic State File Persistence (`save_plugin_state`)
To protect `plugin_state.json` from corruption during power cuts or crashes:

```rust
pub fn atomic_save_state(state_path: &Path, state: &PluginStateStore) -> Result<(), PluginError> {
    let parent = state_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let temp_file_path = state_path.with_extension("tmp");
    let serialized = serde_json::to_string_pretty(state)?;

    let mut file = std::fs::File::create(&temp_file_path)?;
    file.write_all(serialized.as_bytes())?;
    file.sync_all()?; // Flush buffer to physical storage

    std::fs::rename(&temp_file_path, state_path)?; // Atomic filesystem replacement
    Ok(())
}
```
