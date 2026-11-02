//! Module 5 - Vegetation Coverage Analytics Plugin
//!
//! Pure analytical core (indices, coverage, classification, zonation, plot
//! comparison, GeoJSON assembly) plus the impure edges (image/mask loading and
//! artifact writing) partitioned into distinct functional modules.
//!
//! Structure:
//! - `types`: Domain structs, enums, errors, and constants.
//! - `indices`: Vegetation index mathematical formulas (ExG, ExR, VARI) and thresholding.
//! - `coverage`: Coverage percentage, low/medium/high classification, and area calculations.
//! - `geo`: Ground sample distance (GSD), GPS coordinates, and bounds resolution.
//! - `zonation`: NxN grid partitioning and GeoJSON zonation layer generation.
//! - `plots`: Per-plot raycasting polygon containment, per-plot metrics, and ranking.
//! - `pipeline`: Impure edges, file I/O, payload parsing, and output writing.

pub mod coverage;
pub mod geo;
pub mod indices;
pub mod pipeline;
pub mod plots;
pub mod types;
pub mod zonation;

pub use coverage::*;
pub use geo::*;
pub use indices::*;
pub use pipeline::*;
pub use plots::*;
pub use types::*;
pub use zonation::*;
