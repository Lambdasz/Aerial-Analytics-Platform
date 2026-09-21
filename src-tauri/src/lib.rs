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
        .map_err(|e| format!("Gagal membaca berkas '{path}': {e}"))?;

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
