//! Module 9 — Area & Plot Analytics.
//!
//! Spatial analytical system for defining plots (polygon boundaries) and
//! summarising analytical results within them: plot definition, area and
//! perimeter, per-plot analysis, statistics, comparison, ranking and summary.
//!
//! Plot geometry follows the platform convention: GeoJSON `Polygon` in WGS 84
//! (`EPSG:4326`), `[longitude, latitude]` order, first ring = outer boundary,
//! any further rings = holes.
//!
//! The storage layer lives in [`plots`]; Tauri commands that expose it live in
//! `crate::commands::plots`.

pub mod indicators;
pub mod plots;

pub use indicators::IndicatorRecord;
pub use plots::{migrate_plot_schema, PlotRecord};
