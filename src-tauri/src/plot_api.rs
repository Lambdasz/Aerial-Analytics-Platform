//! # Plot Analytics API (Module 9) — deprecated
//!
//! **Deprecated (ISU-9.2 in `src/modules/module-09/REQUIREMENTS.md`).** This
//! is the original cross-module contract. It is superseded by
//! `crate::modules::module_09` and the commands in `crate::commands::plots`;
//! the functions below are kept only until the Project Lead approves removing
//! the file, and they return an error naming the replacement instead of
//! panicking.
//!
//! | Legacy function | Replacement |
//! |-----------------|-------------|
//! | [`define_plot`] | `create_plot` / `modules::module_09::plots::insert_plot` |
//! | [`analyze_plot`] | `analyse_plots_from_ortho` |
//! | [`get_plot_statistics`] | `list_plot_indicators` / `modules::module_09::indicators::list_indicators` |
//!
//! Cross-module API for defining plots, running plugins per-plot, and
//! delivering aggregated statistics to the dashboard.
//!
//! ## Integration Flow
//!
//! ```text
//!   ┌──────────┐     ┌──────────┐     ┌──────────────┐
//!   │ Module 3 │────▶│ Module 9 │────▶│  Module 2    │
//!   │ (Map)    │◀────│ (Plots)  │◀────│ (Plugin Mgr) │
//!   └──────────┘     └────┬─────┘     └──────────────┘
//!                         │
//!                         ▼
//!                  ┌──────────────┐
//!                  │  Module 11   │
//!                  │ (Dashboard)  │
//!                  └──────────────┘
//! ```
//!
//! ## Functions
//!
//! | Function | Direction | Description |
//! |----------|-----------|-------------|
//! | [`define_plot`] | Module 3 → 9 → 3 | Validate and store a plot boundary |
//! | [`analyze_plot`] | Module 9 → 2 → 9 | Run selected plugins within a plot |
//! | [`get_plot_statistics`] | Module 9 → 11 | Deliver per-plot statistics for the dashboard |
//!
//! ## Geometry Conventions
//!
//! - Format: GeoJSON `Feature<Polygon>` (RFC 7946).
//! - CRS: WGS 84 (`EPSG:4326`).
//! - Coordinate order: `[longitude, latitude]`.

use crate::plugin_manager::result_handler::ExecutionResult;
use geojson::Feature;
use serde::{Deserialize, Serialize};

/// Plot layer returned to Module 3 for map display.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlotLayer {
    /// Unique plot identifier (UUID string).
    pub plot_id: String,
    /// Display name shown to the user.
    pub plot_name: String,
    /// Plot geometry: `Feature<Polygon>` WGS 84, identical to
    /// `docs/examples/aoi.example.geojson`. Properties use Module 3 field
    /// names: `aoi_id` (= `plot_id`) and `name` (= `plot_name`).
    pub geometry: Feature,
}

/// Analysis result of a single plugin on a single plot.
#[derive(Serialize, Deserialize, Debug)]
pub struct PluginRunResult {
    /// Plugin identifier that was executed.
    pub plugin_id: String,
    /// Execution result from Module 2 (`plugin_manager::result_handler::ExecutionResult`).
    pub result: ExecutionResult,
}

/// Combined analysis result for a plot across all executed plugins.
#[derive(Serialize, Deserialize, Debug)]
pub struct PlotAnalysisResult {
    /// Plot identifier that was analysed.
    pub plot_id: String,
    /// Per-plugin results, in the same order as the `plugin_ids` request.
    pub results: Vec<PluginRunResult>,
}

/// Summary statistics for a single indicator on a plot.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndicatorStats {
    /// Mean value.
    pub mean: Option<f64>,
    /// Minimum value.
    pub min: Option<f64>,
    /// Maximum value.
    pub max: Option<f64>,
    /// Standard deviation.
    pub stddev: Option<f64>,
}

/// A single plugin indicator on a plot.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlotIndicator {
    /// Indicator name, e.g. `"vegetation_cover"`.
    pub name: String,
    /// Unit of measurement, e.g. `"%"` or `"trees"`.
    pub unit: String,
    /// Plugin or module that produced this indicator.
    pub source: String,
    /// Primary indicator value.
    pub value: f64,
    /// Additional statistics; `None` when the plugin does not provide them.
    pub stats: Option<IndicatorStats>,
}

/// Aggregated plot statistics for the Module 11 dashboard.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlotStatistics {
    /// Plot identifier.
    pub plot_id: String,
    /// Plot display name.
    pub plot_name: String,
    /// Area in square metres (from Module 3 Spatial Measurement).
    pub area_m2: f64,
    /// Area in hectares (from Module 3 Spatial Measurement).
    pub area_ha: f64,
    /// Perimeter in metres (from Module 3 Spatial Measurement).
    pub perimeter_m: f64,
    /// List of plugin indicators for this plot.
    pub indicators: Vec<PlotIndicator>,
}

/// Defines a plot from an AOI drawn in Module 3.
///
/// # Integration
///
/// - **Direction**: Module 3 → Module 9 → Module 3
/// - **Diagram steps**: 2 and 5
///
/// # Description
///
/// Receives an AOI from Module 3, validates its geometry (closed polygon,
/// minimum 3 vertices, no self-intersection — see `RULES_MAP_CONTROLLER.md`),
/// stores it as a plot, and returns a [`PlotLayer`] for Module 3 to display.
/// The plot name is taken from `properties.name`.
///
/// # Arguments
///
/// * `aoi` — `Feature<Polygon>` RFC 7946, WGS 84, `[longitude, latitude]`
///   order, matching `docs/examples/aoi.example.geojson`.
///
/// # Returns
///
/// `Ok(PlotLayer)` on success. `Err` with a descriptive message if the
/// geometry is invalid.
///
/// # Example
///
/// ```json
/// {
///   "type": "Feature",
///   "properties": { "name": "Plot A" },
///   "geometry": {
///     "type": "Polygon",
///     "coordinates": [[[116.833,-1.27],[116.834,-1.27],[116.834,-1.271],[116.833,-1.271],[116.833,-1.27]]]
///   }
/// }
/// ```
#[deprecated(note = "use the `create_plot` command / `modules::module_09::plots::insert_plot`")]
pub fn define_plot(aoi: Feature) -> Result<PlotLayer, String> {
    let _ = aoi;
    Err(superseded("define_plot", "create_plot"))
}

/// Runs selected plugins within a single plot via Module 2.
///
/// # Integration
///
/// - **Direction**: Module 9 → Module 2 → Module 9
/// - **Diagram steps**: 3 and 4
///
/// # Description
///
/// For each plugin, Module 9 assembles an execution payload
/// with the plot geometry as AOI, calls
/// `validate_and_create_payload`,
/// then after execution reads the result via
/// `process_execution_result`.
/// Plugins are never called directly — Module 2 always mediates.
///
/// # Arguments
///
/// * `plot_id` — UUID string of an already-defined plot.
/// * `plugin_ids` — list of plugin IDs to execute on this plot.
/// * `image_path` — absolute path to the source aerial image.
///
/// # Returns
///
/// `Ok(PlotAnalysisResult)` with per-plugin execution results.
///
/// # Example
///
/// ```json
/// {
///   "plot_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///   "plugin_ids": ["vegetation_index_v1"],
///   "image_path": "/data/DJI_0042.JPG"
/// }
/// ```
#[deprecated(note = "use the `analyse_plots_from_ortho` command")]
pub fn analyze_plot(
    plot_id: String,
    plugin_ids: Vec<String>,
    image_path: String,
) -> Result<PlotAnalysisResult, String> {
    let _ = (plot_id, plugin_ids, image_path);
    Err(superseded("analyze_plot", "analyse_plots_from_ortho"))
}

/// Delivers per-plot statistics to the Module 11 dashboard.
///
/// # Integration
///
/// - **Direction**: Module 9 → Module 11
/// - **Diagram step**: 6
///
/// # Description
///
/// Collects all plots in a project with their area, perimeter, and plugin
/// indicators. Area and perimeter are computed by Module 3 Spatial
/// Measurement (not yet available). Module 11 uses the returned data for
/// comparison and ranking.
///
/// # Arguments
///
/// * `project_id` — identifier of the project whose plots are requested.
///
/// # Returns
///
/// `Ok(Vec<PlotStatistics>)` — one entry per plot.
#[deprecated(
    note = "use the `list_plot_indicators` command / `modules::module_09::indicators::list_indicators`"
)]
pub fn get_plot_statistics(project_id: String) -> Result<Vec<PlotStatistics>, String> {
    let _ = project_id;
    Err(superseded("get_plot_statistics", "list_plot_indicators"))
}

/// Error message for a legacy entry point. Pure.
fn superseded(legacy: &str, replacement: &str) -> String {
    format!("plot_api::{legacy} is deprecated; use the `{replacement}` command (Module 9)")
}

#[cfg(test)]
mod tests {
    #[test]
    #[allow(deprecated)]
    fn legacy_functions_fail_without_panicking() {
        let error = super::get_plot_statistics("p".into()).unwrap_err();
        assert!(error.contains("list_plot_indicators"), "{error}");
        let error = super::analyze_plot("p".into(), Vec::new(), String::new()).unwrap_err();
        assert!(error.contains("analyse_plots_from_ortho"), "{error}");
    }
}
