//! # Aerial Analytics Platform
//!
//! Desktop application for processing and analysing RGB aerial imagery
//! captured by standard drones. Built with [Tauri](https://tauri.app) and
//! React, the platform uses a **plugin-based architecture** so analytical
//! capabilities can be added independently without modifying the core system.
//!
//! ---
//!
//! ## Module 1 — Aerial Image & Project Manager
//!
//! Core system for organising aerial imagery, projects, acquisition sessions,
//! and associated metadata (EXIF, XMP DJI).
//!
//! - `models` — shared image metadata types ([`models::ImageMetadata`], [`models::ImageFormat`]).
//!
//! **Status**: Metadata struct defined. EXIF parsing, project management, and
//! flight session management are not yet implemented.
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
//! - `map_controller` — backend controller for map operations.
//!
//! **Status**: Placeholder. Error types defined; commands and types are not yet
//! implemented.
//!
//! ---
//!
//! ## Module 4 — RGB Vegetation Detection Plugin
//!
//! Plugin for identifying vegetation from standard RGB aerial imagery using
//! indices such as ExG, ExR, or VARI. Generates vegetation masks and
//! statistics.
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
//!   detection and temporal results.
//!
//! **Status**: Cross-module contract types and constructors implemented with
//! unit tests. Multi-date management and comparison logic not yet implemented.
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

mod modules;
mod plugin_manager;

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

mod map_controller;
mod plot;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
