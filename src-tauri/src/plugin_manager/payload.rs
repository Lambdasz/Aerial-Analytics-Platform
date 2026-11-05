//! Backward-compatibility shim for execution payload assembly and pre-flight validation.
//!
//! This module is maintained as a compatibility bridge for Module 1 (Image Import & Session Management).
//! Canonical execution payloads and targets are defined in [`crate::plugin_manager::models`]
//! and executed via [`crate::plugin_manager::executor`].

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Source image properties and metadata for pre-flight checking and execution.
///
/// Retained as a compatibility contract for Module 1 (`commands::import`, `modules::module_01`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageMetadata {
    pub path: String,
    pub has_gps: bool,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

impl From<&ImageMetadata> for crate::plugin_manager::models::ExecutionTarget {
    fn from(meta: &ImageMetadata) -> Self {
        let mime_type = match meta.format.to_lowercase().as_str() {
            "jpeg" | "jpg" => "image/jpeg",
            "png" => "image/png",
            "tif" | "tiff" | "dng" => "image/tiff",
            _ => "application/octet-stream",
        };

        let mut metadata_map = serde_json::Map::new();
        if meta.width > 0 && meta.height > 0 {
            metadata_map.insert(
                "resolution".to_string(),
                serde_json::json!([meta.width, meta.height]),
            );
        }

        let metadata_val = if metadata_map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(metadata_map))
        };

        Self {
            granularity: "single_image".to_string(),
            image_path: meta.path.clone(),
            mime_type: mime_type.to_string(),
            metadata: metadata_val,
        }
    }
}

impl From<ImageMetadata> for crate::plugin_manager::models::ExecutionTarget {
    fn from(meta: ImageMetadata) -> Self {
        crate::plugin_manager::models::ExecutionTarget::from(&meta)
    }
}

/// Area of Interest (AOI) geospatial geometry definition (RFC 7946 GeoJSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aoi {
    pub geojson: serde_json::Value,
}

/// Execution input payload written to `payload.json` for plugin execution.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionPayload {
    pub plugin_id: String,
    pub image: ImageMetadata,
    pub aoi: Option<Aoi>,
    pub parameters: serde_json::Value,
    pub output_dir: String,
}

/// Input constraints and requirements defined by the plugin for pre-flight verification.
#[derive(Debug, Deserialize)]
pub struct InputsRequirement {
    pub require_gps: Option<bool>,
    pub allowed_formats: Option<Vec<String>>,
}

pub fn preflight_check(
    image_meta: &ImageMetadata,
    inputs_req: &InputsRequirement,
) -> Result<(), String> {
    if let Some(true) = inputs_req.require_gps {
        if !image_meta.has_gps {
            return Err(
                "Plugin requires GPS data on the image, but the uploaded image has no GPS data."
                    .to_string(),
            );
        }
    }

    if let Some(ref formats) = inputs_req.allowed_formats {
        let ext = image_meta.format.to_lowercase();
        if !formats.iter().any(|f| f.to_lowercase() == ext) {
            return Err(format!(
                "Image format '{}' is not supported by this plugin. Accepted formats: {:?}",
                image_meta.format, formats
            ));
        }
    }

    Ok(())
}

pub fn assemble_payload(payload: &ExecutionPayload, output_dir: &Path) -> Result<PathBuf, String> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    let payload_file_path = output_dir.join("payload.json");
    let json_data = serde_json::to_string_pretty(payload)
        .map_err(|e| format!("Failed to serialise payload JSON: {}", e))?;

    fs::write(&payload_file_path, json_data)
        .map_err(|e| format!("Failed to write payload.json: {}", e))?;

    Ok(payload_file_path)
}
