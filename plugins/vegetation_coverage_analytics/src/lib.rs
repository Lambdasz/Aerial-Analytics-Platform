//! Module 5 - Vegetation Coverage Analytics Plugin
//!
//! Pure analytical core (indices, coverage, classification, zonation, plot
//! comparison, GeoJSON assembly) plus the impure edges (image/mask loading and
//! artifact writing) kept apart at the bottom of the file.
//!
//! Usage:
//!     main --healthcheck
//!     main --input payload.json --output result.json

use image::{DynamicImage, RgbaImage};
use serde_json::{json, Map, Value};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

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

const METERS_PER_DEGREE_LAT: f64 = 111_320.0;
const EPSILON: f64 = 1e-6;
/// Fallback horizontal field of view when metadata carries no focal length.
const ASSUMED_HFOV_DEG: f64 = 60.0;
/// Suggestion table from Module 4 — used when the payload omits a threshold.
const VALID_METHODS: [&str; 3] = ["ExG", "ExR", "VARI"];

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
    fn at(&self, x: usize, y: usize) -> u8 {
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

// ------------------------------------------------------------------ indices

/// Per-pixel vegetation index value on normalized channels.
pub fn pixel_index(method: &str, p: [f64; 3]) -> Result<f64> {
    let [r, g, b] = p;
    match method {
        "ExG" => Ok(2.0 * g - r - b),
        "ExR" => Ok(1.4 * r - g),
        "VARI" => Ok((g - r) / (g + r - b + EPSILON)),
        _ => Err(PluginError::Value(format!("Unknown method: {method}"))),
    }
}

/// Default detection threshold per index, mirroring the Module 4 suggestion.
pub fn default_threshold(method: &str) -> f64 {
    match method {
        "ExR" => 0.0,
        "VARI" => 0.03,
        _ => 0.10,
    }
}

/// Maps every pixel to its index value. Pure.
pub fn vegetation_index(rgb: &RgbArray, method: &str) -> Result<Vec<f64>> {
    rgb.pixels.iter().map(|&p| pixel_index(method, p)).collect()
}

/// Thresholds index values into a 0/1 mask. Pure.
pub fn apply_threshold(index: &[f64], threshold_value: f64) -> Result<Vec<u8>> {
    if !(0.0..=1.0).contains(&threshold_value) {
        return Err(PluginError::Value(format!(
            "threshold_value must be between 0.0 and 1.0, got {threshold_value}"
        )));
    }
    Ok(index
        .iter()
        .map(|&v| u8::from(v > threshold_value))
        .collect())
}

/// Derives a mask straight from an RGB image. Pure.
pub fn mask_from_rgb(rgb: &RgbArray, method: &str, threshold_value: f64) -> Result<BinaryMask> {
    let index = vegetation_index(rgb, method)?;
    let data = apply_threshold(&index, threshold_value)?;
    Ok(BinaryMask {
        width: rgb.width,
        height: rgb.height,
        data,
    })
}

// ----------------------------------------------------------------- coverage

/// Counts coverage over the whole mask, or only where `keep` is true (AOI). Pure.
pub fn calculate_coverage(mask: &BinaryMask, keep: Option<&[bool]>) -> Coverage {
    let (mut total, mut vegetation) = (0usize, 0usize);
    for (i, &flag) in mask.data.iter().enumerate() {
        if let Some(selector) = keep {
            if !selector.get(i).copied().unwrap_or(false) {
                continue;
            }
        }
        total += 1;
        vegetation += flag as usize;
    }
    Coverage {
        total_pixels: total,
        vegetation_pixels: vegetation,
        coverage_percentage: if total == 0 {
            0.0
        } else {
            (vegetation as f64 / total as f64) * 100.0
        },
    }
}

/// Vegetation and total ground area in m²; both `None` without a GSD. Pure.
pub fn area_metrics(coverage: &Coverage, gsd_m: Option<f64>) -> (Option<f64>, Option<f64>) {
    match gsd_m {
        Some(gsd) if gsd > 0.0 => {
            let pixel_area = gsd * gsd;
            (
                Some(coverage.vegetation_pixels as f64 * pixel_area),
                Some(coverage.total_pixels as f64 * pixel_area),
            )
        }
        _ => (None, None),
    }
}

/// Low/medium/high classification from user thresholds. Pure.
pub fn classify_coverage(
    coverage_percentage: f64,
    low_threshold: f64,
    medium_threshold: f64,
) -> Result<String> {
    if !(0.0..=100.0).contains(&low_threshold) || !(0.0..=100.0).contains(&medium_threshold) {
        return Err(PluginError::Value(
            "coverage thresholds must be between 0 and 100".into(),
        ));
    }
    if low_threshold > medium_threshold {
        return Err(PluginError::Value(format!(
            "low_threshold ({low_threshold}) must not exceed medium_threshold ({medium_threshold})"
        )));
    }
    let class = if coverage_percentage <= low_threshold {
        "low"
    } else if coverage_percentage <= medium_threshold {
        "medium"
    } else {
        "high"
    };
    Ok(class.to_string())
}

// ----------------------------------------------------------------- zonation

/// Splits the mask into `grid_size` × `grid_size` cells and counts coverage in
/// each. Cell boundaries use integer arithmetic, so uneven dimensions never
/// lose pixels. Pure.
pub fn generate_zonation(
    mask: &BinaryMask,
    keep: Option<&[bool]>,
    grid_size: usize,
) -> Result<Vec<GridCell>> {
    if grid_size == 0 || grid_size > 64 {
        return Err(PluginError::Value(format!(
            "grid_size must be between 1 and 64, got {grid_size}"
        )));
    }
    if mask.width == 0 || mask.height == 0 {
        return Err(PluginError::Value("mask has no pixels".into()));
    }

    let bounds = |total: usize, part: usize, index: usize| -> (usize, usize) {
        let start = index * total / part;
        let end = (index + 1) * total / part;
        (start, end)
    };

    let mut cells = Vec::with_capacity(grid_size * grid_size);
    for row in 0..grid_size {
        let (y0, y1) = bounds(mask.height, grid_size, row);
        for col in 0..grid_size {
            let (x0, x1) = bounds(mask.width, grid_size, col);
            let (mut total, mut vegetation) = (0usize, 0usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * mask.width + x;
                    if let Some(selector) = keep {
                        if !selector.get(i).copied().unwrap_or(false) {
                            continue;
                        }
                    }
                    total += 1;
                    vegetation += mask.at(x, y) as usize;
                }
            }
            cells.push(GridCell {
                row,
                col,
                x0,
                y0,
                x1,
                y1,
                coverage: Coverage {
                    total_pixels: total,
                    vegetation_pixels: vegetation,
                    coverage_percentage: if total == 0 {
                        0.0
                    } else {
                        (vegetation as f64 / total as f64) * 100.0
                    },
                },
            });
        }
    }
    Ok(cells)
}

// ------------------------------------------------------- georeferencing math

/// Ground sample distance from altitude and 35 mm-equivalent focal length.
/// Pure.
pub fn calculate_gsd(
    relative_altitude_m: f64,
    focal_length_35mm: f64,
    image_width_px: usize,
    image_height_px: usize,
) -> Option<f64> {
    const SENSOR_DIAGONAL_35MM_MM: f64 = 43.27;
    if relative_altitude_m <= 0.0 || focal_length_35mm <= 0.0 {
        return None;
    }
    let diagonal_px = (image_width_px as f64).hypot(image_height_px as f64);
    if diagonal_px <= 0.0 {
        return None;
    }
    Some((relative_altitude_m * SENSOR_DIAGONAL_35MM_MM) / (focal_length_35mm * diagonal_px))
}

/// Ground sample distance from altitude and an assumed horizontal field of
/// view. Pure.
pub fn calculate_gsd_from_hfov(
    relative_altitude_m: f64,
    hfov_deg: f64,
    image_width_px: usize,
) -> Option<f64> {
    if relative_altitude_m <= 0.0 || image_width_px == 0 || !(1.0..179.0).contains(&hfov_deg) {
        return None;
    }
    let ground_width_m = 2.0 * relative_altitude_m * (hfov_deg / 2.0).to_radians().tan();
    Some(ground_width_m / image_width_px as f64)
}

/// `[min_lon, min_lat, max_lon, max_lat]` centered on a GPS point. Pure.
pub fn bounds_from_gps(
    lat: f64,
    lon: f64,
    gsd_m: f64,
    image_width_px: usize,
    image_height_px: usize,
) -> Option<[f64; 4]> {
    if gsd_m <= 0.0 || image_width_px == 0 || image_height_px == 0 {
        return None;
    }
    let ground_width = image_width_px as f64 * gsd_m;
    let ground_height = image_height_px as f64 * gsd_m;
    let delta_lat = (ground_height / 2.0) / METERS_PER_DEGREE_LAT;
    let delta_lon = (ground_width / 2.0) / (METERS_PER_DEGREE_LAT * lat.to_radians().cos());
    Some([
        lon - delta_lon,
        lat - delta_lat,
        lon + delta_lon,
        lat + delta_lat,
    ])
}

/// Pixel centers expressed in WGS84 lon/lat across the image bounds. Pure.
pub fn pixel_centers(
    bounds: [f64; 4],
    width: usize,
    height: usize,
) -> impl Iterator<Item = (usize, f64, f64)> {
    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let lon_span = max_lon - min_lon;
    let lat_span = max_lat - min_lat;
    (0..height).flat_map(move |y| {
        (0..width).map(move |x| {
            let lon = min_lon + lon_span * ((x as f64 + 0.5) / width as f64);
            let lat = max_lat - lat_span * ((y as f64 + 0.5) / height as f64);
            (y * width + x, lon, lat)
        })
    })
}

/// Ray-casting point-in-polygon test on a lon/lat ring. Pure.
pub fn point_in_polygon(lon: f64, lat: f64, ring: &[(f64, f64)]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        let straddles = (yi > lat) != (yj > lat);
        if straddles {
            let x_cross = (xj - xi) * (lat - yi) / (yj - yi) + xi;
            if lon < x_cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Pixel selector for a polygon, using pixel centers. Pure.
pub fn polygon_pixel_mask(
    bounds: [f64; 4],
    width: usize,
    height: usize,
    ring: &[(f64, f64)],
) -> Vec<bool> {
    let mut keep = vec![false; width * height];
    for (index, lon, lat) in pixel_centers(bounds, width, height) {
        keep[index] = point_in_polygon(lon, lat, ring);
    }
    keep
}

/// Coverage inside one plot polygon (Module 5 §5). Pure.
pub fn calculate_plot_coverage(
    mask: &BinaryMask,
    bounds: [f64; 4],
    plot_id: &str,
    ring: &[(f64, f64)],
    gsd_m: Option<f64>,
    low_threshold: f64,
    medium_threshold: f64,
) -> Result<PlotResult> {
    if ring.len() < 3 {
        return Err(PluginError::Value(format!(
            "plot '{plot_id}' polygon needs at least 3 vertices"
        )));
    }
    let keep = polygon_pixel_mask(bounds, mask.width, mask.height, ring);
    let coverage = calculate_coverage(mask, Some(&keep));
    if coverage.total_pixels == 0 {
        return Err(PluginError::Value(format!(
            "plot '{plot_id}' does not overlap the image extent"
        )));
    }
    Ok(PlotResult {
        plot_id: plot_id.to_string(),
        coverage,
        vegetation_area_sqm: area_metrics(&coverage, gsd_m).0,
        classification: classify_coverage(
            coverage.coverage_percentage,
            low_threshold,
            medium_threshold,
        )?,
    })
}

/// Average / extremes / ranking across plots (Module 5 §6). Pure.
pub fn compare_plots(plots: &[PlotResult]) -> PlotComparison {
    let mut ranked: Vec<&PlotResult> = plots.iter().collect();
    ranked.sort_by(|a, b| {
        b.coverage
            .coverage_percentage
            .partial_cmp(&a.coverage.coverage_percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let average = if ranked.is_empty() {
        None
    } else {
        let sum: f64 = ranked.iter().map(|p| p.coverage.coverage_percentage).sum();
        Some(sum / ranked.len() as f64)
    };
    PlotComparison {
        average_coverage: average,
        highest_plot_id: ranked.first().map(|p| p.plot_id.clone()),
        lowest_plot_id: ranked.last().map(|p| p.plot_id.clone()),
        ranking: ranked.iter().map(|p| p.plot_id.clone()).collect(),
    }
}

// ---------------------------------------------------------------- geojson

/// Pixel window of a cell projected into lon/lat. Pure.
fn cell_ring(cell: &GridCell, bounds: [f64; 4], width: usize, height: usize) -> Vec<Vec<f64>> {
    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let lon_at = |x: usize| min_lon + (max_lon - min_lon) * (x as f64 / width as f64);
    let lat_at = |y: usize| max_lat - (max_lat - min_lat) * (y as f64 / height as f64);
    let corners = [
        (lon_at(cell.x0), lat_at(cell.y0)),
        (lon_at(cell.x1), lat_at(cell.y0)),
        (lon_at(cell.x1), lat_at(cell.y1)),
        (lon_at(cell.x0), lat_at(cell.y1)),
        (lon_at(cell.x0), lat_at(cell.y0)),
    ];
    corners.iter().map(|&(lon, lat)| vec![lon, lat]).collect()
}

/// Zone grid as a GeoJSON FeatureCollection with per-cell attributes. Pure.
pub fn zonation_geojson(
    cells: &[GridCell],
    bounds: [f64; 4],
    width: usize,
    height: usize,
    low_threshold: f64,
    medium_threshold: f64,
) -> Result<Value> {
    let features: Vec<Value> = cells
        .iter()
        .map(|cell| {
            let class = classify_coverage(
                cell.coverage.coverage_percentage,
                low_threshold,
                medium_threshold,
            )
            .unwrap_or_else(|_| "unknown".into());
            json!({
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [cell_ring(cell, bounds, width, height)]
                },
                "properties": {
                    "cell_id": format!("r{}c{}", cell.row + 1, cell.col + 1),
                    "row": cell.row,
                    "col": cell.col,
                    "total_pixels": cell.coverage.total_pixels,
                    "vegetation_pixels": cell.coverage.vegetation_pixels,
                    "coverage_percentage": round6(cell.coverage.coverage_percentage),
                    "classification": class,
                }
            })
        })
        .collect();
    Ok(json!({ "type": "FeatureCollection", "features": features }))
}

/// Plot results as a GeoJSON FeatureCollection. Pure.
pub fn plots_geojson(plots: &[PlotResult], rings: &[(String, Vec<(f64, f64)>)]) -> Value {
    let features: Vec<Value> = plots
        .iter()
        .map(|plot| {
            let ring = rings
                .iter()
                .find(|(id, _)| id == &plot.plot_id)
                .map(|(_, ring)| ring.clone())
                .unwrap_or_default();
            let mut coords: Vec<Vec<f64>> = ring.iter().map(|&(lon, lat)| vec![lon, lat]).collect();
            let closed = coords
                .first()
                .zip(coords.last())
                .is_some_and(|(f, l)| f == l);
            if !closed && !coords.is_empty() {
                coords.push(coords[0].clone());
            }
            json!({
                "type": "Feature",
                "geometry": { "type": "Polygon", "coordinates": [coords] },
                "properties": {
                    "plot_id": plot.plot_id,
                    "total_pixels": plot.coverage.total_pixels,
                    "vegetation_pixels": plot.coverage.vegetation_pixels,
                    "coverage_percentage": round6(plot.coverage.coverage_percentage),
                    "vegetation_area_sqm": plot.vegetation_area_sqm.map(round6),
                    "classification": plot.classification,
                }
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

/// Rounds a float for stable JSON output.
pub fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        value
    }
}

// --------------------------------------------------------------------- I/O

/// Reads an image into normalized RGB. I/O edge.
pub fn load_image(image_path: &str) -> Result<RgbArray> {
    if !Path::new(image_path).is_file() {
        return Err(PluginError::FileNotFound(format!(
            "Image file not found: {image_path}"
        )));
    }
    let img = image::open(image_path).map_err(|e| PluginError::Io(e.to_string()))?;
    rgb_from_dynamic(img)
}

/// Converts a decoded image into a normalized RGB array. Pure.
pub fn rgb_from_dynamic(image: DynamicImage) -> Result<RgbArray> {
    let rgb = image.to_rgb8();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    if width == 0 || height == 0 {
        return Err(PluginError::Value("image has no pixels".into()));
    }
    let pixels = rgb
        .pixels()
        .map(|p| {
            [
                p[0] as f64 / 255.0,
                p[1] as f64 / 255.0,
                p[2] as f64 / 255.0,
            ]
        })
        .collect();
    Ok(RgbArray {
        width,
        height,
        pixels,
    })
}

/// Reads a mask PNG from disk (any non-zero pixel counts as vegetation). I/O edge.
pub fn load_mask_file(mask_path: &str) -> Result<BinaryMask> {
    if !Path::new(mask_path).is_file() {
        return Err(PluginError::FileNotFound(format!(
            "Mask file not found: {mask_path}"
        )));
    }
    let img = image::open(mask_path).map_err(|e| PluginError::Io(e.to_string()))?;
    let luma = img.to_luma8();
    let (width, height) = (luma.width() as usize, luma.height() as usize);
    let data = luma.pixels().map(|p| u8::from(p[0] > 0)).collect();
    Ok(BinaryMask {
        width,
        height,
        data,
    })
}

/// Writes the binary mask as a tinted RGBA PNG (transparent where clear) so
/// the overlay never hides the basemap. I/O edge.
pub fn write_mask_png(mask: &BinaryMask, path: &str) -> Result<()> {
    let mut rgba = Vec::with_capacity(mask.data.len() * 4);
    for &hit in &mask.data {
        if hit == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.extend_from_slice(&[42, 160, 70, 210]);
        }
    }
    RgbaImage::from_raw(mask.width as u32, mask.height as u32, rgba)
        .ok_or_else(|| PluginError::Value("mask size does not match image size".into()))?
        .save(path)
        .map_err(|e| PluginError::Io(e.to_string()))
}

/// Writes pretty JSON to disk. I/O edge.
pub fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| PluginError::Io(e.to_string()))?;
    fs::write(path, text).map_err(|e| PluginError::Io(e.to_string()))
}

// ------------------------------------------------------------ orchestrator

/// Manifest identity for the healthcheck payload.
pub fn load_manifest_metadata() -> (String, String) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest.json");
    let fallback = (
        "vegetation-coverage-analytics".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );
    let Ok(text) = fs::read_to_string(manifest) else {
        return fallback;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return fallback;
    };
    (
        value["metadata"]["id"]
            .as_str()
            .unwrap_or(&fallback.0)
            .to_string(),
        value["metadata"]["version"]
            .as_str()
            .unwrap_or(&fallback.1)
            .to_string(),
    )
}

/// Healthcheck payload printed for `--healthcheck`.
pub fn healthcheck_info() -> Value {
    let (plugin_id, version) = load_manifest_metadata();
    json!({
        "status": "healthy",
        "plugin_id": plugin_id,
        "version": version,
        "runtime": "rust",
        "dependencies": { "image": "available", "serde_json": "available" },
    })
}

fn number_param(parameters: &Value, key: &str) -> Option<f64> {
    match &parameters[key] {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn resolve_threshold(parameters: &Value, method: &str) -> f64 {
    number_param(parameters, "threshold_value").unwrap_or_else(|| default_threshold(method))
}

fn resolve_output_dir(payload: &Value, output_path: &Path) -> Result<PathBuf> {
    let fallback = std::path::absolute(output_path)
        .map_err(|e| PluginError::Io(e.to_string()))?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let raw = payload["output_dir"].as_str().unwrap_or("");
    let dir = if raw.trim().is_empty() {
        fallback
    } else {
        PathBuf::from(raw)
    };
    fs::create_dir_all(&dir).map_err(|e| PluginError::Io(e.to_string()))?;
    Ok(dir)
}

/// Outer ring of an AOI geometry (`Polygon`) as lon/lat pairs. Pure.
fn aoi_ring(payload: &Value) -> Option<Vec<(f64, f64)>> {
    let geometry = &payload["aoi"]["geometry"];
    let coordinates = geometry["coordinates"].as_array()?;
    let ring = coordinates.first()?.as_array()?;
    let points: Vec<(f64, f64)> = ring
        .iter()
        .filter_map(|point| {
            let pair = point.as_array()?;
            Some((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
        })
        .collect();
    (points.len() >= 3).then_some(points)
}

/// Bounds of an AOI: polygon ring or bbox array. Pure.
fn bounds_from_aoi(payload: &Value) -> Option<[f64; 4]> {
    if let Some(ring) = aoi_ring(payload) {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for (lon, lat) in ring {
            bounds[0] = bounds[0].min(lon);
            bounds[1] = bounds[1].min(lat);
            bounds[2] = bounds[2].max(lon);
            bounds[3] = bounds[3].max(lat);
        }
        return Some(bounds);
    }
    let bbox = payload["aoi"]["geometry"]["coordinates"].as_array()?;
    let values: Vec<f64> = bbox.iter().filter_map(Value::as_f64).collect();
    (values.len() == 4).then(|| [values[0], values[1], values[2], values[3]])
}

fn metadata_number(metadata: &Value, key: &str) -> Option<f64> {
    metadata[key].as_f64()
}

/// GSD from parameters first, then metadata (focal length, then assumed HFOV).
fn resolve_gsd(parameters: &Value, metadata: &Value, width: usize, height: usize) -> Option<f64> {
    if let Some(gsd) = number_param(parameters, "gsd_m").filter(|g| *g > 0.0) {
        return Some(gsd);
    }
    let altitude = metadata_number(metadata, "relative_altitude")
        .or_else(|| metadata_number(metadata, "altitude"));
    let altitude = altitude.filter(|a| *a > 0.0)?;
    if let Some(focal) = metadata_number(metadata, "focal_length_35mm") {
        if focal > 0.0 {
            return calculate_gsd(altitude, focal, width, height);
        }
    }
    calculate_gsd_from_hfov(altitude, ASSUMED_HFOV_DEG, width)
}

/// Explicit `[min_lon, min_lat, max_lon, max_lat]` override supplied by the
/// caller (the app sends the visible flight-photo tile so the grid lines up
/// with the imagery instead of the full camera footprint). Pure.
fn parse_bounds_override(parameters: &Value) -> Option<[f64; 4]> {
    let raw = &parameters["image_bounds"];
    let parsed = match raw {
        Value::Array(_) => raw.clone(),
        Value::String(text) if !text.trim().is_empty() => {
            serde_json::from_str::<Value>(text).ok()?
        }
        _ => return None,
    };
    let values: Vec<f64> = parsed
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    if values.len() != 4 || values[2] <= values[0] || values[3] <= values[1] {
        return None;
    }
    Some([values[0], values[1], values[2], values[3]])
}

/// Bounds from the explicit override, else AOI, else GPS + GSD.
fn resolve_bounds(
    payload: &Value,
    parameters: &Value,
    metadata: &Value,
    gsd_m: Option<f64>,
    width: usize,
    height: usize,
) -> Option<[f64; 4]> {
    if let Some(bounds) = parse_bounds_override(parameters) {
        return Some(bounds);
    }
    if let Some(bounds) = bounds_from_aoi(payload) {
        return Some(bounds);
    }
    let gps = &metadata["gps"];
    let lat = gps["lat"].as_f64()?;
    let lon = gps["lon"].as_f64()?;
    bounds_from_gps(lat, lon, gsd_m?, width, height)
}

/// One plot description parsed from the `plots` parameter.
struct PlotRequest {
    plot_id: String,
    ring: Vec<(f64, f64)>,
}

/// Parses the `plots` parameter: a JSON array of `{plot_id, polygon}`. Pure.
fn parse_plots(parameters: &Value) -> std::result::Result<Vec<PlotRequest>, String> {
    let raw = &parameters["plots"];
    let parsed = match raw {
        Value::Array(items) => Value::Array(items.clone()),
        Value::String(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .map_err(|e| format!("plots parameter is not valid JSON: {e}"))?,
        _ => return Ok(Vec::new()),
    };
    let items = parsed
        .as_array()
        .ok_or_else(|| "plots parameter must be a JSON array".to_string())?;
    let mut requests = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let plot_id = item["plot_id"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("plot_{}", index + 1));
        let polygon = item["polygon"]
            .as_array()
            .ok_or_else(|| format!("plot '{plot_id}' is missing a polygon array"))?;
        let ring: Vec<(f64, f64)> = polygon
            .iter()
            .filter_map(|point| {
                let pair = point.as_array()?;
                Some((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
            })
            .collect();
        if ring.len() < 3 {
            return Err(format!(
                "plot '{plot_id}' polygon needs at least 3 lon/lat pairs"
            ));
        }
        requests.push(PlotRequest { plot_id, ring });
    }
    Ok(requests)
}

fn artifact(path: &Path, format: &str, bounds: Option<[f64; 4]>) -> Value {
    let mut map = Map::new();
    map.insert(
        "file_path".into(),
        json!(path.to_string_lossy().into_owned()),
    );
    map.insert("format".into(), json!(format));
    if let Some(b) = bounds {
        map.insert("bounds".into(), json!(b));
    }
    Value::Object(map)
}

fn resolve_mask(payload: &Value, parameters: &Value) -> Result<(BinaryMask, String)> {
    let source = parameters["mask_source"].as_str().unwrap_or("auto");
    match source {
        "file" => {
            let mask_path = parameters["mask_path"].as_str().unwrap_or("");
            if mask_path.trim().is_empty() {
                return Err(PluginError::Value(
                    "mask_source is 'file' but mask_path is empty".into(),
                ));
            }
            Ok((load_mask_file(mask_path)?, "file".into()))
        }
        "auto" => {
            let image_path = payload["target"]["image_path"].as_str().unwrap_or("");
            let method = parameters["method"].as_str().unwrap_or("ExG");
            if !VALID_METHODS.contains(&method) {
                return Err(PluginError::Value(format!("Unknown method: {method}")));
            }
            let rgb = load_image(image_path)?;
            let threshold = resolve_threshold(parameters, method);
            Ok((mask_from_rgb(&rgb, method, threshold)?, "auto".into()))
        }
        other => Err(PluginError::Value(format!(
            "mask_source must be 'auto' or 'file', got {other:?}"
        ))),
    }
}

/// Pipeline from payload to successful result JSON.
fn analyze(payload: &Value, output_path: &Path, start: Instant) -> Result<Value> {
    let exec_id = payload["execution_id"].as_str().unwrap_or("exec_unknown");
    let output_dir = resolve_output_dir(payload, output_path)?;
    let parameters = &payload["parameters"];
    let metadata = &payload["target"]["metadata"];
    let mut warnings: Vec<String> = Vec::new();

    let (mask, mask_source) = resolve_mask(payload, parameters)?;
    if mask.is_empty() {
        return Err(PluginError::Value("vegetation mask is empty".into()));
    }

    let gsd_m = resolve_gsd(parameters, metadata, mask.width, mask.height);
    let bounds = resolve_bounds(
        payload,
        parameters,
        metadata,
        gsd_m,
        mask.width,
        mask.height,
    );
    if gsd_m.is_none() {
        warnings.push("no ground sample distance available; area metrics omitted".into());
    }
    if bounds.is_none() {
        warnings.push("no georeference available; vector artifacts omitted".into());
    }

    let aoi = aoi_ring(payload);
    let keep = aoi
        .as_ref()
        .and_then(|ring| bounds.map(|b| polygon_pixel_mask(b, mask.width, mask.height, ring)));
    if aoi.is_some() && keep.is_none() {
        warnings.push("AOI ignored because the image has no georeference".into());
    }

    let low_threshold = number_param(parameters, "low_threshold").unwrap_or(20.0);
    let medium_threshold = number_param(parameters, "medium_threshold").unwrap_or(50.0);
    let grid_size = number_param(parameters, "grid_size")
        .unwrap_or(4.0)
        .max(1.0) as usize;

    // Areas must follow the extent actually used: with an explicit bounds
    // override the pixel scale comes from that rectangle, not from altitude.
    let area_gsd = bounds
        .filter(|_| parse_bounds_override(parameters).is_some())
        .and_then(|b| {
            let center_lat = (b[1] + b[3]) / 2.0;
            let width_m = (b[2] - b[0]) * METERS_PER_DEGREE_LAT * center_lat.to_radians().cos();
            let height_m = (b[3] - b[1]) * METERS_PER_DEGREE_LAT;
            let pixels = (mask.width * mask.height) as f64;
            (pixels > 0.0).then(|| (width_m * height_m / pixels).sqrt())
        })
        .or(gsd_m);

    let coverage = calculate_coverage(&mask, keep.as_deref());
    let (vegetation_area_sqm, total_area_sqm) = area_metrics(&coverage, area_gsd);
    let classification = classify_coverage(
        coverage.coverage_percentage,
        low_threshold,
        medium_threshold,
    )?;
    let cells = generate_zonation(&mask, keep.as_deref(), grid_size)?;

    let mut plots: Vec<PlotResult> = Vec::new();
    let mut plot_rings: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    match parse_plots(parameters) {
        Ok(requests) => {
            let Some(b) = bounds else {
                if !requests.is_empty() {
                    warnings.push("plots ignored because the image has no georeference".into());
                }
                return finish(
                    exec_id,
                    start,
                    &output_dir,
                    &mask,
                    bounds,
                    &coverage,
                    (vegetation_area_sqm, total_area_sqm),
                    &classification,
                    area_gsd,
                    mask_source,
                    &cells,
                    low_threshold,
                    medium_threshold,
                    Vec::new(),
                    Vec::new(),
                    warnings,
                );
            };
            for request in requests {
                match calculate_plot_coverage(
                    &mask,
                    b,
                    &request.plot_id,
                    &request.ring,
                    area_gsd,
                    low_threshold,
                    medium_threshold,
                ) {
                    Ok(result) => {
                        plots.push(result);
                        plot_rings.push((request.plot_id, request.ring));
                    }
                    Err(e) => warnings.push(e.to_string()),
                }
            }
        }
        Err(message) => warnings.push(message),
    }

    finish(
        exec_id,
        start,
        &output_dir,
        &mask,
        bounds,
        &coverage,
        (vegetation_area_sqm, total_area_sqm),
        &classification,
        area_gsd,
        mask_source,
        &cells,
        low_threshold,
        medium_threshold,
        plots,
        plot_rings,
        warnings,
    )
}

/// Assembles artifacts and the result JSON once every computation succeeded.
#[allow(clippy::too_many_arguments)]
fn finish(
    exec_id: &str,
    start: Instant,
    output_dir: &Path,
    mask: &BinaryMask,
    bounds: Option<[f64; 4]>,
    coverage: &Coverage,
    areas: (Option<f64>, Option<f64>),
    classification: &str,
    gsd_m: Option<f64>,
    mask_source: String,
    cells: &[GridCell],
    low_threshold: f64,
    medium_threshold: f64,
    plots: Vec<PlotResult>,
    plot_rings: Vec<(String, Vec<(f64, f64)>)>,
    warnings: Vec<String>,
) -> Result<Value> {
    let mut artifacts = Map::new();

    if let Some(b) = bounds {
        let zonation_path = output_dir.join(format!("{exec_id}_coverage_zonation.geojson"));
        let zonation = zonation_geojson(
            cells,
            b,
            mask.width,
            mask.height,
            low_threshold,
            medium_threshold,
        )?;
        write_json_file(&zonation_path, &zonation)?;
        artifacts.insert(
            "coverage_zonation".into(),
            artifact(&zonation_path, "application/geo+json", Some(b)),
        );

        if !plots.is_empty() {
            let plots_path = output_dir.join(format!("{exec_id}_plot_coverage.geojson"));
            write_json_file(&plots_path, &plots_geojson(&plots, &plot_rings))?;
            artifacts.insert(
                "plot_coverage".into(),
                artifact(&plots_path, "application/geo+json", Some(b)),
            );
        }
    }

    let mask_path = output_dir.join(format!("{exec_id}_vegetation_mask.png"));
    write_mask_png(mask, &mask_path.to_string_lossy())?;
    artifacts.insert(
        "vegetation_mask".into(),
        artifact(&mask_path, "image/png", bounds),
    );

    let comparison = compare_plots(&plots);
    let status = if warnings.is_empty() {
        "success"
    } else {
        "warning"
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(json!({
        "execution_id": exec_id,
        "status": status,
        "execution_time_ms": duration_ms,
        "metrics": {
            "total_pixels": coverage.total_pixels,
            "vegetation_pixels": coverage.vegetation_pixels,
            "coverage_percentage": round6(coverage.coverage_percentage),
            "vegetation_area_sqm": areas.0.map(round6),
            "total_area_sqm": areas.1.map(round6),
            "classification": classification,
            "gsd_m": gsd_m.map(round6),
            "grid_cells": cells.len(),
            "plots_analyzed": plots.len(),
            "average_plot_coverage": comparison.average_coverage.map(round6),
            "highest_plot_id": comparison.highest_plot_id,
            "lowest_plot_id": comparison.lowest_plot_id,
            "mask_source": mask_source,
        },
        "artifacts": Value::Object(artifacts),
        "error": if warnings.is_empty() { Value::Null } else { json!(warnings.join("; ")) },
    }))
}

fn failure_result(execution_id: &str, duration_ms: u64, error: &str) -> Value {
    json!({
        "execution_id": execution_id,
        "status": "failure",
        "execution_time_ms": duration_ms,
        "metrics": {},
        "artifacts": {},
        "error": error,
    })
}

/// Orchestrator: reads the payload, runs the pipeline, writes the result.
/// Returns the process exit code.
pub fn run_analysis(input_path: &str, output_path: &str) -> i32 {
    let start = Instant::now();
    let out = Path::new(output_path);

    if !Path::new(input_path).exists() {
        let msg = format!("Input payload file not found: {input_path}");
        let _ = write_json_file(out, &failure_result("unknown", 0, &msg));
        return 1;
    }

    let payload = fs::read_to_string(input_path)
        .map_err(|e| PluginError::Io(e.to_string()))
        .and_then(|s| {
            serde_json::from_str::<Value>(&s).map_err(|e| PluginError::Value(e.to_string()))
        });

    let outcome = payload
        .as_ref()
        .map_err(|e| e.to_string())
        .and_then(|p| analyze(p, out, start).map_err(|e| e.to_string()));

    match outcome {
        Ok(result) => match write_json_file(out, &result) {
            Ok(()) => {
                println!(
                    "Analysis completed successfully in {}ms",
                    result["execution_time_ms"]
                );
                0
            }
            Err(e) => {
                eprintln!("Analysis failed: {e}");
                1
            }
        },
        Err(msg) => {
            let exec_id = payload
                .as_ref()
                .ok()
                .and_then(|p| p["execution_id"].as_str())
                .unwrap_or("unknown");
            let duration_ms = start.elapsed().as_millis() as u64;
            let _ = write_json_file(out, &failure_result(exec_id, duration_ms, &msg));
            eprintln!("Analysis failed: {msg}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_from(width: usize, height: usize, data: Vec<u8>) -> BinaryMask {
        BinaryMask {
            width,
            height,
            data,
        }
    }

    fn img(pixels: Vec<[f64; 3]>) -> RgbArray {
        RgbArray {
            width: pixels.len(),
            height: 1,
            pixels,
        }
    }

    #[test]
    fn exg_flags_green_but_not_gray() {
        let rgb = img(vec![[0.04, 0.78, 0.04], [0.4, 0.4, 0.4], [0.0, 0.0, 0.0]]);
        let mask = mask_from_rgb(&rgb, "ExG", 0.1).unwrap();
        assert_eq!(mask.data, vec![1, 0, 0]);
    }

    #[test]
    fn unknown_method_and_out_of_range_threshold_fail() {
        assert!(pixel_index("XYZ", [0.1, 0.2, 0.3]).is_err());
        assert!(apply_threshold(&[0.5], 1.5).is_err());
    }

    #[test]
    fn coverage_counts_and_percentage() {
        let mask = mask_from(4, 1, vec![1, 1, 0, 0]);
        let coverage = calculate_coverage(&mask, None);
        assert_eq!(coverage.total_pixels, 4);
        assert_eq!(coverage.vegetation_pixels, 2);
        assert!((coverage.coverage_percentage - 50.0).abs() < 1e-9);
    }

    #[test]
    fn coverage_respects_selector() {
        let mask = mask_from(4, 1, vec![1, 1, 0, 0]);
        let coverage = calculate_coverage(&mask, Some(&[true, false, true, false]));
        assert_eq!(coverage.total_pixels, 2);
        assert_eq!(coverage.vegetation_pixels, 1);
    }

    #[test]
    fn area_metrics_need_gsd() {
        let coverage = Coverage {
            total_pixels: 100,
            vegetation_pixels: 40,
            coverage_percentage: 40.0,
        };
        assert_eq!(area_metrics(&coverage, None), (None, None));
        let (veg, total) = area_metrics(&coverage, Some(0.5));
        assert_eq!((veg, total), (Some(10.0), Some(25.0)));
    }

    #[test]
    fn classification_boundaries_are_inclusive() {
        assert_eq!(classify_coverage(20.0, 20.0, 50.0).unwrap(), "low");
        assert_eq!(classify_coverage(20.1, 20.0, 50.0).unwrap(), "medium");
        assert_eq!(classify_coverage(50.0, 20.0, 50.0).unwrap(), "medium");
        assert_eq!(classify_coverage(50.1, 20.0, 50.0).unwrap(), "high");
        assert!(classify_coverage(10.0, 60.0, 40.0).is_err());
    }

    #[test]
    fn zonation_covers_every_pixel_once() {
        let mask = mask_from(7, 5, vec![1; 35]);
        let cells = generate_zonation(&mask, None, 3).unwrap();
        assert_eq!(cells.len(), 9);
        let total: usize = cells.iter().map(|c| c.coverage.total_pixels).sum();
        assert_eq!(total, 35);
        assert!(cells
            .iter()
            .all(|c| (c.coverage.coverage_percentage - 100.0).abs() < 1e-9));
    }

    #[test]
    fn zonation_rejects_bad_grid_size() {
        let mask = mask_from(4, 4, vec![0; 16]);
        assert!(generate_zonation(&mask, None, 0).is_err());
        assert!(generate_zonation(&mask, None, 65).is_err());
    }

    #[test]
    fn point_in_polygon_handles_ring_and_outside() {
        let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        assert!(point_in_polygon(1.0, 1.0, &square));
        assert!(!point_in_polygon(3.0, 1.0, &square));
        assert!(!point_in_polygon(1.0, 1.0, &[(0.0, 0.0), (1.0, 1.0)]));
    }

    #[test]
    fn plot_coverage_uses_polygon_area() {
        // 4x4 mask, left half vegetation, right half bare.
        let data: Vec<u8> = (0..16).map(|i| u8::from(i % 4 < 2)).collect();
        let mask = mask_from(4, 4, data);
        let bounds = [0.0, 0.0, 4.0, 4.0];
        let left = [(0.0, 0.0), (2.0, 0.0), (2.0, 4.0), (0.0, 4.0)];
        let result =
            calculate_plot_coverage(&mask, bounds, "left", &left, Some(1.0), 20.0, 50.0).unwrap();
        assert_eq!(result.coverage.total_pixels, 8);
        assert_eq!(result.coverage.vegetation_pixels, 8);
        assert_eq!(result.classification, "high");
        assert_eq!(result.vegetation_area_sqm, Some(8.0));
    }

    #[test]
    fn plot_outside_extent_is_rejected() {
        let mask = mask_from(2, 2, vec![0; 4]);
        let bounds = [0.0, 0.0, 2.0, 2.0];
        let far = [(10.0, 10.0), (11.0, 10.0), (11.0, 11.0), (10.0, 11.0)];
        assert!(calculate_plot_coverage(&mask, bounds, "far", &far, None, 20.0, 50.0).is_err());
    }

    #[test]
    fn compare_plots_ranks_descending() {
        let plot = |id: &str, pct: f64| PlotResult {
            plot_id: id.into(),
            coverage: Coverage {
                total_pixels: 100,
                vegetation_pixels: pct as usize,
                coverage_percentage: pct,
            },
            vegetation_area_sqm: None,
            classification: "low".into(),
        };
        let comparison = compare_plots(&[plot("a", 10.0), plot("b", 60.0), plot("c", 30.0)]);
        assert_eq!(comparison.highest_plot_id.as_deref(), Some("b"));
        assert_eq!(comparison.lowest_plot_id.as_deref(), Some("a"));
        assert_eq!(comparison.ranking, vec!["b", "c", "a"]);
        assert!((comparison.average_coverage.unwrap() - 100.0 / 3.0).abs() < 1e-9);
        assert_eq!(compare_plots(&[]).average_coverage, None);
    }

    #[test]
    fn gsd_fallbacks_behave() {
        assert!(calculate_gsd(0.0, 24.0, 4000, 3000).is_none());
        let gsd = calculate_gsd(100.0, 24.0, 4000, 3000).unwrap();
        assert!(gsd > 0.0);
        let hfov = calculate_gsd_from_hfov(100.0, 60.0, 4000).unwrap();
        assert!((hfov - (2.0 * 100.0 * 30.0_f64.to_radians().tan() / 4000.0)).abs() < 1e-12);
        assert!(calculate_gsd_from_hfov(-1.0, 60.0, 4000).is_none());
    }

    #[test]
    fn bounds_center_on_gps_point() {
        let bounds = bounds_from_gps(0.0, 0.0, 1.0, 100, 50).unwrap();
        assert!((bounds[0] + bounds[2]).abs() < 1e-9);
        assert!((bounds[1] + bounds[3]).abs() < 1e-9);
        assert!(bounds_from_gps(0.0, 0.0, 0.0, 10, 10).is_none());
    }

    #[test]
    fn zonation_geojson_is_a_feature_collection() {
        let mask = mask_from(4, 4, vec![1; 16]);
        let cells = generate_zonation(&mask, None, 2).unwrap();
        let geojson = zonation_geojson(&cells, [0.0, 0.0, 4.0, 4.0], 4, 4, 20.0, 50.0).unwrap();
        assert_eq!(geojson["type"], "FeatureCollection");
        let features = geojson["features"].as_array().unwrap();
        assert_eq!(features.len(), 4);
        assert_eq!(features[0]["geometry"]["type"], "Polygon");
        assert_eq!(features[0]["properties"]["coverage_percentage"], 100.0);
    }

    #[test]
    fn plots_parameter_parses_and_validates() {
        let parameters =
            json!({ "plots": "[{\"plot_id\":\"A\",\"polygon\":[[0,0],[1,0],[1,1]]}]" });
        let requests = parse_plots(&parameters).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].plot_id, "A");

        let broken = json!({ "plots": "[{\"plot_id\":\"A\",\"polygon\":[[0,0]]}]" });
        assert!(parse_plots(&broken).is_err());
        assert!(parse_plots(&json!({ "plots": "" })).unwrap().is_empty());
    }
}
