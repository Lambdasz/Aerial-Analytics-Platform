//! Module 4 - RGB Vegetation Detection Plugin (inti pipeline).
//!
//! Fungsi-fungsi perhitungan bersifat pure (tanpa mutasi / efek samping).
//! Efek samping (baca/tulis file) hanya ada di `load_image`,
//! `generate_mask_artifact`, `save_rgb_artifact`, dan `run_analysis`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};

/// Citra RGB: piksel disimpan row-major, tiap piksel `[r, g, b]` bertipe f64.
#[derive(Debug, Clone, PartialEq)]
pub struct RgbArray {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f64; 3]>,
}

#[derive(Debug)]
pub enum AnalysisError {
    FileNotFound(String),
    UnsupportedFormat(String),
    Value(String),
    Io(String),
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(m) => write!(f, "FileNotFoundError: {m}"),
            Self::UnsupportedFormat(m) | Self::Value(m) => write!(f, "ValueError: {m}"),
            Self::Io(m) => write!(f, "OSError: {m}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

pub type Result<T> = std::result::Result<T, AnalysisError>;

/// Threshold awal yang disarankan per method.
pub fn default_threshold(method: &str) -> Option<f64> {
    match method {
        "ExG" => Some(0.10),
        "ExR" => Some(0.0),
        "VARI" => Some(0.03),
        _ => None,
    }
}

pub fn load_manifest_metadata() -> (String, String) {
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|d| d.join("manifest.json")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("manifest.json"))),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest.json")),
    ];

    let meta = candidates
        .into_iter()
        .flatten()
        .find_map(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .map(|v| v["metadata"].clone())
        .unwrap_or(Value::Null);

    let pick = |key: &str, default: &str| meta[key].as_str().unwrap_or(default).to_string();
    (
        pick("id", "rgb-vegetation-detection"),
        pick("version", "1.0.0"),
    )
}

/// Healthcheck: semua dependensi ter-link saat compile, jadi selalu "available".
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

// ---------------------------------------------------------------- Module 4

/// Membaca file gambar, konversi jadi array RGB.
pub fn load_image(image_path: &str) -> Result<RgbArray> {
    if !Path::new(image_path).is_file() {
        return Err(AnalysisError::FileNotFound(format!(
            "Image file not found: {image_path}"
        )));
    }
    let rgb = image::open(image_path)
        .map_err(|err| match err {
            image::ImageError::Unsupported(_) | image::ImageError::Decoding(_) => {
                AnalysisError::UnsupportedFormat(format!("Unsupported image format: {image_path}"))
            }
            other => AnalysisError::Io(other.to_string()),
        })?
        .to_rgb8();

    Ok(RgbArray {
        width: rgb.width() as usize,
        height: rgb.height() as usize,
        pixels: rgb
            .pixels()
            .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
            .collect(),
    })
}

/// Index vegetasi satu piksel.
fn pixel_index(method: &str, [r, g, b]: [f64; 3]) -> f64 {
    match method {
        "ExG" | "ExR" => {
            let sum = r + g + b;
            let total = if sum == 0.0 { 1e-6 } else { sum };
            let (rn, gn, bn) = (r / total, g / total, b / total);
            if method == "ExG" {
                2.0 * gn - rn - bn
            } else {
                gn - 1.4 * rn
            }
        }
        // VARI
        _ => {
            let denom = g + r - b;
            if denom.abs() >= 1.0 {
                ((g - r) / denom).clamp(-1.0, 1.0)
            } else {
                0.0
            }
        }
    }
}

/// Menghitung index vegetasi per-pixel (ExG / ExR / VARI). Pure function.
pub fn calculate_vegetation_index(rgb: &RgbArray, method: &str) -> Result<Vec<f64>> {
    match method {
        "ExG" | "ExR" | "VARI" => Ok(rgb.pixels.iter().map(|&p| pixel_index(method, p)).collect()),
        _ => Err(AnalysisError::Value(format!("Unknown method: {method}"))),
    }
}

/// Menerapkan threshold; hasil mask berisi 0/1. Pure function.
pub fn apply_threshold(index: &[f64], threshold_value: f64) -> Result<Vec<u8>> {
    if !(0.0..=1.0).contains(&threshold_value) {
        return Err(AnalysisError::Value(format!(
            "threshold_value must be between 0.0 and 1.0, got {threshold_value:?}"
        )));
    }
    Ok(index
        .iter()
        .map(|&v| u8::from(v > threshold_value))
        .collect())
}

/// Jumlah mentah pixel bernilai 1.
pub fn count_green_pixels(mask: &[u8]) -> usize {
    mask.iter().map(|&m| m as usize).sum()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GreenStats {
    pub mean_green: f64,
    pub std_green: f64,
    pub max_green: f64,
}

/// Statistik channel hijau (mean, std populasi, max). Pure function.
pub fn analyze_green_pixels(rgb: &RgbArray) -> GreenStats {
    let n = rgb.pixels.len() as f64;
    if rgb.pixels.is_empty() {
        return GreenStats {
            mean_green: 0.0,
            std_green: 0.0,
            max_green: 0.0,
        };
    }
    let greens = || rgb.pixels.iter().map(|p| p[1]);
    let mean = greens().sum::<f64>() / n;
    let variance = greens().map(|g| (g - mean).powi(2)).sum::<f64>() / n;
    GreenStats {
        mean_green: mean,
        std_green: variance.sqrt(),
        max_green: greens().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Simpan mask sebagai PNG (nilai piksel 0/255).
pub fn generate_mask_artifact(
    mask: &[u8],
    width: usize,
    height: usize,
    output_path: &str,
) -> Result<String> {
    let visual: Vec<u8> = mask.iter().map(|&m| m * 255).collect();
    image::GrayImage::from_raw(width as u32, height as u32, visual)
        .ok_or_else(|| AnalysisError::Value("Mask size does not match image size".into()))?
        .save(output_path)
        .map_err(|e| AnalysisError::Io(e.to_string()))?;
    Ok(output_path.to_string())
}

/// Overlay warna hijau transparan di atas citra asli pada area vegetasi. Pure function.
/// Hasil: bytes RGB row-major.
pub fn visualize_detection(rgb: &RgbArray, mask: &[u8]) -> Result<Vec<u8>> {
    if mask.len() != rgb.pixels.len() {
        return Err(AnalysisError::Value(format!(
            "Mask size {} does not match image size {}",
            mask.len(),
            rgb.pixels.len()
        )));
    }
    const GREEN_TINT: [f64; 3] = [0.0, 255.0, 0.0];
    const ALPHA: f64 = 0.4;

    Ok(rgb
        .pixels
        .iter()
        .zip(mask)
        .flat_map(|(px, &m)| {
            (0..3).map(move |c| {
                if m != 0 {
                    (px[c] * (1.0 - ALPHA) + GREEN_TINT[c] * ALPHA) as u8
                } else {
                    px[c] as u8
                }
            })
        })
        .collect())
}

/// Simpan bytes RGB sebagai PNG.
pub fn save_rgb_artifact(
    bytes: Vec<u8>,
    width: usize,
    height: usize,
    output_path: &str,
) -> Result<String> {
    image::RgbImage::from_raw(width as u32, height as u32, bytes)
        .ok_or_else(|| AnalysisError::Value("Buffer size does not match image size".into()))?
        .save(output_path)
        .map_err(|e| AnalysisError::Io(e.to_string()))?;
    Ok(output_path.to_string())
}

/// Menghitung [min_lon, min_lat, max_lon, max_lat] dari GeoJSON Polygon. Pure function.
pub fn calculate_bounds_from_geometry(geometry: &Value) -> Result<Vec<f64>> {
    let ring = geometry["coordinates"][0]
        .as_array()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| AnalysisError::Value("Invalid polygon coordinates".into()))?;

    let coords = ring
        .iter()
        .map(|pt| {
            pt[0]
                .as_f64()
                .zip(pt[1].as_f64())
                .ok_or_else(|| AnalysisError::Value("Invalid polygon coordinate point".into()))
        })
        .collect::<Result<Vec<(f64, f64)>>>()?;

    let min = |f: fn(&(f64, f64)) -> f64| coords.iter().map(f).fold(f64::INFINITY, f64::min);
    let max = |f: fn(&(f64, f64)) -> f64| coords.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
    Ok(vec![min(|p| p.0), min(|p| p.1), max(|p| p.0), max(|p| p.1)])
}

/// Menghitung Ground Sample Distance (meter/pixel). Pure function.
pub fn calculate_gsd(
    relative_altitude_m: f64,
    focal_length_35mm: f64,
    image_width_px: usize,
    image_height_px: usize,
) -> f64 {
    const SENSOR_DIAGONAL_35MM_MM: f64 = 43.27;
    let diagonal_px = (image_width_px as f64).hypot(image_height_px as f64);
    (relative_altitude_m * SENSOR_DIAGONAL_35MM_MM) / (focal_length_35mm * diagonal_px)
}

/// GSD from altitude and an assumed horizontal field of view. Pure function.
pub fn calculate_gsd_from_hfov(
    relative_altitude_m: f64,
    hfov_deg: f64,
    image_width_px: usize,
) -> f64 {
    let ground_width_m = 2.0 * relative_altitude_m * (hfov_deg / 2.0).to_radians().tan();
    ground_width_m / image_width_px as f64
}

/// Menghitung [min_lon, min_lat, max_lon, max_lat] dari titik GPS pusat + GSD, tanpa AOI. Pure function.
pub fn calculate_bounds_from_gps(
    lat: f64,
    lon: f64,
    gsd_m_per_px: f64,
    image_width_px: usize,
    image_height_px: usize,
) -> Vec<f64> {
    const METERS_PER_DEGREE_LAT: f64 = 111_320.0;
    let ground_width_m = image_width_px as f64 * gsd_m_per_px;
    let ground_height_m = image_height_px as f64 * gsd_m_per_px;

    let delta_lat = (ground_height_m / 2.0) / METERS_PER_DEGREE_LAT;
    let delta_lon = (ground_width_m / 2.0) / (METERS_PER_DEGREE_LAT * lat.to_radians().cos());

    vec![
        lon - delta_lon,
        lat - delta_lat,
        lon + delta_lon,
        lat + delta_lat,
    ]
}

// ------------------------------------------------------------ Orchestrator

/// Setara `bool(x)` di Python untuk nilai JSON.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn resolve_threshold(parameters: &Value, method: &str) -> Result<f64> {
    let fallback = default_threshold(method).unwrap_or(0.10);
    match &parameters["threshold_value"] {
        Value::Null => Ok(fallback),
        Value::String(s) if s.trim().eq_ignore_ascii_case("auto") => Ok(fallback),
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| AnalysisError::Value(format!("could not convert string to float: {s:?}"))),
        other => other
            .as_f64()
            .ok_or_else(|| AnalysisError::Value(format!("invalid threshold_value: {other}"))),
    }
}

fn resolve_output_dir(payload: &Value, output_path: &Path) -> Result<PathBuf> {
    let fallback = std::path::absolute(output_path)
        .map_err(|e| AnalysisError::Io(e.to_string()))?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();

    let raw = payload["output_dir"].as_str().unwrap_or("");
    let dir = if raw.is_empty() || raw.starts_with("/absolute/") {
        fallback.clone()
    } else if fs::create_dir_all(raw).is_ok() {
        PathBuf::from(raw)
    } else {
        fallback.clone()
    };
    fs::create_dir_all(&dir).map_err(|e| AnalysisError::Io(e.to_string()))?;
    Ok(dir)
}

/// Explicit `[min_lon, min_lat, max_lon, max_lat]` override. The app sends the
/// visible flight-photo tile so masks line up with the imagery on the map.
fn parse_bounds_override(parameters: &Value) -> Option<Vec<f64>> {
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
    (values.len() == 4 && values[2] > values[0] && values[3] > values[1]).then_some(values)
}

fn resolve_bounds(
    payload: &Value,
    meta: &Value,
    width: usize,
    height: usize,
) -> Result<Option<Vec<f64>>> {
    if let Some(bounds) = parse_bounds_override(&payload["parameters"]) {
        return Ok(Some(bounds));
    }
    let geometry = &payload["aoi"]["geometry"];
    let (gps, resolution, altitude, focal) = (
        &meta["gps"],
        &meta["resolution"],
        &meta["relative_altitude"],
        &meta["focal_length_35mm"],
    );

    if truthy(geometry) {
        return calculate_bounds_from_geometry(geometry).map(Some);
    }
    // GPS + altitude are enough: without a focal length fall back to an
    // assumed horizontal field of view, otherwise masks land on disk with no
    // placement and cannot be published to the map.
    if ![gps, resolution, altitude].into_iter().all(truthy) {
        return Ok(None);
    }

    let num = |v: &Value, name: &str| {
        v.as_f64()
            .ok_or_else(|| AnalysisError::Value(format!("invalid {name}")))
    };
    let distance = num(altitude, "relative_altitude")?;
    let gsd = match focal.as_f64() {
        Some(focal) if focal > 0.0 => calculate_gsd(distance, focal, width, height),
        _ => calculate_gsd_from_hfov(distance, 60.0, width),
    };
    Ok(Some(calculate_bounds_from_gps(
        num(&gps["lat"], "gps.lat")?,
        num(&gps["lon"], "gps.lon")?,
        gsd,
        width,
        height,
    )))
}

fn artifact(path: &str, bounds: &Option<Vec<f64>>) -> Value {
    let base = json!({ "file_path": path, "format": "image/png" });
    match (bounds, base) {
        (Some(b), Value::Object(mut m)) => {
            m.insert("bounds".into(), json!(b));
            Value::Object(m)
        }
        (_, base) => base,
    }
}

/// Pipeline lengkap dari payload sampai JSON hasil sukses.
fn analyze(payload: &Value, output_path: &Path, start: Instant) -> Result<Value> {
    let exec_id = payload["execution_id"].as_str().unwrap_or("exec_unknown");
    let output_dir = resolve_output_dir(payload, output_path)?;

    let target = &payload["target"];
    let image_path = target["image_path"].as_str().unwrap_or("");
    let parameters = &payload["parameters"];

    let method = parameters["method"].as_str().unwrap_or("ExG");
    let threshold_value = resolve_threshold(parameters, method)?;

    // Pipeline inti Module 4
    let rgb = load_image(image_path)?;
    let index = calculate_vegetation_index(&rgb, method)?;
    let mask = apply_threshold(&index, threshold_value)?;

    let green_coverage = count_green_pixels(&mask);
    let stats = analyze_green_pixels(&rgb);

    let mask_path = output_dir.join(format!("{exec_id}_vegetation_mask.png"));
    let mask_path =
        generate_mask_artifact(&mask, rgb.width, rgb.height, &mask_path.to_string_lossy())?;

    let overlay_path = output_dir.join(format!("{exec_id}_overlay.png"));
    let overlay_path = save_rgb_artifact(
        visualize_detection(&rgb, &mask)?,
        rgb.width,
        rgb.height,
        &overlay_path.to_string_lossy(),
    )?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let bounds = resolve_bounds(payload, &target["metadata"], rgb.width, rgb.height)?;

    Ok(json!({
        "execution_id": exec_id,
        "status": "success",
        "execution_time_ms": duration_ms,
        "metrics": {
            "green_coverage": green_coverage,
            "mean_green": stats.mean_green,
            "std_green": stats.std_green,
            "max_green": stats.max_green,
            "method_used": method,
            "threshold_used": threshold_value,
        },
        "artifacts": {
            "vegetation_mask": artifact(&mask_path, &bounds),
            "detection_overlay": artifact(&overlay_path, &bounds),
        },
        "error": null,
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

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| AnalysisError::Io(e.to_string()))?;
    fs::write(path, text).map_err(|e| AnalysisError::Io(e.to_string()))
}

/// Orchestrator: baca payload, jalankan pipeline, tulis hasil. Mengembalikan exit code.
pub fn run_analysis(input_path: &str, output_path: &str) -> i32 {
    let start = Instant::now();
    let out = Path::new(output_path);

    if !Path::new(input_path).exists() {
        let msg = format!("Input payload file not found: {input_path}");
        let _ = write_json(out, &failure_result("unknown", 0, &msg));
        return 1;
    }

    let payload = fs::read_to_string(input_path)
        .map_err(|e| AnalysisError::Io(e.to_string()))
        .and_then(|s| {
            serde_json::from_str::<Value>(&s).map_err(|e| AnalysisError::Value(e.to_string()))
        });

    let outcome = payload
        .as_ref()
        .map_err(|e| e.to_string())
        .and_then(|p| analyze(p, out, start).map_err(|e| e.to_string()));

    match outcome {
        Ok(result) => match write_json(out, &result) {
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
            let _ = write_json(out, &failure_result(exec_id, duration_ms, &msg));
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

    #[test]
    fn exg_detects_green_not_gray() {
        let rgb = img(vec![
            [10.0, 200.0, 10.0],
            [100.0, 100.0, 100.0],
            [0.0, 0.0, 0.0],
        ]);
        let idx = calculate_vegetation_index(&rgb, "ExG").unwrap();
        assert_eq!(apply_threshold(&idx, 0.10).unwrap(), vec![1, 0, 0]);
    }

    #[test]
    fn unknown_method_and_bad_threshold_fail() {
        assert!(calculate_vegetation_index(&img(vec![]), "XYZ").is_err());
        assert!(apply_threshold(&[0.5], 1.5).is_err());
    }

    #[test]
    fn vari_is_clamped_and_stable() {
        let rgb = img(vec![[0.0, 10.0, 0.0], [5.0, 5.0, 10.0]]);
        let idx = calculate_vegetation_index(&rgb, "VARI").unwrap();
        assert_eq!(idx, vec![1.0, 0.0]);
    }

    #[test]
    fn green_stats_use_population_std() {
        let s = analyze_green_pixels(&img(vec![[0.0, 2.0, 0.0], [0.0, 4.0, 0.0]]));
        assert_eq!((s.mean_green, s.std_green, s.max_green), (3.0, 1.0, 4.0));
    }

    #[test]
    fn overlay_tints_only_masked_pixels() {
        let rgb = img(vec![[100.0, 100.0, 100.0], [100.0, 100.0, 100.0]]);
        let out = visualize_detection(&rgb, &[1, 0]).unwrap();
        assert_eq!(out, vec![60, 162, 60, 100, 100, 100]);
        assert!(visualize_detection(&rgb, &[1]).is_err());
    }

    #[test]
    fn bounds_from_geometry() {
        let g = json!({"coordinates": [[[1.0, 2.0], [3.0, 0.5], [2.0, 4.0]]]});
        assert_eq!(
            calculate_bounds_from_geometry(&g).unwrap(),
            vec![1.0, 0.5, 3.0, 4.0]
        );
    }

    #[test]
    fn gps_bounds_are_centered() {
        let b = calculate_bounds_from_gps(0.0, 100.0, 0.05, 4000, 3000);
        assert!((b[0] + b[2] - 200.0).abs() < 1e-9 && (b[1] + b[3]).abs() < 1e-9);
    }
}
