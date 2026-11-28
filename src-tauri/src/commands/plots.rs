//! Tauri commands for Module 9 plot definition and listing.
//!
//! Thin wrappers: they take `AppState`'s database connection, delegate to
//! `modules::module_09`, and map errors to `CommandError`. Follows the
//! functional style rules NFR-9.1.6–9.1.7 (`src/modules/module-09/REQUIREMENTS.md`).

use crate::modules::module_03::sp_result::types::LayerPayload;
use crate::modules::module_09::indicators::{
    list_indicators, normalise_indicators, save_indicators, IndicatorRecord,
};
pub use crate::modules::module_09::layer::PlotImportOutcome;
use crate::modules::module_09::layer::{import_candidates, import_outcome, plot_layer_payload};
use crate::modules::module_09::plots::{
    delete_plot as delete_plot_row, get_plot, insert_plot, list_plots as list_plot_rows,
    polygon_from_geometry, rename_plot as rename_plot_row, PlotError, PlotRecord,
};
use crate::plugin_manager::error::CommandError;
use serde_json::Value;
use tauri::State;

fn lock_error() -> CommandError {
    CommandError {
        code: "STATE_ERROR".to_string(),
        message: "Database lock poisoned while accessing plots".to_string(),
    }
}

impl From<PlotError> for CommandError {
    fn from(error: PlotError) -> Self {
        CommandError {
            code: "PLOT_ERROR".to_string(),
            message: error.to_string(),
        }
    }
}

/// Creates a plot from a GeoJSON polygon.
#[tauri::command]
pub fn create_plot(
    state: State<'_, crate::AppState>,
    project_id: Option<String>,
    session_id: Option<String>,
    name: String,
    geometry: Value,
    source_aoi_id: Option<String>,
) -> Result<PlotRecord, CommandError> {
    let polygon = polygon_from_geometry(&geometry)?;
    let conn = state.db.lock().map_err(|_| lock_error())?;
    Ok(insert_plot(
        &conn,
        project_id,
        session_id,
        &name,
        &polygon,
        source_aoi_id,
    )?)
}

/// Lists plots, optionally limited to one project.
#[tauri::command]
pub fn list_plots(
    state: State<'_, crate::AppState>,
    project_id: Option<String>,
) -> Result<Vec<PlotRecord>, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    Ok(list_plot_rows(&conn, project_id.as_deref())?)
}

/// Fetches a single plot.
#[tauri::command]
pub fn get_plot_by_id(
    state: State<'_, crate::AppState>,
    plot_id: String,
) -> Result<PlotRecord, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    get_plot(&conn, &plot_id)?.ok_or_else(|| CommandError {
        code: "PLOT_NOT_FOUND".to_string(),
        message: format!("plot '{plot_id}' not found"),
    })
}

/// Renames a plot.
#[tauri::command]
pub fn rename_plot(
    state: State<'_, crate::AppState>,
    plot_id: String,
    name: String,
) -> Result<PlotRecord, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    Ok(rename_plot_row(&conn, &plot_id, &name)?)
}

/// Deletes a plot.
#[tauri::command]
pub fn delete_plot(state: State<'_, crate::AppState>, plot_id: String) -> Result<(), CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    delete_plot_indicators_for(&conn, &plot_id)?;
    Ok(delete_plot_row(&conn, &plot_id)?)
}

/// Imports every polygon in a GeoJSON file as a plot.
#[tauri::command]
pub fn import_plots_into_project(
    state: State<'_, crate::AppState>,
    path: String,
    project_id: Option<String>,
) -> Result<PlotImportOutcome, CommandError> {
    let text = std::fs::read_to_string(&path).map_err(|error| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("cannot read '{path}': {error}"),
    })?;
    let parsed = crate::plot::parse_plots(&text).map_err(|error| CommandError {
        code: "INVALID_GEOJSON".to_string(),
        message: error,
    })?;

    let conn = state.db.lock().map_err(|_| lock_error())?;
    let (stored, skipped) = import_candidates(&conn, project_id, parsed);
    import_outcome(stored, skipped).map_err(|message| CommandError {
        code: "NO_PLOTS".to_string(),
        message,
    })
}

/// Stores the indicator values of one plugin run for one plot.
#[tauri::command]
pub fn save_plot_indicators(
    state: State<'_, crate::AppState>,
    plot_id: String,
    plugin_id: String,
    execution_id: Option<String>,
    metrics: Value,
) -> Result<usize, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    // The plot must exist, otherwise indicators would be orphaned.
    get_plot(&conn, &plot_id)?.ok_or_else(|| CommandError {
        code: "PLOT_NOT_FOUND".to_string(),
        message: format!("plot '{plot_id}' not found"),
    })?;
    let indicators = normalise_indicators(&metrics);
    Ok(save_indicators(
        &conn,
        &plot_id,
        &plugin_id,
        execution_id.as_deref(),
        &indicators,
    )?)
}

/// Lists stored indicators, optionally for one plot or one project.
#[tauri::command]
pub fn list_plot_indicators(
    state: State<'_, crate::AppState>,
    plot_id: Option<String>,
    project_id: Option<String>,
) -> Result<Vec<IndicatorRecord>, CommandError> {
    let conn = state.db.lock().map_err(|_| lock_error())?;
    Ok(list_indicators(
        &conn,
        plot_id.as_deref(),
        project_id.as_deref(),
    )?)
}

/// Removes indicator rows when a plot is deleted (best effort inside delete).
fn delete_plot_indicators_for(
    conn: &rusqlite::Connection,
    plot_id: &str,
) -> Result<(), CommandError> {
    crate::modules::module_09::indicators::delete_plot_indicators(conn, plot_id)
        .map(|_| ())
        .map_err(Into::into)
}

/// Publishes every plot of a project as one map layer.
///
/// Plot boundaries become a vector layer (`display_preference: polygon`) so
/// Map Explorer can show, toggle and popup them without extra plumbing. The
/// popup lists name, area, perimeter and the latest indicator of every plugin
/// run on the plot (see `modules::module_09::layer`).
#[tauri::command]
pub fn publish_plots_layer(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    project_id: Option<String>,
    layer_name: Option<String>,
) -> Result<LayerPayload, CommandError> {
    use crate::modules::module_03::state::get_global_state;
    use tauri::Emitter;

    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let (rows, indicators) = {
        let conn = state.db.lock().map_err(|_| lock_error())?;
        (
            list_plot_rows(&conn, project_id.as_deref())?,
            // All rows: plots without a project are listed too, so filter by plot id later.
            list_indicators(&conn, None, None)?,
        )
    };
    let rows = Some(rows)
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| CommandError {
            code: "NO_PLOTS".to_string(),
            message: "No plots to show on the map".to_string(),
        })?;

    let payload = plot_layer_payload(
        project_id.as_deref(),
        layer_name,
        &rows,
        &indicators,
        chrono::Utc::now().to_rfc3339(),
    );
    let saved = get_global_state().save_spatial_result(payload)?;
    let _ = app.emit("spatial_result:saved", &saved);
    Ok(saved)
}

/// Runs a plugin over selected plots using a georeferenced raster orthomosaic.
///
/// Clips the orthomosaic per plot and tiles large AOIs into 1024x1024 slices to prevent OOM,
/// runs the plugin subprocess sequentially per slice, aggregates the metrics,
/// and stores the indicators into `plot_indicator`.
#[tauri::command]
pub async fn analyse_plots_from_ortho(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    payload: crate::modules::module_09::ortho_analysis::AnalysePlotsOrthoPayload,
) -> Result<crate::modules::module_09::ortho_analysis::PlotAnalysisOutcome, CommandError> {
    crate::modules::module_09::ortho_analysis::analyse_plots_from_ortho_impl(&app, &state, payload)
        .await
}
