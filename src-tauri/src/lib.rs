pub mod plugin_manager;

use plugin_manager::executor::ActiveJobTracker;
use plugin_manager::registry::PluginRegistry;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state injected into every Tauri command via `.manage()`.
pub struct AppState {
    /// Discovered and installed analytical plugins (Role 2 — registry.rs).
    pub plugins: Arc<RwLock<PluginRegistry>>,
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

/// Discovers and registers plugins from a specified directory.
pub fn init_plugin_registry_from_dir(dir: &std::path::Path) -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    if let Ok(bundles) = plugin_manager::registry::discover_plugins(dir) {
        for bundle in bundles {
            registry.register(bundle);
        }
    }
    registry
}

/// Bootstraps discovery across candidate directory locations at application startup.
pub fn init_plugin_registry() -> PluginRegistry {
    let candidate_dirs = [
        std::path::PathBuf::from("plugins"),
        std::path::PathBuf::from("../plugins"),
        std::path::PathBuf::from("../../plugins"),
    ];

    for dir in &candidate_dirs {
        if dir.is_dir() {
            return init_plugin_registry_from_dir(dir);
        }
    }

    PluginRegistry::new()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let registry = init_plugin_registry();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            plugins: Arc::new(RwLock::new(registry)),
            jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // Role 1 — Subprocess Supervisor & Isolation Engineer
            plugin_manager::executor::start_plugin_job,
            plugin_manager::executor::abort_plugin_job,
            plugin_manager::executor::get_job_status,
            plugin_manager::executor::get_job_result,
            // Role 2 — Plugin Lifecycle & Package Manager (Discovery & Registry)
            plugin_manager::registry::get_installed_plugins,
            plugin_manager::registry::get_plugin_details,
            plugin_manager::registry::query_compatible_plugins,
            plugin_manager::registry::check_plugin_health,
            plugin_manager::lifecycle::set_plugin_status,
            // Plot import
            import_plots,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_plugin_registry_from_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path().join("my-test-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        std::fs::write(
            plugin_dir.join("manifest.json"),
            r#"{
                "manifest_version": 1,
                "metadata": {"id": "my-test-plugin", "name": "My Plugin", "version": "1.0.0", "description": "Desc"},
                "runtime": {"type": "python", "entrypoint": "main.py"},
                "parameters": "parameters.json",
                "inputs": "inputs.json",
                "outputs": "outputs.json"
            }"#,
        ).unwrap();
        std::fs::write(plugin_dir.join("parameters.json"), r#"{"parameters": []}"#).unwrap();
        std::fs::write(
            plugin_dir.join("inputs.json"),
            r#"{"granularity": "single_image", "supported_mime_types": ["image/jpeg"]}"#,
        )
        .unwrap();
        std::fs::write(plugin_dir.join("outputs.json"), r#"{}"#).unwrap();

        let registry = init_plugin_registry_from_dir(temp_dir.path());
        assert_eq!(registry.get_summaries().len(), 1);
        assert_eq!(registry.get_summaries()[0].id, "my-test-plugin");
    }

    #[test]
    fn test_init_plugin_registry_discovers_builtin_plugins() {
        let registry = init_plugin_registry();
        // Repository has plugins/ directory with real plugins
        assert!(
            registry.plugins.contains_key("rgb-vegetation-detection"),
            "Built-in plugins should be discovered on startup"
        );
    }
}
