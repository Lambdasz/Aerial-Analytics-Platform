//! Tauri commands for Module 9 plot definition and listing.
//!
//! Thin wrappers: they take `AppState`'s database connection, delegate to
//! `modules::module_09::plots`, and map errors to `CommandError`.

use crate::modules::module_03::sp_result::types::LayerPayload;
use crate::modules::module_09::indicators::{save_indicators, IndicatorRecord};
use crate::modules::module_09::plots::{
    delete_plot as delete_plot_row, get_plot, insert_plot, list_plots as list_plot_rows,
    polygon_from_geometry, rename_plot as rename_plot_row, PlotError, PlotRecord,
};
use crate::plugin_manager::error::CommandError;
use serde::Serialize;
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

/// Result of a plot import.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotImportOutcome {
    pub plots: Vec<PlotRecord>,
    pub skipped: Vec<String>,
}

/// Creates a plot from a GeoJSON polygon.
#[tauri::command]
pub fn create_plot(
    state: State<'_, crate::AppState>,
    project_id: Option<String>,
    session_id: Option<String>,
    name: String,
    geometry: Value,
) -> Result<PlotRecord, CommandError> {
    let polygon = polygon_from_geometry(&geometry)?;
    let conn = state.db.lock().map_err(|_| lock_error())?;
    Ok(insert_plot(&conn, project_id, session_id, &name, &polygon)?)
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
    let mut stored = Vec::with_capacity(parsed.plots.len());
    let mut skipped = parsed.skipped;
    for candidate in parsed.plots {
        let geometry = serde_json::json!({ "type": "Polygon", "coordinates": candidate.rings });
        match polygon_from_geometry(&geometry) {
            Ok(polygon) => {
                match insert_plot(
                    &conn,
                    project_id.clone(),
                    None,
                    &candidate.plot_name,
                    &polygon,
                ) {
                    Ok(record) => stored.push(record),
                    Err(error) => skipped.push(format!("{}: {error}", candidate.plot_name)),
                }
            }
            Err(error) => skipped.push(format!("{}: {error}", candidate.plot_name)),
        }
    }

    if stored.is_empty() {
        return Err(CommandError {
            code: "NO_PLOTS".to_string(),
            message: if skipped.is_empty() {
                "the file contains no plots".to_string()
            } else {
                format!("no plot could be imported ({})", skipped.join("; "))
            },
        });
    }
    Ok(PlotImportOutcome {
        plots: stored,
        skipped,
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
    if get_plot(&conn, &plot_id)?.is_none() {
        return Err(CommandError {
            code: "PLOT_NOT_FOUND".to_string(),
            message: format!("plot '{plot_id}' not found"),
        });
    }
    let indicators = crate::modules::module_09::indicators::normalise_indicators(&metrics);
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
    Ok(crate::modules::module_09::indicators::list_indicators(
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
/// Map Explorer can show, toggle and popup them without extra plumbing.
#[tauri::command]
pub fn publish_plots_layer(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    project_id: Option<String>,
    layer_name: Option<String>,
) -> Result<LayerPayload, CommandError> {
    use crate::modules::module_03::sp_result::types::{
        FieldDef, LayerDisplayMode, LayerPayload, SpatialGeometry, SpatialProperties, SpatialResult,
    };
    use crate::modules::module_03::state::get_global_state;
    use serde_json::json;
    use tauri::Emitter;

    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let rows = {
        let conn = state.db.lock().map_err(|_| lock_error())?;
        list_plot_rows(&conn, project_id.as_deref())?
    };
    if rows.is_empty() {
        return Err(CommandError {
            code: "NO_PLOTS".to_string(),
            message: "No plots to show on the map".to_string(),
        });
    }

    let features: Vec<SpatialResult> = rows
        .iter()
        .filter_map(|plot| {
            let geometry: SpatialGeometry = serde_json::from_value(plot.geometry.clone()).ok()?;
            let mut properties = serde_json::Map::new();
            properties.insert("plot_id".into(), json!(plot.id));
            properties.insert("plot_name".into(), json!(plot.name));
            properties.insert("area_m2".into(), json!(plot.area_m2));
            properties.insert("area_ha".into(), json!(plot.area_ha));
            properties.insert("perimeter_m".into(), json!(plot.perimeter_m));
            Some(SpatialResult {
                source: plot.id.clone(),
                geometry,
                properties: SpatialProperties::Generic(properties),
                image_url: None,
                edited_fields: None,
                extra: serde_json::Map::new(),
            })
        })
        .collect();

    let fields = vec![
        FieldDef {
            key: "plot_name".into(),
            label: "Plot".into(),
            unit: None,
            visible: true,
            custom: false,
            kind: Some("text".into()),
        },
        FieldDef {
            key: "area_ha".into(),
            label: "Area".into(),
            unit: Some("ha".into()),
            visible: true,
            custom: false,
            kind: Some("number".into()),
        },
        FieldDef {
            key: "perimeter_m".into(),
            label: "Perimeter".into(),
            unit: Some("m".into()),
            visible: true,
            custom: false,
            kind: Some("number".into()),
        },
    ];

    let layer_id = project_id
        .as_deref()
        .map(|id| format!("plots_{}", id.replace('-', "")))
        .unwrap_or_else(|| "plots_all".to_string());
    let payload = LayerPayload {
        layer_id: layer_id.clone(),
        layer_name: layer_name.unwrap_or_else(|| "Plots".to_string()),
        display_preference: LayerDisplayMode::Polygon {
            color: "#0066cc".to_string(),
            opacity: 0.25,
            image_url: None,
            fields: Some(fields),
        },
        spatial_results: features,
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
        concurrency_token: None,
        extra: serde_json::Map::new(),
    };

    let saved = get_global_state().save_spatial_result(payload)?;
    let _ = app.emit("spatial_result:saved", &saved);
    Ok(saved)
}
