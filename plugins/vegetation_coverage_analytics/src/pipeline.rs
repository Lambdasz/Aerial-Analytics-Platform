//! I/O edge, payload parsing, pipeline orchestration, and artifact emission.
//!
//! Keeps impure file operations, environment variables, and timestamps isolated
//! from the pure analytical core.

use image::{DynamicImage, RgbaImage};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::coverage::{area_metrics, calculate_coverage, classify_coverage};
use crate::geo::{aoi_ring, number_param, parse_bounds_override, resolve_bounds, resolve_gsd};
use crate::indices::{default_threshold, mask_from_rgb};
use crate::plots::{
    calculate_plot_coverage, compare_plots, parse_plots, plots_geojson, polygon_pixel_mask,
};
use crate::types::{
    BinaryMask, Coverage, GridCell, PlotResult, PluginError, Result, RgbArray,
    METERS_PER_DEGREE_LAT, VALID_METHODS,
};
use crate::zonation::{generate_zonation, round6, zonation_geojson};

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

pub fn resolve_threshold(parameters: &Value, method: &str) -> f64 {
    number_param(parameters, "threshold_value").unwrap_or_else(|| default_threshold(method))
}

pub fn resolve_output_dir(payload: &Value, output_path: &Path) -> Result<PathBuf> {
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

pub fn artifact(path: &Path, format: &str, bounds: Option<[f64; 4]>) -> Value {
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

pub fn resolve_mask(payload: &Value, parameters: &Value) -> Result<(BinaryMask, String)> {
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
pub fn analyze(payload: &Value, output_path: &Path, start: Instant) -> Result<Value> {
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
        .filter(|_| {
            parse_bounds_override(parameters).is_some() || metadata["gps"]["lat"].as_f64().is_none()
        })
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
pub fn finish(
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

pub fn failure_result(execution_id: &str, duration_ms: u64, error: &str) -> Value {
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
