//! # Aerial Analytics Platform
//!
//! Desktop application for processing and analysing RGB aerial imagery
//! captured by standard drones. Built with [Tauri](https://tauri.app) and
//! React, the platform uses a **plugin-based architecture** so analytical
//! capabilities can be added independently without modifying the core system.
//!
//! ## Crate Layout
//!
//! | Module | Description |
//! |--------|-------------|
//! | `plugin_manager` | Plugin discovery, lifecycle, execution, and error isolation (Module 2) |
//! | `models` | Shared aerial image metadata types extracted from EXIF / XMP DJI (Module 1) |
//! | `plot` | GeoJSON plot import and validation (Module 9) |
//! | `plot_api` | Cross-module plot analytics API — plot definition, per-plot analysis, statistics (Module 9) |
//! | `modules` | Analytical module implementations (Module 10+) |
//! | `map_controller` | Interactive geospatial map explorer backend (Module 3) |
//!
//! ## Tauri Commands
//!
//! The following commands are registered in [`run`] and callable from the
//! frontend via `invoke()`:
//!
//! - `greet` — hello-world demo command.
//! - `import_plots` — parse a GeoJSON file into plot boundaries.
//! - `validate_and_create_payload` — pre-flight check and write `payload.json`.
//! - `process_execution_result` — parse `result.json` and record run history.
//! - `start_plugin_job` — launch an async plugin subprocess.
//! - `abort_plugin_job` — kill a running plugin subprocess.
//! - `get_job_status` — poll the lifecycle state of a job.
//! - `get_job_result` — retrieve the full result of a completed job.

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
