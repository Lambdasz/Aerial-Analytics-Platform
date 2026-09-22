mod plugin_manager;

use crate::map_controller::sp_measurement::spatial_rs::dummy::get_dummy_spatial_layers;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            plugin_manager::commands::validate_and_create_payload,
            plugin_manager::commands::process_execution_result,
            import_plots,
            get_dummy_spatial_layers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
