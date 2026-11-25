//! Module 6 - RGB Vegetation Condition Analysis Plugin
//!
//! Ports the pure-functional Module 6 pipeline (canopy isolation → VARI green
//! intensity → HSL colour statistics → condition classes → chlorosis and
//! necrosis detection) into the platform's plugin contract, plus zonation and
//! per-plot comparison layers.
//!
//! Usage:
//!     main --healthcheck
//!     main --input payload.json --output result.json

use image::RgbaImage;
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

const EPSILON: f64 = 1e-6;
const METERS_PER_DEGREE_LAT: f64 = 111_320.0;
const ASSUMED_HFOV_DEG: f64 = 60.0;

/// Defaults from the Module 6 research brief.
pub const DEFAULT_EXG_THRESHOLD: f64 = 0.04;
pub const DEFAULT_MIN_BRIGHTNESS: f64 = 0.08;
pub const DEFAULT_MAX_BRIGHTNESS: f64 = 0.95;
pub const DEFAULT_VARI_VIGOROUS: f64 = 0.25;
pub const DEFAULT_VARI_MODERATE: f64 = 0.05;
pub const DEFAULT_YELLOWING_HUE_MIN: f64 = 50.0;
pub const DEFAULT_YELLOWING_HUE_MAX: f64 = 75.0;
pub const DEFAULT_YELLOWING_SAT_MIN: f64 = 0.20;
pub const DEFAULT_BROWNING_MIN_RBVI: f64 = 0.15;
pub const DEFAULT_BROWNING_R_G_RATIO: f64 = 0.90;

/// Normalized RGB image: channels in 0..1, row-major.
#[derive(Debug, Clone)]
pub struct RgbArray {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f64; 3]>,
}

/// Binary canopy mask: one byte per pixel, 0 or 1.
#[derive(Debug, Clone)]
pub struct BinaryMask {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// Condition threshold pair (VARI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionThresholds {
    pub vigorous: f64,
    pub moderate: f64,
}

/// Counts and shares of the three canopy condition classes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionStats {
    pub total_vegetation_pixels: usize,
    pub vigorous_pixels: usize,
    pub moderate_pixels: usize,
    pub severe_pixels: usize,
    pub vigorous_pct: f64,
    pub moderate_pct: f64,
    pub severe_pct: f64,
}

/// Canopy colour statistics (RGB means/std plus HSL means).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStats {
    pub mean_r: f64,
    pub mean_g: f64,
    pub mean_b: f64,
    pub std_r: f64,
    pub std_g: f64,
    pub std_b: f64,
    pub mean_hue_deg: f64,
    pub mean_saturation: f64,
}

/// One zonation cell: pixel window plus canopy/condition counts.
#[derive(Debug, Clone)]
pub struct GridCell {
    pub row: usize,
    pub col: usize,
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
    pub canopy_pixels: usize,
    pub total_pixels: usize,
    pub vigorous: usize,
    pub moderate: usize,
    pub severe: usize,
}

/// Per-plot condition summary.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotResult {
    pub plot_id: String,
    pub canopy_pixels: usize,
    pub total_pixels: usize,
    pub mean_vari: f64,
    pub vigorous_pct: f64,
    pub moderate_pct: f64,
    pub severe_pct: f64,
    pub dominant_condition: String,
}

// ------------------------------------------------------------ colour spaces

/// HSL of one normalized RGB pixel: `(hue_deg, saturation, lightness)`.
///
/// Mirrors the Module 6 `rgb_to_hsl_pure` formulation exactly, including the
/// `delta < EPSILON` guard and the final modulo on the hue angle. Pure.
pub fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let cmax = r.max(g).max(b);
    let cmin = r.min(g).min(b);
    let delta = cmax - cmin;
    let lightness = (cmax + cmin) / 2.0;

    let denominator = if lightness <= 0.5 {
        cmax + cmin + EPSILON
    } else {
        2.0 - cmax - cmin + EPSILON
    };
    let saturation = if delta < EPSILON {
        0.0
    } else {
        delta / denominator
    };

    let delta_safe = if delta < EPSILON { EPSILON } else { delta };
    let hue_raw = if delta < EPSILON {
        0.0
    } else if cmax == r {
        60.0 * (((g - b) / delta_safe) % 6.0)
    } else if cmax == g {
        60.0 * (((b - r) / delta_safe) + 2.0)
    } else {
        60.0 * (((r - g) / delta_safe) + 4.0)
    };
    (hue_raw.rem_euclid(360.0), saturation, lightness)
}

/// Visible Atmospherically Resistant Index `(G - R) / (G + R - B)`, clamped to
/// `[-1, 1]`. Pure.
pub fn vari(r: f64, g: f64, b: f64) -> f64 {
    let denominator = g + r - b;
    let safe = if denominator.abs() < EPSILON {
        EPSILON
    } else {
        denominator
    };
    ((g - r) / safe).clamp(-1.0, 1.0)
}

/// Red-Blue Vegetation Index `(R - B) / (R + B)`. Pure.
pub fn rbvi(r: f64, _g: f64, b: f64) -> f64 {
    (r - b) / (r + b + EPSILON)
}

// --------------------------------------------------------- canopy isolation

/// Isolates active canopy with the Module 4 mock adapter: normalized ExG above
/// the threshold, green dominance, and brightness inside the photo range. Pure.
pub fn detect_canopy(
    rgb: &RgbArray,
    exg_threshold: f64,
    min_brightness: f64,
    max_brightness: f64,
) -> BinaryMask {
    let data = rgb
        .pixels
        .iter()
        .map(|&[r, g, b]| {
            let sum = r + g + b + EPSILON;
            let (r_norm, g_norm, b_norm) = (r / sum, g / sum, b / sum);
            let exg = 2.0 * g_norm - r_norm - b_norm;
            let brightness = sum / 3.0;
            u8::from(
                exg > exg_threshold
                    && g > r
                    && brightness > min_brightness
                    && brightness < max_brightness,
            )
        })
        .collect();
    BinaryMask {
        width: rgb.width,
        height: rgb.height,
        data,
    }
}

// ------------------------------------------------------- canopy statistics

/// Metrics restricted to canopy pixels (zeros when the canopy is empty). Pure.
pub fn analyze_canopy_colour(rgb: &RgbArray, canopy: &BinaryMask) -> ColorStats {
    let mut count = 0usize;
    let (mut sum_r, mut sum_g, mut sum_b) = (0.0, 0.0, 0.0);
    let (mut sum_hue, mut sum_sat) = (0.0, 0.0);
    for (index, &[r, g, b]) in rgb.pixels.iter().enumerate() {
        if canopy.data.get(index).copied().unwrap_or(0) == 0 {
            continue;
        }
        count += 1;
        sum_r += r;
        sum_g += g;
        sum_b += b;
        let (hue, saturation, _) = rgb_to_hsl(r, g, b);
        sum_hue += hue;
        sum_sat += saturation;
    }
    if count == 0 {
        return ColorStats {
            mean_r: 0.0,
            mean_g: 0.0,
            mean_b: 0.0,
            std_r: 0.0,
            std_g: 0.0,
            std_b: 0.0,
            mean_hue_deg: 0.0,
            mean_saturation: 0.0,
        };
    }
    let n = count as f64;
    let (mean_r, mean_g, mean_b) = (sum_r / n, sum_g / n, sum_b / n);
    let mut var_r = 0.0;
    let mut var_g = 0.0;
    let mut var_b = 0.0;
    for (index, &[r, g, b]) in rgb.pixels.iter().enumerate() {
        if canopy.data.get(index).copied().unwrap_or(0) == 0 {
            continue;
        }
        var_r += (r - mean_r).powi(2);
        var_g += (g - mean_g).powi(2);
        var_b += (b - mean_b).powi(2);
    }
    ColorStats {
        mean_r,
        mean_g,
        mean_b,
        std_r: (var_r / n).sqrt(),
        std_g: (var_g / n).sqrt(),
        std_b: (var_b / n).sqrt(),
        mean_hue_deg: sum_hue / n,
        mean_saturation: sum_sat / n,
    }
}

// ------------------------------------------------------------- condition

/// Per-pixel VARI grid; background pixels read 0.0. Pure.
pub fn vari_grid(rgb: &RgbArray, canopy: &BinaryMask) -> Vec<f64> {
    rgb.pixels
        .iter()
        .enumerate()
        .map(|(index, &[r, g, b])| {
            if canopy.data.get(index).copied().unwrap_or(0) == 0 {
                0.0
            } else {
                vari(r, g, b)
            }
        })
        .collect()
}

/// Mean VARI restricted to canopy pixels. Pure.
pub fn mean_canopy_vari(grid: &[f64], canopy: &BinaryMask) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for (index, &value) in grid.iter().enumerate() {
        if canopy.data.get(index).copied().unwrap_or(0) == 0 {
            continue;
        }
        sum += value;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Class grid: 0 background, 1 severe, 2 moderate, 3 vigorous. Pure.
pub fn classify_condition(
    grid: &[f64],
    canopy: &BinaryMask,
    thresholds: ConditionThresholds,
) -> Vec<u8> {
    grid.iter()
        .enumerate()
        .map(|(index, &value)| {
            if canopy.data.get(index).copied().unwrap_or(0) == 0 {
                0
            } else if value >= thresholds.vigorous {
                3
            } else if value >= thresholds.moderate {
                2
            } else {
                1
            }
        })
        .collect()
}

/// Counts and shares per condition class. Pure.
pub fn condition_stats(classes: &[u8], canopy: &BinaryMask) -> ConditionStats {
    let total = canopy.data.iter().map(|&v| v as usize).sum::<usize>();
    let mut vigorous = 0usize;
    let mut moderate = 0usize;
    let mut severe = 0usize;
    for (index, &class) in classes.iter().enumerate() {
        if canopy.data.get(index).copied().unwrap_or(0) == 0 {
            continue;
        }
        match class {
            3 => vigorous += 1,
            2 => moderate += 1,
            _ => severe += 1,
        }
    }
    let pct = |count: usize| {
        if total == 0 {
            0.0
        } else {
            (count as f64 / total as f64) * 100.0
        }
    };
    ConditionStats {
        total_vegetation_pixels: total,
        vigorous_pixels: vigorous,
        moderate_pixels: moderate,
        severe_pixels: severe,
        vigorous_pct: pct(vigorous),
        moderate_pct: pct(moderate),
        severe_pct: pct(severe),
    }
}

/// Dominant condition class name. Pure.
pub fn dominant_condition(stats: &ConditionStats) -> &'static str {
    if stats.total_vegetation_pixels == 0 {
        return "none";
    }
    if stats.vigorous_pixels >= stats.moderate_pixels
        && stats.vigorous_pixels >= stats.severe_pixels
    {
        "vigorous"
    } else if stats.moderate_pixels >= stats.severe_pixels {
        "moderate_stress"
    } else {
        "severe_stress"
    }
}

// ---------------------------------------------------- chlorosis / necrosis

/// Chlorotic canopy pixels: hue inside the yellow window with enough
/// saturation. Returns the mask and its share of the canopy. Pure.
pub fn detect_yellowing(
    rgb: &RgbArray,
    canopy: &BinaryMask,
    hue_min: f64,
    hue_max: f64,
    sat_min: f64,
) -> (BinaryMask, f64) {
    let data = rgb
        .pixels
        .iter()
        .enumerate()
        .map(|(index, &[r, g, b])| {
            if canopy.data.get(index).copied().unwrap_or(0) == 0 {
                return 0;
            }
            let (hue, saturation, _) = rgb_to_hsl(r, g, b);
            u8::from(hue >= hue_min && hue <= hue_max && saturation > sat_min)
        })
        .collect();
    let mask = BinaryMask {
        width: rgb.width,
        height: rgb.height,
        data,
    };
    (mask.clone(), percentage_of(&mask, canopy))
}

/// Necrotic canopy pixels: high RBVI with red dominance. Returns the mask and
/// its share of the canopy. Pure.
pub fn detect_browning(
    rgb: &RgbArray,
    canopy: &BinaryMask,
    min_rbvi: f64,
    r_g_ratio: f64,
) -> (BinaryMask, f64) {
    let data = rgb
        .pixels
        .iter()
        .enumerate()
        .map(|(index, &[r, g, b])| {
            if canopy.data.get(index).copied().unwrap_or(0) == 0 {
                return 0;
            }
            u8::from(rbvi(r, g, b) > min_rbvi && r > r_g_ratio * g)
        })
        .collect();
    let mask = BinaryMask {
        width: rgb.width,
        height: rgb.height,
        data,
    };
    (mask.clone(), percentage_of(&mask, canopy))
}

/// `hits` as a percentage of canopy pixels. Pure.
pub fn percentage_of(hits: &BinaryMask, canopy: &BinaryMask) -> f64 {
    let total = canopy.data.iter().map(|&v| v as usize).sum::<usize>();
    if total == 0 {
        return 0.0;
    }
    let found = hits.data.iter().map(|&v| v as usize).sum::<usize>();
    (found as f64 / total as f64) * 100.0
}

// ---------------------------------------------------------------- zonation

/// Splits the image into `grid_size` × `grid_size` cells and counts canopy and
/// class pixels per cell. Pure.
pub fn generate_zonation(
    canopy: &BinaryMask,
    classes: &[u8],
    grid_size: usize,
) -> Result<Vec<GridCell>> {
    if grid_size == 0 || grid_size > 64 {
        return Err(PluginError::Value(format!(
            "grid_size must be between 1 and 64, got {grid_size}"
        )));
    }
    if canopy.width == 0 || canopy.height == 0 {
        return Err(PluginError::Value("image has no pixels".into()));
    }
    let bounds = |total: usize, part: usize, index: usize| {
        (index * total / part, (index + 1) * total / part)
    };
    let mut cells = Vec::with_capacity(grid_size * grid_size);
    for row in 0..grid_size {
        let (y0, y1) = bounds(canopy.height, grid_size, row);
        for col in 0..grid_size {
            let (x0, x1) = bounds(canopy.width, grid_size, col);
            let mut cell = GridCell {
                row,
                col,
                x0,
                y0,
                x1,
                y1,
                canopy_pixels: 0,
                total_pixels: 0,
                vigorous: 0,
                moderate: 0,
                severe: 0,
            };
            for y in y0..y1 {
                for x in x0..x1 {
                    let index = y * canopy.width + x;
                    cell.total_pixels += 1;
                    if canopy.data[index] == 0 {
                        continue;
                    }
                    cell.canopy_pixels += 1;
                    match classes[index] {
                        3 => cell.vigorous += 1,
                        2 => cell.moderate += 1,
                        _ => cell.severe += 1,
                    }
                }
            }
            cells.push(cell);
        }
    }
    Ok(cells)
}

/// Dominant class of one cell as `(name, share_pct)`. Pure.
pub fn cell_dominant(cell: &GridCell) -> (&'static str, f64) {
    if cell.canopy_pixels == 0 {
        return ("none", 0.0);
    }
    let pick = [
        ("vigorous", cell.vigorous),
        ("moderate_stress", cell.moderate),
        ("severe_stress", cell.severe),
    ]
    .into_iter()
    .max_by_key(|&(_, count)| count)
    .unwrap_or(("none", 0));
    (pick.0, (pick.1 as f64 / cell.canopy_pixels as f64) * 100.0)
}

// ------------------------------------------------------ georeferencing

/// `[min_lon, min_lat, max_lon, max_lat]` from an explicit override. Pure.
pub fn parse_bounds_override(parameters: &Value) -> Option<[f64; 4]> {
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
    (values.len() == 4 && values[2] > values[0] && values[3] > values[1])
        .then(|| [values[0], values[1], values[2], values[3]])
}

/// GSD from altitude and an assumed horizontal field of view. Pure.
pub fn gsd_from_hfov(altitude_m: f64, hfov_deg: f64, width_px: usize) -> Option<f64> {
    if altitude_m <= 0.0 || width_px == 0 || !(1.0..179.0).contains(&hfov_deg) {
        return None;
    }
    Some((2.0 * altitude_m * (hfov_deg / 2.0).to_radians().tan()) / width_px as f64)
}

/// `[min_lon, min_lat, max_lon, max_lat]` centered on a GPS point. Pure.
pub fn bounds_from_gps(
    lat: f64,
    lon: f64,
    gsd_m: f64,
    width_px: usize,
    height_px: usize,
) -> Option<[f64; 4]> {
    if gsd_m <= 0.0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let ground_width = width_px as f64 * gsd_m;
    let ground_height = height_px as f64 * gsd_m;
    let delta_lat = (ground_height / 2.0) / METERS_PER_DEGREE_LAT;
    let delta_lon = (ground_width / 2.0) / (METERS_PER_DEGREE_LAT * lat.to_radians().cos());
    Some([
        lon - delta_lon,
        lat - delta_lat,
        lon + delta_lon,
        lat + delta_lat,
    ])
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
        if (yi > lat) != (yj > lat) {
            let cross = (xj - xi) * (lat - yi) / (yj - yi) + xi;
            if lon < cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Pixel selector for a lon/lat polygon, using pixel centers. Pure.
pub fn polygon_pixel_mask(
    bounds: [f64; 4],
    width: usize,
    height: usize,
    ring: &[(f64, f64)],
) -> Vec<bool> {
    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let lon_span = max_lon - min_lon;
    let lat_span = max_lat - min_lat;
    let mut keep = vec![false; width * height];
    for y in 0..height {
        let lat = max_lat - lat_span * ((y as f64 + 0.5) / height as f64);
        for x in 0..width {
            let lon = min_lon + lon_span * ((x as f64 + 0.5) / width as f64);
            keep[y * width + x] = point_in_polygon(lon, lat, ring);
        }
    }
    keep
}

/// Condition summary for one plot (Module 6 plot comparison). Pure.
pub fn plot_condition(
    plot_id: &str,
    canopy: &BinaryMask,
    classes: &[u8],
    grid: &[f64],
    keep: &[bool],
) -> PlotResult {
    let mut total = 0usize;
    let mut canopy_pixels = 0usize;
    let (mut vigorous, mut moderate, mut severe) = (0usize, 0usize, 0usize);
    let mut vari_sum = 0.0;
    for (index, &selected) in keep.iter().enumerate() {
        if !selected {
            continue;
        }
        total += 1;
        if canopy.data.get(index).copied().unwrap_or(0) == 0 {
            continue;
        }
        canopy_pixels += 1;
        vari_sum += grid.get(index).copied().unwrap_or(0.0);
        match classes.get(index).copied().unwrap_or(0) {
            3 => vigorous += 1,
            2 => moderate += 1,
            _ => severe += 1,
        }
    }
    let pct = |count: usize| {
        if canopy_pixels == 0 {
            0.0
        } else {
            (count as f64 / canopy_pixels as f64) * 100.0
        }
    };
    let stats = ConditionStats {
        total_vegetation_pixels: canopy_pixels,
        vigorous_pixels: vigorous,
        moderate_pixels: moderate,
        severe_pixels: severe,
        vigorous_pct: pct(vigorous),
        moderate_pct: pct(moderate),
        severe_pct: pct(severe),
    };
    PlotResult {
        plot_id: plot_id.to_string(),
        canopy_pixels,
        total_pixels: total,
        mean_vari: if canopy_pixels == 0 {
            0.0
        } else {
            vari_sum / canopy_pixels as f64
        },
        vigorous_pct: stats.vigorous_pct,
        moderate_pct: stats.moderate_pct,
        severe_pct: stats.severe_pct,
        dominant_condition: dominant_condition(&stats).to_string(),
    }
}

/// Sorts plots by mean VARI, returning `(highest, lowest)` ids. Pure.
pub fn compare_plots(plots: &[PlotResult]) -> (Option<String>, Option<String>) {
    if plots.is_empty() {
        return (None, None);
    }
    let mut ranked: Vec<&PlotResult> = plots.iter().collect();
    ranked.sort_by(|a, b| {
        b.mean_vari
            .partial_cmp(&a.mean_vari)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (
        ranked.first().map(|p| p.plot_id.clone()),
        ranked.last().map(|p| p.plot_id.clone()),
    )
}

// ---------------------------------------------------------------- geojson

fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        value
    }
}

fn cell_ring(cell: &GridCell, bounds: [f64; 4], width: usize, height: usize) -> Vec<Vec<f64>> {
    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let lon_at = |x: usize| min_lon + (max_lon - min_lon) * (x as f64 / width as f64);
    let lat_at = |y: usize| max_lat - (max_lat - min_lat) * (y as f64 / height as f64);
    [
        (lon_at(cell.x0), lat_at(cell.y0)),
        (lon_at(cell.x1), lat_at(cell.y0)),
        (lon_at(cell.x1), lat_at(cell.y1)),
        (lon_at(cell.x0), lat_at(cell.y1)),
        (lon_at(cell.x0), lat_at(cell.y0)),
    ]
    .iter()
    .map(|&(lon, lat)| vec![lon, lat])
    .collect()
}

/// Condition zonation as a GeoJSON FeatureCollection. Pure.
pub fn zonation_geojson(
    cells: &[GridCell],
    bounds: [f64; 4],
    width: usize,
    height: usize,
) -> Value {
    let features: Vec<Value> = cells
        .iter()
        .map(|cell| {
            let (dominant, share) = cell_dominant(cell);
            json!({
                "type": "Feature",
                "geometry": { "type": "Polygon", "coordinates": [cell_ring(cell, bounds, width, height)] },
                "properties": {
                    "cell_id": format!("r{}c{}", cell.row + 1, cell.col + 1),
                    "row": cell.row,
                    "col": cell.col,
                    "total_pixels": cell.total_pixels,
                    "canopy_pixels": cell.canopy_pixels,
                    "canopy_pct": round6(if cell.total_pixels == 0 { 0.0 } else { (cell.canopy_pixels as f64 / cell.total_pixels as f64) * 100.0 }),
                    "vigorous_pct": round6(if cell.canopy_pixels == 0 { 0.0 } else { (cell.vigorous as f64 / cell.canopy_pixels as f64) * 100.0 }),
                    "moderate_stress_pct": round6(if cell.canopy_pixels == 0 { 0.0 } else { (cell.moderate as f64 / cell.canopy_pixels as f64) * 100.0 }),
                    "severe_stress_pct": round6(if cell.canopy_pixels == 0 { 0.0 } else { (cell.severe as f64 / cell.canopy_pixels as f64) * 100.0 }),
                    "dominant_condition": dominant,
                    "dominant_pct": round6(share),
                }
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

/// Plot condition as a GeoJSON FeatureCollection. Pure.
pub fn plots_geojson(plots: &[PlotResult], rings: &[(String, Vec<(f64, f64)>)]) -> Value {
    let features: Vec<Value> = plots
        .iter()
        .map(|plot| {
            let ring = rings
                .iter()
                .find(|(id, _)| id == &plot.plot_id)
                .map(|(_, ring)| ring.clone())
                .unwrap_or_default();
            let mut coordinates: Vec<Vec<f64>> =
                ring.iter().map(|&(lon, lat)| vec![lon, lat]).collect();
            let closed = coordinates
                .first()
                .zip(coordinates.last())
                .is_some_and(|(first, last)| first == last);
            if !closed && !coordinates.is_empty() {
                coordinates.push(coordinates[0].clone());
            }
            json!({
                "type": "Feature",
                "geometry": { "type": "Polygon", "coordinates": [coordinates] },
                "properties": {
                    "plot_id": plot.plot_id,
                    "total_pixels": plot.total_pixels,
                    "canopy_pixels": plot.canopy_pixels,
                    "mean_vari": round6(plot.mean_vari),
                    "vigorous_pct": round6(plot.vigorous_pct),
                    "moderate_stress_pct": round6(plot.moderate_pct),
                    "severe_stress_pct": round6(plot.severe_pct),
                    "dominant_condition": plot.dominant_condition,
                }
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

// --------------------------------------------------------------------- I/O

/// Reads an image into normalized RGB. I/O edge.
pub fn load_image(image_path: &str) -> Result<RgbArray> {
    if !Path::new(image_path).is_file() {
        return Err(PluginError::FileNotFound(format!(
            "Image file not found: {image_path}"
        )));
    }
    let rgb = image::open(image_path)
        .map_err(|e| PluginError::Io(e.to_string()))?
        .to_rgb8();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    if width == 0 || height == 0 {
        return Err(PluginError::Value("image has no pixels".into()));
    }
    Ok(RgbArray {
        width,
        height,
        pixels: rgb
            .pixels()
            .map(|p| {
                [
                    p[0] as f64 / 255.0,
                    p[1] as f64 / 255.0,
                    p[2] as f64 / 255.0,
                ]
            })
            .collect(),
    })
}

/// Reads a canopy mask PNG (any non-zero pixel counts as canopy). I/O edge.
pub fn load_mask_file(mask_path: &str) -> Result<BinaryMask> {
    if !Path::new(mask_path).is_file() {
        return Err(PluginError::FileNotFound(format!(
            "Mask file not found: {mask_path}"
        )));
    }
    let luma = image::open(mask_path)
        .map_err(|e| PluginError::Io(e.to_string()))?
        .to_luma8();
    Ok(BinaryMask {
        width: luma.width() as usize,
        height: luma.height() as usize,
        data: luma.pixels().map(|p| u8::from(p[0] > 0)).collect(),
    })
}

/// Writes the condition class map with the brief's palette. I/O edge.
pub fn write_condition_png(classes: &[u8], width: usize, height: usize, path: &str) -> Result<()> {
    let mut rgba = Vec::with_capacity(classes.len() * 4);
    for &class in classes {
        let pixel: [u8; 4] = match class {
            3 => [46, 160, 67, 210],  // vigorous: green
            2 => [230, 180, 40, 210], // moderate stress: amber
            1 => [200, 60, 40, 210],  // severe stress: red
            _ => [0, 0, 0, 0],        // background: transparent
        };
        rgba.extend_from_slice(&pixel);
    }
    RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| PluginError::Value("class map size does not match image size".into()))?
        .save(path)
        .map_err(|e| PluginError::Io(e.to_string()))
}

/// Writes a binary mask as a tinted RGBA PNG: transparent where clear, the
/// given colour where set, so the overlay never hides the basemap. I/O edge.
pub fn write_mask_png(mask: &BinaryMask, path: &str, tint: [u8; 3]) -> Result<()> {
    let mut rgba = Vec::with_capacity(mask.data.len() * 4);
    for &hit in &mask.data {
        if hit == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.extend_from_slice(&[tint[0], tint[1], tint[2], 220]);
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
        "vegetation-condition-analysis".to_string(),
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

fn number_param(parameters: &Value, key: &str, fallback: f64) -> f64 {
    match &parameters[key] {
        Value::Number(n) => n.as_f64().unwrap_or(fallback),
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(fallback),
        _ => fallback,
    }
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

/// Outer ring of an AOI geometry as lon/lat pairs. Pure.
fn aoi_ring(payload: &Value) -> Option<Vec<(f64, f64)>> {
    let coordinates = payload["aoi"]["geometry"]["coordinates"].as_array()?;
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

/// Bounds from explicit override, else AOI, else GPS + assumed GSD.
fn resolve_bounds(
    payload: &Value,
    parameters: &Value,
    width: usize,
    height: usize,
) -> Option<[f64; 4]> {
    if let Some(bounds) = parse_bounds_override(parameters) {
        return Some(bounds);
    }
    if let Some(bounds) = bounds_from_aoi(payload) {
        return Some(bounds);
    }
    let metadata = &payload["target"]["metadata"];
    let altitude = metadata["relative_altitude"]
        .as_f64()
        .or_else(|| metadata["altitude"].as_f64())?;
    let gsd = gsd_from_hfov(altitude, ASSUMED_HFOV_DEG, width)?;
    let lat = metadata["gps"]["lat"].as_f64()?;
    let lon = metadata["gps"]["lon"].as_f64()?;
    bounds_from_gps(lat, lon, gsd, width, height)
}

/// One plot request parsed from the `plots` parameter.
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
            .map_err(|e| format!("plots is not valid JSON: {e}"))?,
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

/// Resolves the canopy mask: from the image (auto) or from a Mask PNG (file).
fn resolve_canopy(parameters: &Value, rgb: &RgbArray) -> Result<BinaryMask> {
    let source = parameters["mask_source"].as_str().unwrap_or("auto");
    match source {
        "auto" => Ok(detect_canopy(
            rgb,
            number_param(parameters, "exg_threshold", DEFAULT_EXG_THRESHOLD),
            number_param(parameters, "min_brightness", DEFAULT_MIN_BRIGHTNESS),
            number_param(parameters, "max_brightness", DEFAULT_MAX_BRIGHTNESS),
        )),
        "file" => {
            let mask_path = parameters["mask_path"].as_str().unwrap_or("");
            if mask_path.trim().is_empty() {
                return Err(PluginError::Value(
                    "mask_source is 'file' but mask_path is empty".into(),
                ));
            }
            let mask = load_mask_file(mask_path)?;
            if mask.width != rgb.width || mask.height != rgb.height {
                return Err(PluginError::Value(format!(
                    "mask is {}×{} but the image is {}×{}",
                    mask.width, mask.height, rgb.width, rgb.height
                )));
            }
            Ok(mask)
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
    let mut warnings: Vec<String> = Vec::new();

    let rgb = load_image(payload["target"]["image_path"].as_str().unwrap_or(""))?;
    let canopy = resolve_canopy(parameters, &rgb)?;
    let bounds = resolve_bounds(payload, parameters, rgb.width, rgb.height);
    if bounds.is_none() {
        warnings.push("no georeference available; vector artifacts omitted".into());
    }

    // AOI restriction (all metrics then describe the AOI only).
    let aoi = aoi_ring(payload);
    let keep = aoi
        .as_ref()
        .and_then(|ring| bounds.map(|b| polygon_pixel_mask(b, rgb.width, rgb.height, ring)));
    if aoi.is_some() && keep.is_none() {
        warnings.push("AOI ignored because the image has no georeference".into());
    }
    let canopy = if let Some(selector) = &keep {
        BinaryMask {
            width: canopy.width,
            height: canopy.height,
            data: canopy
                .data
                .iter()
                .zip(selector.iter())
                .map(|(&c, &k)| if k { c } else { 0 })
                .collect(),
        }
    } else {
        canopy
    };

    let thresholds = ConditionThresholds {
        vigorous: number_param(parameters, "threshold_vigorous", DEFAULT_VARI_VIGOROUS),
        moderate: number_param(parameters, "threshold_moderate", DEFAULT_VARI_MODERATE),
    };
    if thresholds.moderate > thresholds.vigorous {
        return Err(PluginError::Value(format!(
            "threshold_moderate ({}) must not exceed threshold_vigorous ({})",
            thresholds.moderate, thresholds.vigorous
        )));
    }

    let grid = vari_grid(&rgb, &canopy);
    let classes = classify_condition(&grid, &canopy, thresholds);
    let stats = condition_stats(&classes, &canopy);
    let colours = analyze_canopy_colour(&rgb, &canopy);
    let mean_vari = mean_canopy_vari(&grid, &canopy);

    let (yellow_mask, yellow_pct) = detect_yellowing(
        &rgb,
        &canopy,
        number_param(parameters, "yellowing_hue_min", DEFAULT_YELLOWING_HUE_MIN),
        number_param(parameters, "yellowing_hue_max", DEFAULT_YELLOWING_HUE_MAX),
        number_param(parameters, "yellowing_sat_min", DEFAULT_YELLOWING_SAT_MIN),
    );
    let (brown_mask, brown_pct) = detect_browning(
        &rgb,
        &canopy,
        number_param(parameters, "browning_min_rbvi", DEFAULT_BROWNING_MIN_RBVI),
        number_param(parameters, "browning_r_g_ratio", DEFAULT_BROWNING_R_G_RATIO),
    );

    let grid_size = number_param(parameters, "grid_size", 4.0).max(1.0) as usize;
    let cells = generate_zonation(&canopy, &classes, grid_size)?;

    let mut plots: Vec<PlotResult> = Vec::new();
    let mut plot_rings: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    match parse_plots(parameters) {
        Ok(requests) => {
            if let Some(b) = bounds {
                for request in requests {
                    let selector = polygon_pixel_mask(b, rgb.width, rgb.height, &request.ring);
                    if !selector.iter().any(|&k| k) {
                        warnings.push(format!(
                            "plot '{}' does not overlap the image extent",
                            request.plot_id
                        ));
                        continue;
                    }
                    plots.push(plot_condition(
                        &request.plot_id,
                        &canopy,
                        &classes,
                        &grid,
                        &selector,
                    ));
                    plot_rings.push((request.plot_id, request.ring));
                }
            } else if !requests.is_empty() {
                warnings.push("plots ignored because the image has no georeference".into());
            }
        }
        Err(message) => warnings.push(message),
    }

    finish(
        exec_id,
        start,
        &output_dir,
        &rgb,
        bounds,
        &classes,
        stats,
        colours,
        mean_vari,
        yellow_pct,
        brown_pct,
        &yellow_mask,
        &brown_mask,
        &cells,
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
    rgb: &RgbArray,
    bounds: Option<[f64; 4]>,
    classes: &[u8],
    stats: ConditionStats,
    colours: ColorStats,
    mean_vari: f64,
    yellow_pct: f64,
    brown_pct: f64,
    yellow_mask: &BinaryMask,
    brown_mask: &BinaryMask,
    cells: &[GridCell],
    plots: Vec<PlotResult>,
    plot_rings: Vec<(String, Vec<(f64, f64)>)>,
    warnings: Vec<String>,
) -> Result<Value> {
    let mut artifacts = Map::new();

    let class_path = output_dir.join(format!("{exec_id}_condition_map.png"));
    write_condition_png(
        classes,
        rgb.width,
        rgb.height,
        &class_path.to_string_lossy(),
    )?;
    artifacts.insert(
        "condition_map".into(),
        artifact(&class_path, "image/png", bounds),
    );

    let yellow_path = output_dir.join(format!("{exec_id}_yellowing_mask.png"));
    write_mask_png(yellow_mask, &yellow_path.to_string_lossy(), [240, 200, 40])?;
    artifacts.insert(
        "yellowing_mask".into(),
        artifact(&yellow_path, "image/png", bounds),
    );

    let brown_path = output_dir.join(format!("{exec_id}_browning_mask.png"));
    write_mask_png(brown_mask, &brown_path.to_string_lossy(), [178, 84, 34])?;
    artifacts.insert(
        "browning_mask".into(),
        artifact(&brown_path, "image/png", bounds),
    );

    if let Some(b) = bounds {
        let zonation_path = output_dir.join(format!("{exec_id}_condition_zonation.geojson"));
        write_json_file(
            &zonation_path,
            &zonation_geojson(cells, b, rgb.width, rgb.height),
        )?;
        artifacts.insert(
            "condition_zonation".into(),
            artifact(&zonation_path, "application/geo+json", Some(b)),
        );

        if !plots.is_empty() {
            let plots_path = output_dir.join(format!("{exec_id}_plot_condition.geojson"));
            write_json_file(&plots_path, &plots_geojson(&plots, &plot_rings))?;
            artifacts.insert(
                "plot_condition".into(),
                artifact(&plots_path, "application/geo+json", Some(b)),
            );
        }
    }

    let (highest, lowest) = compare_plots(&plots);
    let duration_ms = start.elapsed().as_millis() as u64;
    let status = if warnings.is_empty() {
        "success"
    } else {
        "warning"
    };
    Ok(json!({
        "execution_id": exec_id,
        "status": status,
        "execution_time_ms": duration_ms,
        "metrics": {
            "total_pixels": rgb.width * rgb.height,
            "vegetation_pixels": stats.total_vegetation_pixels,
            "vegetation_coverage_pct": round6(if rgb.width * rgb.height == 0 { 0.0 } else { (stats.total_vegetation_pixels as f64 / (rgb.width * rgb.height) as f64) * 100.0 }),
            "mean_vari": round6(mean_vari),
            "mean_r": round6(colours.mean_r),
            "mean_g": round6(colours.mean_g),
            "mean_b": round6(colours.mean_b),
            "std_r": round6(colours.std_r),
            "std_g": round6(colours.std_g),
            "std_b": round6(colours.std_b),
            "mean_hue_deg": round6(colours.mean_hue_deg),
            "mean_saturation": round6(colours.mean_saturation),
            "vigorous_pct": round6(stats.vigorous_pct),
            "moderate_stress_pct": round6(stats.moderate_pct),
            "severe_stress_pct": round6(stats.severe_pct),
            "dominant_condition": dominant_condition(&stats),
            "yellowing_pct": round6(yellow_pct),
            "browning_pct": round6(brown_pct),
            "grid_cells": cells.len(),
            "plots_analyzed": plots.len(),
            "highest_plot_id": highest,
            "lowest_plot_id": lowest,
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

    fn img(pixels: Vec<[f64; 3]>) -> RgbArray {
        RgbArray {
            width: pixels.len(),
            height: 1,
            pixels,
        }
    }

    fn mask_from(data: Vec<u8>) -> BinaryMask {
        BinaryMask {
            width: data.len(),
            height: 1,
            data,
        }
    }

    #[test]
    fn hsl_matches_reference_points() {
        let (hue, sat, light) = rgb_to_hsl(1.0, 1.0, 0.0);
        assert!((hue - 60.0).abs() < 1e-6);
        assert!((sat - 1.0).abs() < 1e-6);
        assert!((light - 0.5).abs() < 1e-6);

        let (green_hue, _, _) = rgb_to_hsl(0.0, 1.0, 0.0);
        assert!((green_hue - 120.0).abs() < 1e-6);

        let (grey_hue, grey_sat, _) = rgb_to_hsl(0.5, 0.5, 0.5);
        assert_eq!((grey_hue, grey_sat), (0.0, 0.0));
    }

    #[test]
    fn vari_is_clamped_and_handles_zero_denominator() {
        assert!((vari(0.2, 0.6, 0.1) - 0.4 / 0.7).abs() < 1e-9);
        assert!(vari(0.0, 0.0, 0.0).abs() <= 1.0);
        assert!((-1.0..=1.0).contains(&vari(0.0, 0.001, 0.0)));
    }

    #[test]
    fn canopy_masks_green_but_not_grey_or_bright_glare() {
        let rgb = img(vec![
            [0.2, 0.6, 0.2],    // green -> canopy
            [0.5, 0.5, 0.5],    // grey -> no
            [0.99, 0.99, 0.99], // glare -> no
            [0.02, 0.02, 0.02], // shadow -> no
        ]);
        let mask = detect_canopy(
            &rgb,
            DEFAULT_EXG_THRESHOLD,
            DEFAULT_MIN_BRIGHTNESS,
            DEFAULT_MAX_BRIGHTNESS,
        );
        assert_eq!(mask.data, vec![1, 0, 0, 0]);
    }

    #[test]
    fn conditions_split_at_thresholds() {
        let thresholds = ConditionThresholds {
            vigorous: 0.25,
            moderate: 0.05,
        };
        let grid = vec![0.3, 0.25, 0.1, 0.05, 0.0];
        let canopy = mask_from(vec![1, 1, 1, 1, 1]);
        assert_eq!(
            classify_condition(&grid, &canopy, thresholds),
            vec![3, 3, 2, 2, 1]
        );

        let stats = condition_stats(&classify_condition(&grid, &canopy, thresholds), &canopy);
        assert_eq!(stats.vigorous_pixels, 2);
        assert_eq!(stats.moderate_pixels, 2);
        assert_eq!(stats.severe_pixels, 1);
        assert!((stats.vigorous_pct - 40.0).abs() < 1e-9);
    }

    #[test]
    fn dominant_condition_picks_largest_share() {
        let stats = ConditionStats {
            total_vegetation_pixels: 10,
            vigorous_pixels: 2,
            moderate_pixels: 5,
            severe_pixels: 3,
            vigorous_pct: 20.0,
            moderate_pct: 50.0,
            severe_pct: 30.0,
        };
        assert_eq!(dominant_condition(&stats), "moderate_stress");
        let empty = ConditionStats {
            total_vegetation_pixels: 0,
            vigorous_pixels: 0,
            moderate_pixels: 0,
            severe_pixels: 0,
            vigorous_pct: 0.0,
            moderate_pct: 0.0,
            severe_pct: 0.0,
        };
        assert_eq!(dominant_condition(&empty), "none");
    }

    #[test]
    fn yellowing_detects_yellow_hue_and_ignores_green() {
        let rgb = img(vec![
            [0.8, 0.8, 0.2], // yellow hue 60 -> chlorotic
            [0.2, 0.8, 0.2], // green hue 120 -> not
            [0.5, 0.5, 0.5], // grey, no canopy
        ]);
        let canopy = mask_from(vec![1, 1, 1]);
        let (mask, pct) = detect_yellowing(&rgb, &canopy, 50.0, 75.0, 0.2);
        assert_eq!(mask.data, vec![1, 0, 0]);
        assert!((pct - 100.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn browning_detects_red_dominant_canopy() {
        let rgb = img(vec![
            [0.6, 0.4, 0.2], // red dominant -> necrotic
            [0.2, 0.6, 0.2], // green -> not
        ]);
        let canopy = mask_from(vec![1, 1]);
        let (mask, pct) = detect_browning(&rgb, &canopy, 0.15, 0.9);
        assert_eq!(mask.data, vec![1, 0]);
        assert!((pct - 50.0).abs() < 1e-9);
    }

    #[test]
    fn colour_stats_use_canopy_only() {
        let rgb = img(vec![[1.0, 1.0, 0.0], [0.0, 0.0, 0.0]]);
        let canopy = mask_from(vec![1, 0]);
        let stats = analyze_canopy_colour(&rgb, &canopy);
        assert!((stats.mean_r - 1.0).abs() < 1e-9);
        assert!((stats.mean_hue_deg - 60.0).abs() < 1e-6);
        assert_eq!((stats.std_r, stats.std_g), (0.0, 0.0));
    }

    #[test]
    fn zonation_covers_every_pixel_once() {
        let data = vec![1u8; 35];
        let canopy = BinaryMask {
            width: 7,
            height: 5,
            data: data.clone(),
        };
        let classes = vec![3u8; 35];
        let cells = generate_zonation(&canopy, &classes, 3).unwrap();
        assert_eq!(cells.len(), 9);
        let total: usize = cells.iter().map(|c| c.total_pixels).sum();
        let canopy_total: usize = cells.iter().map(|c| c.canopy_pixels).sum();
        assert_eq!(total, 35);
        assert_eq!(canopy_total, 35);
        assert!(cells.iter().all(|c| cell_dominant(c).0 == "vigorous"));
        assert!(generate_zonation(&canopy, &classes, 0).is_err());
    }

    #[test]
    fn point_in_polygon_and_plot_condition() {
        let ring = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        assert!(point_in_polygon(1.0, 1.0, &ring));
        assert!(!point_in_polygon(3.0, 1.0, &ring));

        let canopy = BinaryMask {
            width: 4,
            height: 4,
            data: vec![1; 16],
        };
        let classes = vec![3u8; 16];
        let grid = vec![0.4f64; 16];
        let keep = polygon_pixel_mask([0.0, 0.0, 4.0, 4.0], 4, 4, &ring).to_vec();
        let plot = plot_condition("A", &canopy, &classes, &grid, &keep);
        assert_eq!(plot.canopy_pixels, 4);
        assert!((plot.mean_vari - 0.4).abs() < 1e-9);
        assert_eq!(plot.dominant_condition, "vigorous");
    }

    #[test]
    fn compare_plots_orders_by_mean_vari() {
        let plot = |id: &str, vari: f64| PlotResult {
            plot_id: id.into(),
            canopy_pixels: 10,
            total_pixels: 10,
            mean_vari: vari,
            vigorous_pct: 0.0,
            moderate_pct: 0.0,
            severe_pct: 0.0,
            dominant_condition: "moderate_stress".into(),
        };
        let (high, low) = compare_plots(&[plot("a", 0.1), plot("b", 0.5), plot("c", 0.3)]);
        assert_eq!(high.as_deref(), Some("b"));
        assert_eq!(low.as_deref(), Some("a"));
        assert_eq!(compare_plots(&[]), (None, None));
    }

    #[test]
    fn bounds_and_gsd_fallbacks_behave() {
        assert!(gsd_from_hfov(0.0, 60.0, 4000).is_none());
        let gsd = gsd_from_hfov(100.0, 60.0, 4000).unwrap();
        assert!((gsd - (2.0 * 100.0 * 30.0_f64.to_radians().tan() / 4000.0)).abs() < 1e-12);
        let bounds = bounds_from_gps(0.0, 0.0, 1.0, 100, 50).unwrap();
        assert!((bounds[0] + bounds[2]).abs() < 1e-9);
        assert_eq!(
            parse_bounds_override(&json!({ "image_bounds": "[1,2,3,4]" })),
            Some([1.0, 2.0, 3.0, 4.0])
        );
        assert!(parse_bounds_override(&json!({ "image_bounds": "[4,2,3,1]" })).is_none());
    }

    #[test]
    fn zonation_geojson_has_one_feature_per_cell() {
        let canopy = BinaryMask {
            width: 4,
            height: 4,
            data: vec![1; 16],
        };
        let classes = vec![2u8; 16];
        let cells = generate_zonation(&canopy, &classes, 2).unwrap();
        let geojson = zonation_geojson(&cells, [0.0, 0.0, 4.0, 4.0], 4, 4);
        assert_eq!(geojson["type"], "FeatureCollection");
        let features = geojson["features"].as_array().unwrap();
        assert_eq!(features.len(), 4);
        assert_eq!(
            features[0]["properties"]["dominant_condition"],
            "moderate_stress"
        );
    }

    #[test]
    fn plots_parameter_parses_and_validates() {
        let ok = json!({ "plots": "[{\"plot_id\":\"A\",\"polygon\":[[0,0],[1,0],[1,1]]}]" });
        assert_eq!(parse_plots(&ok).unwrap().len(), 1);
        let broken = json!({ "plots": "[{\"plot_id\":\"A\",\"polygon\":[[0,0]]}]" });
        assert!(parse_plots(&broken).is_err());
        assert!(parse_plots(&json!({ "plots": "" })).unwrap().is_empty());
    }
}
