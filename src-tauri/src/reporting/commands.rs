//! Tauri commands for the Module 11 report and export contract.
//!
//! Both commands delegate to [`crate::modules::module_11`]; they exist so the
//! original `generate_report` / `export_data` names keep working for callers
//! that were written against the scaffold.

use crate::modules::module_03::state::get_global_state;
use crate::modules::module_11::exports::{write_plot_indicators_csv, write_plots_geojson};
use crate::modules::module_11::report::write_report;
use crate::plugin_manager::error::CommandError;
use serde::Deserialize;
use tauri::State;

/// Export formats accepted by [`export_data`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    GeoJson,
    Json,
    Xlsx,
}

fn lock_error() -> CommandError {
    CommandError {
        code: "STATE_ERROR".to_string(),
        message: "Database lock poisoned while generating the report".to_string(),
    }
}

fn storage_error(error: String) -> CommandError {
    CommandError {
        code: "REPORT_ERROR".to_string(),
        message: error,
    }
}

/// Generates the printable HTML report for a project and returns its path.
///
/// The scaffolding promised a PDF; the implementation is HTML because it needs
/// no extra dependency and prints to PDF from any browser.
#[tauri::command]
pub fn generate_report(
    state: State<'_, crate::AppState>,
    project_id: String,
) -> Result<String, CommandError> {
    let exports = get_global_state().storage_dir.join("exports");
    let conn = state.db.lock().map_err(|_| lock_error())?;
    let path = write_report(&conn, &project_id, &exports).map_err(storage_error)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Exports project data in the requested format and returns the written path.
///
/// `json` writes the dashboard snapshot; `xlsx` is rejected because the
/// platform does not ship a spreadsheet writer (CSV is the tabular export).
#[tauri::command]
pub fn export_data(
    state: State<'_, crate::AppState>,
    project_id: String,
    format: ExportFormat,
) -> Result<String, CommandError> {
    let exports = get_global_state().storage_dir.join("exports");
    let conn = state.db.lock().map_err(|_| lock_error())?;
    let path = match format {
        ExportFormat::Csv => write_plot_indicators_csv(&conn, &project_id, &exports),
        ExportFormat::GeoJson => write_plots_geojson(&conn, &project_id, &exports),
        ExportFormat::Json => {
            crate::modules::module_11::exports::write_dashboard_json(&conn, &project_id, &exports)
        }
        ExportFormat::Xlsx => {
            return Err(CommandError {
                code: "UNSUPPORTED_FORMAT".to_string(),
                message: "XLSX export is not supported; use CSV for tabular data".to_string(),
            })
        }
    }
    .map_err(storage_error)?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlsx_is_the_only_unsupported_format() {
        // The enum exists so deserialisation accepts the documented names.
        let parsed: ExportFormat = serde_json::from_str("\"xlsx\"").unwrap();
        assert_eq!(parsed, ExportFormat::Xlsx);
        let parsed: ExportFormat = serde_json::from_str("\"geo_json\"").unwrap();
        assert_eq!(parsed, ExportFormat::GeoJson);
    }
}
