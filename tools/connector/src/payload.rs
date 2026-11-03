//! Builds `payload.json` per `schemas/execution_payload.schema.json`.
//!
//! Mirrors the app-side builder (`plugin_manager::executor::build_execution_payload`):
//! `single_image` granularity, DB-derived metadata (`resolution`, `gps`,
//! `altitude`, `timestamp`), parameters defaulted from the plugin's
//! `parameters.json` (same source the app UI form uses).

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

use crate::models::ImageRow;

#[derive(Debug, serde::Serialize)]
pub struct ExecutionPayload {
    pub execution_id: String,
    pub session_id: String,
    pub output_dir: String,
    pub target: ExecutionTarget,
    pub parameters: Map<String, Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutionTarget {
    pub granularity: &'static str,
    pub image_path: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// MIME type from the stored format column, restricted to the schema enum.
pub fn mime_type(image: &ImageRow) -> Result<String> {
    match image.format.as_deref() {
        Some("jpeg") | Some("jpg") => Ok("image/jpeg".into()),
        Some("png") => Ok("image/png".into()),
        Some("tiff") | Some("tif") | Some("dng") => Ok("image/tiff".into()),
        other => bail!(
            "unsupported image format {:?} for {} (schema allows jpeg/png/tiff)",
            other,
            image.file_name()
        ),
    }
}

/// Schema metadata block: `resolution`, `gps`, `altitude`, `timestamp`.
/// Requires GPS — plugins that geolocate detections cannot run without it.
fn build_metadata(image: &ImageRow) -> Result<Map<String, Value>> {
    let (Some(lat), Some(lon)) = (image.location_lat, image.location_lon) else {
        bail!(
            "image {} has no GPS coordinates; geotagged images required",
            image.file_name()
        );
    };
    let (Some(width), Some(height)) = (image.width, image.height) else {
        bail!(
            "image {} lacks width/height; re-import needed for metadata",
            image.file_name()
        );
    };
    let mut meta = Map::new();
    meta.insert(
        "resolution".into(),
        Value::Array(vec![width.into(), height.into()]),
    );
    let mut gps = Map::new();
    gps.insert("lat".into(), lat.into());
    gps.insert("lon".into(), lon.into());
    meta.insert("gps".into(), Value::Object(gps));
    if let Some(alt) = image.gps_altitude_m {
        meta.insert("altitude".into(), alt.into());
    }
    if let Some(ts) = &image.captured_at {
        meta.insert("timestamp".into(), Value::String(ts.clone()));
    }
    Ok(meta)
}

/// Parameter defaults from the plugin's `parameters.json`
/// (`parameters[].name` + `parameters[].default`), same file the app UI reads.
pub fn default_parameters(plugin_dir: &Path) -> Result<Map<String, Value>> {
    let path = plugin_dir.join("parameters.json");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: Value =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    let Some(defs) = doc.get("parameters").and_then(Value::as_array) else {
        bail!("{}: no `parameters` array", path.display());
    };
    let mut out = Map::new();
    for p in defs {
        let (Some(name), Some(default)) = (p.get("name").and_then(Value::as_str), p.get("default"))
        else {
            continue;
        };
        out.insert(name.to_string(), default.clone());
    }
    Ok(out)
}

/// Merge user overrides over plugin defaults (deep-shallow: top-level keys only,
/// matching how the app form submits flat parameter values).
pub fn merge_parameters(
    defaults: Map<String, Value>,
    overrides: Map<String, Value>,
) -> Map<String, Value> {
    let mut merged = defaults;
    for (k, v) in overrides {
        merged.insert(k, v);
    }
    merged
}

pub fn build(
    execution_id: &str,
    output_dir: &Path,
    session_id: &str,
    image: &ImageRow,
    parameters: Map<String, Value>,
) -> Result<ExecutionPayload> {
    let path = Path::new(&image.file_path);
    if !path.exists() {
        bail!("image file missing on disk: {}", image.file_path);
    }
    Ok(ExecutionPayload {
        execution_id: execution_id.to_string(),
        session_id: session_id.to_string(),
        output_dir: output_dir.display().to_string(),
        target: ExecutionTarget {
            granularity: "single_image",
            image_path: image.file_path.clone(),
            mime_type: mime_type(image)?,
            metadata: Some(build_metadata(image)?),
        },
        parameters,
    })
}
