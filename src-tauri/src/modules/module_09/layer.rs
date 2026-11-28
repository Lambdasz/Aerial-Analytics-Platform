//! Plot import tally and Map Explorer layer building (Module 9).
//!
//! Everything here except [`import_candidates`] is pure: the Tauri commands in
//! `crate::commands::plots` read the database, call these functions, and do
//! the I/O (saving the layer, emitting events).
//!
//! Follows the functional style rules NFR-9.1.6–9.1.7
//! (`src/modules/module-09/REQUIREMENTS.md`).

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};

use super::appended;
use super::indicators::IndicatorRecord;
use super::plots::{insert_plot, polygon_from_geometry, PlotRecord};
use crate::modules::module_03::sp_result::types::{
    FieldDef, LayerDisplayMode, LayerPayload, SpatialGeometry, SpatialProperties, SpatialResult,
};

/// Result of a plot import.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotImportOutcome {
    pub plots: Vec<PlotRecord>,
    pub skipped: Vec<String>,
}

/// Stores every parsed candidate as a plot. Candidates that fail validation or
/// insertion are added to `skipped` instead of aborting the import (FR-9.1.2).
/// Impure — writes rows.
pub fn import_candidates(
    conn: &Connection,
    project_id: Option<String>,
    parsed: crate::plot::ImportResult,
) -> (Vec<PlotRecord>, Vec<String>) {
    parsed.plots.into_iter().fold(
        (Vec::new(), parsed.skipped),
        |(stored, skipped), candidate| {
            let geometry = json!({ "type": "Polygon", "coordinates": candidate.rings });
            match polygon_from_geometry(&geometry).and_then(|polygon| {
                insert_plot(
                    conn,
                    project_id.clone(),
                    None,
                    &candidate.plot_name,
                    &polygon,
                    None,
                )
            }) {
                Ok(record) => (appended(stored, record), skipped),
                Err(error) => (
                    stored,
                    appended(skipped, format!("{}: {error}", candidate.plot_name)),
                ),
            }
        },
    )
}

/// Turns the import tally into an outcome; an import that stored nothing is an
/// error that explains why. Pure.
pub fn import_outcome(
    stored: Vec<PlotRecord>,
    skipped: Vec<String>,
) -> Result<PlotImportOutcome, String> {
    match (stored.is_empty(), skipped.is_empty()) {
        (true, true) => Err("the file contains no plots".to_string()),
        (true, false) => Err(format!(
            "no plot could be imported ({})",
            skipped.join("; ")
        )),
        (false, _) => Ok(PlotImportOutcome {
            plots: stored,
            skipped,
        }),
    }
}

/// Property key of one indicator in the plot layer: plugin and indicator name,
/// so the same metric from two plugins never collides (FR-9.6.3). Pure.
pub fn indicator_property_key(plugin_id: &str, indicator: &str) -> String {
    format!("ind_{plugin_id}_{indicator}")
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '_' => c,
            _ => '_',
        })
        .collect()
}

/// Display value of a stored indicator: the number, otherwise the text. Pure.
fn indicator_value(row: &IndicatorRecord) -> Option<Value> {
    row.value
        .map(|value| json!(value))
        .or_else(|| row.text_value.as_ref().map(|text| json!(text)))
}

/// Popup properties of one plot: name, area, perimeter and its latest
/// indicators (FR-9.8.2, FR-9.8.3). Pure.
pub fn plot_properties(
    plot: &PlotRecord,
    indicators: &[IndicatorRecord],
) -> serde_json::Map<String, Value> {
    let base = [
        ("plot_id".to_string(), json!(plot.id)),
        ("plot_name".to_string(), json!(plot.name)),
        ("area_m2".to_string(), json!(plot.area_m2)),
        ("area_ha".to_string(), json!(plot.area_ha)),
        ("perimeter_m".to_string(), json!(plot.perimeter_m)),
    ];
    let measured = indicators
        .iter()
        .filter(|row| row.plot_id == plot.id)
        .filter_map(|row| {
            indicator_value(row).map(|value| {
                (
                    indicator_property_key(&row.plugin_id, &row.indicator),
                    value,
                )
            })
        });
    base.into_iter().chain(measured).collect()
}

fn field(key: &str, label: &str, unit: Option<&str>, kind: &str) -> FieldDef {
    FieldDef {
        key: key.to_string(),
        label: label.to_string(),
        unit: unit.map(str::to_string),
        visible: true,
        custom: false,
        kind: Some(kind.to_string()),
    }
}

/// Popup field definitions: the plot fields, then one field per
/// (plugin, indicator) pair, sorted for a stable popup order. Pure.
pub fn plot_layer_fields(indicators: &[IndicatorRecord]) -> Vec<FieldDef> {
    let base = [
        field("plot_name", "Plot", None, "text"),
        field("area_ha", "Area", Some("ha"), "number"),
        field("perimeter_m", "Perimeter", Some("m"), "number"),
    ];
    let indicator_fields: BTreeMap<(String, String), FieldDef> = indicators
        .iter()
        .map(|row| {
            let label = format!("{} ({})", row.indicator.replace('_', " "), row.plugin_id);
            let kind = row.value.map(|_| "number").unwrap_or("text");
            (
                (row.plugin_id.clone(), row.indicator.clone()),
                field(
                    &indicator_property_key(&row.plugin_id, &row.indicator),
                    &label,
                    row.unit.as_deref(),
                    kind,
                ),
            )
        })
        .collect();
    base.into_iter()
        .chain(indicator_fields.into_values())
        .collect()
}

/// Layer id for a project's plot layer. Pure.
pub fn plot_layer_id(project_id: Option<&str>) -> String {
    project_id
        .map(|id| format!("plots_{}", id.replace('-', "")))
        .unwrap_or_else(|| "plots_all".to_string())
}

/// Builds the Map Explorer layer for a set of plots. Plots whose geometry
/// cannot be read as a spatial geometry are left out. Pure (the timestamp is
/// passed in).
pub fn plot_layer_payload(
    project_id: Option<&str>,
    layer_name: Option<String>,
    plots: &[PlotRecord],
    indicators: &[IndicatorRecord],
    updated_at: String,
) -> LayerPayload {
    let features: Vec<SpatialResult> = plots
        .iter()
        .filter_map(|plot| {
            let geometry: SpatialGeometry = serde_json::from_value(plot.geometry.clone()).ok()?;
            Some(SpatialResult {
                source: plot.id.clone(),
                geometry,
                properties: SpatialProperties::Generic(plot_properties(plot, indicators)),
                image_url: None,
                edited_fields: None,
                extra: serde_json::Map::new(),
            })
        })
        .collect();
    let plot_ids: Vec<&str> = plots.iter().map(|plot| plot.id.as_str()).collect();
    let own_indicators: Vec<IndicatorRecord> = indicators
        .iter()
        .filter(|row| plot_ids.contains(&row.plot_id.as_str()))
        .cloned()
        .collect();

    LayerPayload {
        layer_id: plot_layer_id(project_id),
        layer_name: layer_name.unwrap_or_else(|| "Plots".to_string()),
        display_preference: LayerDisplayMode::Polygon {
            color: "#0066cc".to_string(),
            opacity: 0.25,
            image_url: None,
            fields: Some(plot_layer_fields(&own_indicators)),
        },
        spatial_results: features,
        updated_at: Some(updated_at),
        concurrency_token: None,
        extra: serde_json::Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plot(id: &str) -> PlotRecord {
        PlotRecord {
            id: id.to_string(),
            project_id: Some("p1".into()),
            session_id: None,
            name: format!("Plot {id}"),
            geometry: json!({
                "type": "Polygon",
                "coordinates": [[[0.0, 0.0], [0.001, 0.0], [0.001, 0.001], [0.0, 0.0]]]
            }),
            source_aoi_id: None,
            area_m2: 5000.0,
            area_ha: 0.5,
            perimeter_m: 340.0,
            created_at: "2026-11-25T00:00:00.000Z".into(),
            updated_at: "2026-11-25T00:00:00.000Z".into(),
        }
    }

    fn indicator(plot_id: &str, plugin_id: &str, name: &str, value: f64) -> IndicatorRecord {
        IndicatorRecord {
            plot_id: plot_id.to_string(),
            plugin_id: plugin_id.to_string(),
            execution_id: None,
            indicator: name.to_string(),
            unit: Some("%".into()),
            value: Some(value),
            text_value: None,
            created_at: "2026-11-25T00:00:00.000Z".into(),
        }
    }

    #[test]
    fn properties_carry_plot_metrics_and_own_indicators() {
        let rows = [
            indicator("a", "veg-cov", "coverage_percentage", 40.0),
            indicator("b", "veg-cov", "coverage_percentage", 70.0),
        ];
        let props = plot_properties(&plot("a"), &rows);
        assert_eq!(props["plot_name"], "Plot a");
        assert_eq!(props["area_ha"], 0.5);
        assert_eq!(props["ind_veg_cov_coverage_percentage"], 40.0);
    }

    #[test]
    fn same_indicator_from_two_plugins_gets_two_fields() {
        let rows = [
            indicator("a", "plugin-x", "coverage_percentage", 40.0),
            indicator("a", "plugin-y", "coverage_percentage", 55.0),
            indicator("b", "plugin-x", "coverage_percentage", 70.0),
        ];
        let fields = plot_layer_fields(&rows);
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "plot_name",
                "area_ha",
                "perimeter_m",
                "ind_plugin_x_coverage_percentage",
                "ind_plugin_y_coverage_percentage"
            ]
        );
        let props = plot_properties(&plot("a"), &rows);
        assert_eq!(props["ind_plugin_x_coverage_percentage"], 40.0);
        assert_eq!(props["ind_plugin_y_coverage_percentage"], 55.0);
    }

    #[test]
    fn import_outcome_reports_why_nothing_was_stored() {
        assert!(import_outcome(Vec::new(), Vec::new())
            .unwrap_err()
            .contains("no plots"));
        assert!(import_outcome(Vec::new(), vec!["bad".into()])
            .unwrap_err()
            .contains("bad"));
        let outcome = import_outcome(vec![plot("a")], vec!["bad".into()]).unwrap();
        assert_eq!(outcome.plots.len(), 1);
        assert_eq!(outcome.skipped, ["bad"]);
    }

    #[test]
    fn layer_id_depends_on_project() {
        assert_eq!(plot_layer_id(Some("ab-cd")), "plots_abcd");
        assert_eq!(plot_layer_id(None), "plots_all");
    }
}
