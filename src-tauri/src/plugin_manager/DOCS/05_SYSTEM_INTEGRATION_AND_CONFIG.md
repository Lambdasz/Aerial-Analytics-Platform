# System Integration & Configuration
> **Module 2: Plugin Architecture & Extension Manager**  
> **Parent Guide**: [README.md](README.md)

---

## 1. Cargo Dependencies (`src-tauri/Cargo.toml`)

To implement Module 2 as designed, the following dependencies must be added to `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Asynchronous child process management & timeout guards
tokio = { version = "1", features = ["process", "time", "sync", "rt", "fs", "macros"] }

# Strongly typed, idiomatic domain errors
thiserror = "2"

# Safe ZIP archive extraction with Zip-Slip defense
zip = { version = "2", default-features = false, features = ["deflate"] }

# Unique execution ID generation (e.g. exec_20260916_uuid)
uuid = { version = "1", features = ["v4"] }

# Directory traversal for plugin discovery
walkdir = "2"

# Sandboxed temporary workspaces and staging directories
tempfile = "3"

# ISO 8601 timestamps for state persistence and telemetry
chrono = "0.4"
```

---

## 2. Tauri 2 Capability Manifest (`src-tauri/capabilities/default.json`)

In Tauri 2's permission model, all commands exposed by Module 2 must be explicitly authorized for the `main` window:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Permissions for main application window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "allow-get-installed-plugins",
    "allow-get-plugin-details",
    "allow-query-compatible-plugins",
    "allow-check-plugin-health",
    "allow-install-plugin",
    "allow-remove-plugin",
    "allow-set-plugin-status",
    "allow-update-plugin-config",
    "allow-start-plugin-job",
    "allow-abort-plugin-job",
    "allow-get-job-status",
    "allow-get-job-result"
  ]
}
```

---

## 3. Tauri Managed State & Command Registration (`src-tauri/src/lib.rs`)

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::plugins::registry::PluginRegistry;
use crate::plugins::executor::ActiveJobTracker;

pub struct AppState {
    pub plugins: Arc<RwLock<PluginRegistry>>,
    pub jobs: Arc<RwLock<ActiveJobTracker>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            plugins: Arc::new(RwLock::new(PluginRegistry::new())),
            jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
        })
        .invoke_handler(tauri::generate_handler![
            crate::plugins::get_installed_plugins,
            crate::plugins::get_plugin_details,
            crate::plugins::query_compatible_plugins,
            crate::plugins::check_plugin_health,
            crate::plugins::install_plugin,
            crate::plugins::remove_plugin,
            crate::plugins::set_plugin_status,
            crate::plugins::update_plugin_config,
            crate::plugins::start_plugin_job,
            crate::plugins::abort_plugin_job,
            crate::plugins::get_job_status,
            crate::plugins::get_job_result,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## 4. Quality Assurance & Pre-commit Gates

Before opening a pull request or committing code, the `.husky/pre-commit` hook enforces four mandatory gates in sequence. All four **MUST** pass:

```bash
# 1. Frontend TypeScript & ESLint (flat config)
npm run lint

# 2. Rust Clippy (Zero warnings allowed: -D warnings)
npm run lint:rust

# 3. Prettier Formatting Check
npm run format:check

# 4. Rust Formatting Check
npm run format:rust:check
```

### Automatic Formatting & Fixes:
```bash
npm run lint:fix
npm run format
npm run format:rust
```

---

## 5. Git Commit Conventions (Conventional Commits)

Commit messages are validated against Conventional Commits (`.husky/commit-msg`):

```text
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

* **Allowed types**: `feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `test`, `style`, `ci`, `build`.
* **Examples**:
  - `feat(plugins): implement zip-slip safe archive extraction`
  - `fix(executor): prevent zombie processes by killing child on timeout`
  - `docs(plugins): add dynamic parameter form specification`
