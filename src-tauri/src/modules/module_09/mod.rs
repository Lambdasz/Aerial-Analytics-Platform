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
//! The storage layer lives in [`plots`] and [`indicators`]; the orthomosaic
//! analysis pipeline in [`ortho_analysis`]; GeoJSON import tallying and the
//! Map Explorer plot layer in [`layer`]. Tauri commands that expose them live
//! in `crate::commands::plots`.
//!
//! Requirements (functional and non-functional) are tracked in
//! `src/modules/module-09/REQUIREMENTS.md`. All Module 9 code follows the
//! functional style rules there (NFR-9.1.6–9.1.7): branching with `match` and
//! combinators, no mutable bindings, no imperative loops over collections, and
//! collections built with iterators. Section 7.4 of that file has the check.

pub mod indicators;
pub mod layer;
pub mod ortho_analysis;
pub mod plots;

pub use indicators::IndicatorRecord;
pub use ortho_analysis::*;
pub use plots::{migrate_plot_schema, PlotRecord};

/// Returns `items` with `item` appended, without mutating in place (used as
/// the fold accumulator step). Pure.
pub(crate) fn appended<T>(items: Vec<T>, item: T) -> Vec<T> {
    items.into_iter().chain(std::iter::once(item)).collect()
}
