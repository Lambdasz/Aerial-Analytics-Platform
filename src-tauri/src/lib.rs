//! # Aerial Analytics Platform
//!
//! Desktop application for processing and analysing RGB aerial imagery
//! captured by standard drones. Built with [Tauri](https://tauri.app) and
//! Blueprint.js, the platform uses a **plugin-based architecture** so analytical
//! capabilities can be added independently without modifying the core system.
//!
//! ---
//!
//! ## Development Priorities
//!
//! Modules are prioritised by their role in the upcoming presentation and
//! the demo itself:
//!
//! | Priority | Modules | Rationale |
//! |----------|---------|-----------|
//! | **Critical** | 1, 2, 3 | The application does not run without these. |
//! | **High** | 4 + 5 + 6 (single plugin), 7, 8 | Proves the plugin architecture — without them the demo is just a map. Implementation details still subject to paper review. |
//! | **Medium** | 9, 10 | Simplify plugin usage and analysis; the app works without them, but with more friction. |
//! | **Low** | 11 | Only fully functional once all other modules are complete. |
//!
//! ---
//!
//! ## Module 1 — Aerial Image & Project Manager
//!
//! Core system for organising aerial imagery, projects, acquisition sessions,
//! and associated metadata (EXIF, XMP DJI).
//!
//! - `models` — shared image metadata types ([`models::ImageMetadata`], [`models::ImageFormat`]).
//! - `modules::module_01` — image metadata extraction API
//!   (`modules::module_01::extract_metadata`).
//! - `commands::session` — flight session command stubs.
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `create_session` | Create a new flight session |
//! | `get_session` | Fetch a session by id |
//! | `get_sessions_by_project` | List sessions for a project |
//! | `update_session_name` | Rename a session |
//! | `update_session_status` | Update a session's status |
//! | `assign_image_to_session` | Associate an image with a session |
//! | `recalculate_session_date_range` | Recompute a session's date range from its images |
//! | `delete_session` | Delete a session |
//!
//! ---
//!
//! ## Module 2 — Plugin System & Extension Manager
//!
//! Plugin architecture for installing, registering, configuring, executing,
//! enabling, disabling, and removing analytical plugins without modifying the
//! core platform.
//!
//! - `plugin_manager` — discovery, lifecycle, execution, and error isolation.
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `validate_and_create_payload` | Pre-flight check and write `payload.json` |
//! | `process_execution_result` | Parse `result.json` and record run history |
//! | `start_plugin_job` | Launch an async plugin subprocess |
//! | `abort_plugin_job` | Kill a running plugin subprocess |
//! | `get_job_status` | Poll the lifecycle state of a job |
//! | `get_job_result` | Retrieve the full result of a completed job |
//!
//! **Status**: Models, error types, payload assembly, and result handling are
//! implemented. Registry discovery, lifecycle management, and async execution
//! are designed but contain `todo!()` stubs.
//!
//! ---
//!
//! ## Module 3 — Aerial Image Map Explorer
//!
//! Interactive geospatial environment for exploring aerial images, drawing
//! areas of interest, managing layers, and performing spatial measurements.
//!
//! - `map_controller` — backend controller for map operations; see the
//!   [`map_controller`] module docs for
//!   responsibilities, key functions, data contracts, and the full I/O
//!   reference.
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `get_project_image_markers` | Generate pseudo-random drone image markers for the map explorer |
//! | `validate_and_create_payload` | Pre-flight check and write `payload.json` |
//! | `process_execution_result` | Parse `result.json` and record run history |
//! | `get_dummy_spatial_layers` | Load dummy spatial layers (dev/testing only) |
//!
//! **Status**: Implemented. Map core, image markers, geometry/AOI, layer
//! management, annotation, and spatial result integration are complete. Dummy
//! spatial layers available for frontend development in parallel with
//! Modul 4/7/8.
//!
//! ---
//!
//! ## Module 4 — RGB Vegetation Detection Plugin
//!
//! Plugin for identifying vegetation from standard RGB aerial imagery using
//! indices such as ExG, ExR, or VARI. Generates vegetation masks and
//! statistics.
//!
//! ### Plugin Contract
//!
//! Uses the Module 2 plugin template architecture (`plugins/template/`).
//! No Rust backend code — the platform side is entirely Module 2.
//!
//! | Item | Value |
//! |------|-------|
//! | Architecture | Module 2 plugin template (`plugins/template/`) |
//! | Interface | `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` |
//! | Entrypoint | `plugins/rgb-vegetation-detection/main.py` — `def main() -> None` (Python) |
//! | CLI | `--healthcheck` \| `--input <payload.json> --output <result.json>` |
//! | Execution | isolated subprocess via Module 2 supervisor: `payload.json` in → `result.json` out |
//! | Progress | stdout `PROGRESS: {json}` lines parsed by Module 2 |
//!
//! Entrypoint (`main.py`, Python):
//!
//! ```python
//! def main() -> None:
//!     # --healthcheck
//!     # --input <execution_payload.json> --output <execution_result.json>
//! ```
//!
//! **Status**: Python plugin skeleton at `plugins/rgb-vegetation-detection/`
//! (simulated detection). No Rust backend code.
//!
//! ---
//!
//! ## Module 5 — Vegetation Coverage Analytics
//!
//! Quantifies vegetation coverage using outputs from Module 4. Calculates
//! coverage percentages, classifies areas (low / medium / high), and supports
//! plot-based and multi-plot comparison.
//!
//! **Status**: Not yet implemented.
//!
//! ---
//!
//! ## Module 6 — RGB Vegetation Condition Analysis
//!
//! Analyses visible vegetation conditions using RGB colour information:
//! greenness intensity, yellowing, browning, and condition zonation.
//!
//! **Status**: Not yet implemented.
//!
//! ---
//!
//! ## Module 7 — Tree Detection & Counting Plugin
//!
//! Computer-vision plugin for detecting and counting individual trees or tree
//! crowns from aerial RGB imagery. Returns spatial locations, confidence
//! values, and density metrics.
//!
//! ### Plugin Contract
//!
//! Uses the Module 2 plugin template architecture (`plugins/template/`) —
//! identical contract, runtime type is `binary` instead of `python`.
//!
//! | Item | Value |
//! |------|-------|
//! | Architecture | Module 2 plugin template (`plugins/template/`) |
//! | Interface | `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` |
//! | Entrypoint | `plugins/mock_rust/src/main.rs` — `fn main()` (Rust binary) |
//! | CLI | `--healthcheck` \| `--input <payload.json> --output <result.json>` |
//! | Execution | isolated subprocess via Module 2 supervisor: `payload.json` in → `result.json` out |
//! | Progress | stdout `PROGRESS: {json}` lines parsed by Module 2 |
//! | Runtime | `binary` (`manifest.json` → `runtime.entrypoint = "bin/mock_rust"`) |
//!
//! Entrypoint (`src/main.rs`, Rust binary):
//!
//! ```rust,ignore
//! fn main() {
//!     // --healthcheck
//!     // --input <payload.json> --output <result.json>
//! }
//! ```
//!
//! **Status**: Mock Rust plugin at `plugins/mock_rust/` (tree-canopy-density).
//! No real CV model integrated.
//!
//! ---
//!
//! ## Module 8 — RGB Land-Cover Classification Plugin
//!
//! Plugin for classifying visible land-cover types (vegetation, bare soil,
//! water, built areas) from RGB aerial imagery. Produces classified masks,
//! per-class statistics, and boundary vectors.
//!
//! ### Plugin Contract
//!
//! Uses the Module 2 plugin template architecture (`plugins/template/`).
//! No Rust backend code — the platform side is entirely Module 2.
//!
//! | Item | Value |
//! |------|-------|
//! | Architecture | Module 2 plugin template (`plugins/template/`) |
//! | Interface | `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` |
//! | Entrypoint | `plugins/rgb-landcover-classification/main.py` — `def main() -> None` (Python) |
//! | CLI | `--healthcheck` \| `--input <payload.json> --output <result.json>` |
//! | Execution | isolated subprocess via Module 2 supervisor: `payload.json` in → `result.json` out |
//! | Progress | stdout `PROGRESS: {json}` lines parsed by Module 2 |
//!
//! Entrypoint (`main.py`, Python):
//!
//! ```python
//! def main() -> None:
//!     # --healthcheck
//!     # --input <payload.json> --output <result.json>
//! ```
//!
//! **Status**: Python plugin at `plugins/rgb-landcover-classification/` with
//! Random Forest classifier. Model weights not yet trained.
//!
//! ---
//!
//! ## Module 9 — Area & Plot Analytics
//!
//! Spatial analytical system for defining plots and summarising analytical
//! results within them. Aggregates per-plot statistics for the dashboard.
//!
//! - `plot` — GeoJSON plot import and validation (`plot::parse_plots`).
//! - `plot_api` — cross-module API for plot definition, per-plot analysis,
//!   and statistics ([`plot_api::define_plot`], [`plot_api::analyze_plot`],
//!   [`plot_api::get_plot_statistics`]).
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `import_plots` | Parse a GeoJSON file into plot boundaries |
//!
//! **Status**: GeoJSON parser fully implemented with unit tests. Plot API
//! functions are `todo!()` stubs.
//!
//! ---
//!
//! ## Module 10 — Simple Temporal Change Analysis
//!
//! Compares RGB aerial observations captured at different times. Calculates
//! changes in vegetation coverage, land-cover transitions, and tree-count
//! deltas between observation periods.
//!
//! - `modules::module_10` — cross-module data contracts for change area
//!   detection and temporal results (`modules::module_10::functions`).
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `compare_sessions` | Run a comparison between two sessions' analysis results (vegetation coverage, land-cover, tree count) and produce a temporal result |
//! | `get_change_summary` | Return the latest computed temporal summary for a project, for Module 11 to consume |
//!
//! **Status**: Cross-module contract types (`ChangeArea`, temporal result
//! structs) and constructors implemented with unit tests. No command exists
//! yet to actually run a comparison or expose a result — `compare_sessions`
//! and `get_change_summary` are `todo!()` stubs so Module 10 has a minimal
//! contract, matching the Module 9 (`plot_api`) pattern.
//!
//! ---
//!
//! ## Module 11 — Aerial Analytics Dashboard & Reporting
//!
//! Integrated interface for summarising and communicating results: project
//! dashboards, plot comparison, temporal summaries, chart generation, report
//! export, and GIS data export.
//!
//! **Status**: Not yet implemented.

mod commands;
pub mod map_controller;
mod modules;
mod plot;
mod plugin_manager;

use crate::map_controller::sp_measurement::spatial_rs::dummy::get_dummy_spatial_layers;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use plugin_manager::executor::ActiveJobTracker;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state injected into every Tauri command via `.manage()`.
pub struct AppState {
    /// Active and recently completed plugin jobs (Role 1 — executor.rs).
    pub jobs: Arc<RwLock<ActiveJobTracker>>,
}

// Learn more about Tauri commands at
// https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn import_plots(path: String) -> Result<plot::ImportResult, String> {
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file '{path}': {e}"))?;

    plot::parse_plots(&text)
}

pub mod models;

pub mod plot_api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // Role 3 — Data Bridge
            plugin_manager::commands::validate_and_create_payload,
            plugin_manager::commands::process_execution_result,
            // Role 1 — Subprocess Supervisor & Isolation Engineer
            plugin_manager::executor::start_plugin_job,
            plugin_manager::executor::abort_plugin_job,
            plugin_manager::executor::get_job_status,
            plugin_manager::executor::get_job_result,
            // Plot import
            import_plots,
            get_dummy_spatial_layers,
            // Flight session
            commands::session::create_session,
            commands::session::get_session,
            commands::session::get_sessions_by_project,
            commands::session::update_session_name,
            commands::session::update_session_status,
            commands::session::assign_image_to_session,
            commands::session::recalculate_session_date_range,
            commands::session::delete_session,
            // Temporal change analysis (Module 10)
            modules::module_10::commands::compare_sessions,
            modules::module_10::commands::get_change_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
