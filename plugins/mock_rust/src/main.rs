use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CRC32 and PNG Generation Utilities (Zero C dependencies)
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if (crc & 1) != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn write_chunk(dest: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    dest.extend_from_slice(&(data.len() as u32).to_be_bytes());
    dest.extend_from_slice(tag);
    dest.extend_from_slice(data);
    let mut crc_buf = Vec::with_capacity(4 + data.len());
    crc_buf.extend_from_slice(tag);
    crc_buf.extend_from_slice(data);
    dest.extend_from_slice(&crc32(&crc_buf).to_be_bytes());
}

fn generate_mock_png(path: &Path, width: u32, height: u32) -> io::Result<()> {
    let pixel = [34u8, 139, 34, 200]; // Forest green (#228B22) RGBA
    let row_len = 1 + (width as usize * 4);
    let mut row = Vec::with_capacity(row_len);
    row.push(0u8); // Filter byte: None
    for _ in 0..width {
        row.extend_from_slice(&pixel);
    }

    let mut raw = Vec::with_capacity(row_len * height as usize);
    for _ in 0..height {
        raw.extend_from_slice(&row);
    }

    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut png, b"IHDR", &ihdr);

    // IDAT
    write_chunk(&mut png, b"IDAT", &compressed);

    // IEND
    write_chunk(&mut png, b"IEND", &[]);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(&png)?;
    Ok(())
}

fn generate_mock_geojson(path: &Path, bbox: &[f64; 4]) -> io::Result<()> {
    let [min_lon, min_lat, max_lon, max_lat] = *bbox;
    let geojson = serde_json::json!({
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [
                        [
                            [min_lon, min_lat],
                            [max_lon, min_lat],
                            [max_lon, max_lat],
                            [min_lon, max_lat],
                            [min_lon, min_lat]
                        ]
                    ]
                },
                "properties": {
                    "class": "canopy_cluster",
                    "density_score": 0.88,
                    "estimated_height_m": 8.5
                }
            }
        ]
    });

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, &geojson)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema Payload & Result Data Structures
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ExecutionPayload {
    execution_id: String,
    #[serde(default)]
    output_dir: String,
    #[serde(default)]
    parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ArtifactDescriptor {
    file_path: String,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bounds: Option<Vec<f64>>,
}

#[derive(Debug, Serialize)]
struct ExecutionResult {
    execution_id: String,
    status: String,
    execution_time_ms: u64,
    metrics: HashMap<String, serde_json::Value>,
    artifacts: HashMap<String, ArtifactDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Execution Logic
// ---------------------------------------------------------------------------

fn handle_healthcheck() {
    let health = serde_json::json!({
        "status": "healthy",
        "plugin_id": "tree-canopy-density",
        "version": "1.0.0",
        "runtime": "binary",
        "compiler": "rustc"
    });
    println!("{}", serde_json::to_string(&health).unwrap());
    std::process::exit(0);
}

fn write_error_result(output_path: &Path, exec_id: &str, err_msg: &str) {
    let result = ExecutionResult {
        execution_id: exec_id.to_string(),
        status: "failure".to_string(),
        execution_time_ms: 0,
        metrics: HashMap::new(),
        artifacts: HashMap::new(),
        error: Some(err_msg.to_string()),
    };
    if let Ok(file) = File::create(output_path) {
        let _ = serde_json::to_writer_pretty(file, &result);
    }
    std::process::exit(1);
}

fn run_analysis(input_path: &Path, output_path: &Path) {
    let start = Instant::now();

    let payload_str = match fs::read_to_string(input_path) {
        Ok(content) => content,
        Err(e) => {
            write_error_result(
                output_path,
                "unknown",
                &format!("Failed to read input payload: {}", e),
            );
            return;
        }
    };

    let payload: ExecutionPayload = match serde_json::from_str(&payload_str) {
        Ok(p) => p,
        Err(e) => {
            write_error_result(
                output_path,
                "unknown",
                &format!("Failed to parse input payload JSON: {}", e),
            );
            return;
        }
    };

    // Determine writable output directory, falling back to output_path parent if dummy or unwritable
    let fallback_dir = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let output_dir =
        if payload.output_dir.is_empty() || payload.output_dir.starts_with("/absolute/") {
            fallback_dir
        } else {
            let candidate = PathBuf::from(&payload.output_dir);
            if fs::create_dir_all(&candidate).is_ok() {
                candidate
            } else {
                fallback_dir
            }
        };

    let _ = fs::create_dir_all(&output_dir);

    // Read parameters with defaults
    let canopy_threshold = payload
        .parameters
        .get("canopy_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.4);

    let export_polygons = payload
        .parameters
        .get("export_polygons")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let exec_id = &payload.execution_id;
    let mask_path = output_dir.join(format!("{}_canopy_mask.png", exec_id));
    let geojson_path = output_dir.join(format!("{}_canopy_polygons.geojson", exec_id));
    let bounds = [116.8523, -1.2460, 116.8550, -1.2435];

    let mut artifacts = HashMap::new();

    // Generate simulated raster mask
    if let Err(e) = generate_mock_png(&mask_path, 128, 128) {
        write_error_result(
            output_path,
            exec_id,
            &format!("Failed to generate mask raster: {}", e),
        );
        return;
    }
    artifacts.insert(
        "canopy_mask".to_string(),
        ArtifactDescriptor {
            file_path: mask_path.to_string_lossy().to_string(),
            format: "image/png".to_string(),
            bounds: Some(bounds.to_vec()),
        },
    );

    // Generate simulated vector boundary if enabled
    if export_polygons {
        if let Err(e) = generate_mock_geojson(&geojson_path, &bounds) {
            write_error_result(
                output_path,
                exec_id,
                &format!("Failed to generate GeoJSON: {}", e),
            );
            return;
        }
        artifacts.insert(
            "canopy_polygons".to_string(),
            ArtifactDescriptor {
                file_path: geojson_path.to_string_lossy().to_string(),
                format: "application/geo+json".to_string(),
                bounds: None,
            },
        );
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let simulated_density = (54.2 + (0.4 - canopy_threshold) * 15.0).clamp(5.0, 95.0);

    let mut metrics = HashMap::new();
    metrics.insert(
        "canopy_density_pct".to_string(),
        serde_json::json!((simulated_density * 10.0).round() / 10.0),
    );
    metrics.insert("canopy_area_sqm".to_string(), serde_json::json!(12450.0));
    metrics.insert("tree_cluster_count".to_string(), serde_json::json!(87));

    let result = ExecutionResult {
        execution_id: exec_id.clone(),
        status: "success".to_string(),
        execution_time_ms: elapsed_ms,
        metrics,
        artifacts,
        error: None,
    };

    match File::create(output_path) {
        Ok(file) => {
            if let Err(e) = serde_json::to_writer_pretty(file, &result) {
                eprintln!("Failed to write execution result: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to create output file: {}", e);
            std::process::exit(1);
        }
    }

    println!("Canopy analysis completed in {}ms", elapsed_ms);
    std::process::exit(0);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut is_healthcheck = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--healthcheck" => {
                is_healthcheck = true;
                i += 1;
            }
            "--input" => {
                if i + 1 < args.len() {
                    input_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Missing argument for --input");
                    std::process::exit(1);
                }
            }
            "--output" => {
                if i + 1 < args.len() {
                    output_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Missing argument for --output");
                    std::process::exit(1);
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    if is_healthcheck {
        handle_healthcheck();
    } else if let (Some(input), Some(output)) = (input_path, output_path) {
        run_analysis(&input, &output);
    } else {
        eprintln!(
            "Usage: mock_rust [--healthcheck] [--input <payload.json> --output <result.json>]"
        );
        std::process::exit(1);
    }
}
