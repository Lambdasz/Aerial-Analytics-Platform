//! Domain types, data structures, and constants for Module 5.
//!
//! Pure data modeling without side-effects or external dependencies beyond serde.

use std::fmt;

pub type Result<T> = std::result::Result<T, PluginError>;

#[derive(Debug)]
pub enum PluginError {
    Io(String),
    Value(String),
    FileNotFound(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::Io(m) => write!(f, "IO error: {m}"),
            PluginError::Value(m) => write!(f, "Invalid value: {m}"),
            PluginError::FileNotFound(m) => write!(f, "File not found: {m}"),
        }
    }
}

impl std::error::Error for PluginError {}

pub const METERS_PER_DEGREE_LAT: f64 = 111_320.0;
pub const EPSILON: f64 = 1e-6;
/// Fallback horizontal field of view when metadata carries no focal length.
pub const ASSUMED_HFOV_DEG: f64 = 60.0;
/// Suggestion table from Module 4 — used when the payload omits a threshold.
pub const VALID_METHODS: [&str; 3] = ["ExG", "ExR", "VARI"];

/// Normalized RGB image: channels in 0..1, row-major.
#[derive(Debug, Clone)]
pub struct RgbArray {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f64; 3]>,
}

/// Binary vegetation mask: one byte per pixel, 0 or 1.
#[derive(Debug, Clone)]
pub struct BinaryMask {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl BinaryMask {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Pixel index of a row/column pair, bounds-checked by the caller.
    pub fn at(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.width + x]
    }
}

/// One NxN zonation cell: pixel window plus its coverage figures.
#[derive(Debug, Clone)]
pub struct GridCell {
    pub row: usize,
    pub col: usize,
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
    pub coverage: Coverage,
}

/// Coverage figures for a pixel set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coverage {
    pub total_pixels: usize,
    pub vegetation_pixels: usize,
    pub coverage_percentage: f64,
}

/// Per-plot coverage result (Module 5 §5).
#[derive(Debug, Clone, PartialEq)]
pub struct PlotResult {
    pub plot_id: String,
    pub coverage: Coverage,
    pub vegetation_area_sqm: Option<f64>,
    pub classification: String,
}

/// Aggregate comparison across plots (Module 5 §6).
#[derive(Debug, Clone, PartialEq)]
pub struct PlotComparison {
    pub average_coverage: Option<f64>,
    pub highest_plot_id: Option<String>,
    pub lowest_plot_id: Option<String>,
    /// Plot ids ordered from highest to lowest coverage.
    pub ranking: Vec<String>,
}
