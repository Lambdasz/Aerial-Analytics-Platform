//! Tauri commands for the Module 11 dashboard and exports.

use crate::modules::module_03::state::get_global_state;
use crate::modules::module_11::dashboard::{
    build_project_dashboard, list_plugin_results, PluginResultRow, ProjectDashboard,
};
use crate::modules::module_11::exports::{
    write_dashboard_json, write_plot_indicators_csv, write_plots_geojson,
};
use crate::modules::module_11::report::write_report;
use crate::plugin_manager::error::CommandError;
use tauri::State;

fn lock_error() -> CommandError {
    CommandError {
        code: "STATE_ERROR".to_string(),
        message: "Database lock poisoned while reading dashboard data".to_string(),
    }
}

fn storage_error(error: String) -> CommandError {
    CommandError {
        code: "EXPORT_ERROR".to_string(),
        message: error,
    }
}

/// Project-level summary of stored sessions, images, plots, plugin results and
/// the latest temporal comparison.
#[tauri::command]
pub fn get_project_dashboard(
    state: State<'_, crate::AppState>,
    project_id: String,
) -> Result<ProjectDashboard, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    build_project_dashboard(&conn, &project_id).map_err(|error| CommandError {
        code: "DASHBOARD_ERROR".to_string(),
        message: error.to_string(),
    })
}

/// Flattened plugin metric rows for the interactive result explorer.
#[tauri::command]
pub fn list_dashboard_plugin_results(
    state: State<'_, crate::AppState>,
    project_id: String,
    limit: Option<usize>,
) -> Result<Vec<PluginResultRow>, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    list_plugin_results(&conn, &project_id, limit.unwrap_or(200)).map_err(|error| CommandError {
        code: "DASHBOARD_ERROR".to_string(),
        message: error.to_string(),
    })
}

fn export_dir() -> std::path::PathBuf {
    get_global_state().storage_dir.join("exports")
}

/// Exports plot indicators as CSV and returns the written path.
#[tauri::command]
pub fn export_dashboard_plot_csv(
    state: State<'_, crate::AppState>,
    project_id: String,
) -> Result<String, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    let path =
        write_plot_indicators_csv(&conn, &project_id, &export_dir()).map_err(storage_error)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Exports plots as GeoJSON and returns the written path.
#[tauri::command]
pub fn export_dashboard_plots_geojson(
    state: State<'_, crate::AppState>,
    project_id: String,
) -> Result<String, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    let path = write_plots_geojson(&conn, &project_id, &export_dir()).map_err(storage_error)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Exports the dashboard snapshot as JSON and returns the written path.
#[tauri::command]
pub fn export_dashboard_json(
    state: State<'_, crate::AppState>,
    project_id: String,
) -> Result<String, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    let path = write_dashboard_json(&conn, &project_id, &export_dir()).map_err(storage_error)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Generates the printable HTML report for a project and returns its path.
#[tauri::command]
pub fn generate_dashboard_report(
    state: State<'_, crate::AppState>,
    project_id: String,
) -> Result<String, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    let path = write_report(&conn, &project_id, &export_dir()).map_err(storage_error)?;
    Ok(path.to_string_lossy().into_owned())
}
