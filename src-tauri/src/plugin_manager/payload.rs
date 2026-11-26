//! Backward-compatibility shim for execution payload assembly and pre-flight validation.
//!
//! This module is maintained as a compatibility bridge for Module 1 (Image Import & Session Management).
//! Canonical execution payloads and targets are defined in [`crate::plugin_manager::models`]
//! and executed via [`crate::plugin_manager::executor`].
//!
//! ## Purity Boundary
//!
//! Data models, `From` conversions, and [`preflight_check`] are strictly pure and
//! perform no I/O or mutation. [`assemble_payload`] is an impure filesystem I/O
//! boundary that creates directories and writes `payload.json` to disk.

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
    /// Converts a borrowed [`ImageMetadata`] into a canonical [`ExecutionTarget`](crate::plugin_manager::models::ExecutionTarget).
    ///
    /// # Purity
    /// Pure transformation over immutable input; performs no I/O or external side effects.
    fn from(meta: &ImageMetadata) -> Self {
        let mime_type = match meta.format.to_lowercase().as_str() {
            "jpeg" | "jpg" => "image/jpeg",
            "png" => "image/png",
            "tif" | "tiff" | "dng" => "image/tiff",
            _ => "application/octet-stream",
        };

        let metadata = (meta.width > 0 && meta.height > 0)
            .then(|| serde_json::json!({ "resolution": [meta.width, meta.height] }));

        Self {
            granularity: "single_image".to_string(),
            image_path: meta.path.clone(),
            mime_type: mime_type.to_string(),
            metadata,
        }
    }
}

impl From<ImageMetadata> for crate::plugin_manager::models::ExecutionTarget {
    /// Converts an owned [`ImageMetadata`] into a canonical [`ExecutionTarget`](crate::plugin_manager::models::ExecutionTarget).
    ///
    /// # Purity
    /// Pure transformation; delegates to [`From<&ImageMetadata>`](From::from) with no I/O or side effects.
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

/// Validates image metadata against plugin input requirements prior to execution.
///
/// # Purity
/// Pure deterministic validation over immutable references; performs no I/O or mutation.
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

/// Serialises an [`ExecutionPayload`] and writes `payload.json` into `output_dir`.
///
/// # Purity
/// Impure filesystem I/O: creates `output_dir` if absent and writes `payload.json` to disk.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::ExecutionTarget;

    fn sample_image_meta(format: &str, has_gps: bool, width: u32, height: u32) -> ImageMetadata {
        ImageMetadata {
            path: "/tmp/sample.jpg".to_string(),
            has_gps,
            width,
            height,
            format: format.to_string(),
        }
    }

    #[test]
    fn test_execution_target_from_image_metadata_mime_and_resolution() {
        let jpeg = sample_image_meta("JPG", true, 4000, 3000);
        let borrowed_target = ExecutionTarget::from(&jpeg);
        let owned_target = ExecutionTarget::from(jpeg.clone());
        assert_eq!(borrowed_target.granularity, "single_image");
        assert_eq!(borrowed_target.image_path, jpeg.path);
        assert_eq!(borrowed_target.mime_type, "image/jpeg");
        assert_eq!(
            borrowed_target.metadata,
            Some(serde_json::json!({ "resolution": [4000, 3000] }))
        );
        assert_eq!(owned_target.mime_type, "image/jpeg");
        assert_eq!(owned_target.metadata, borrowed_target.metadata);

        assert_eq!(
            ExecutionTarget::from(&sample_image_meta("png", false, 0, 100)).mime_type,
            "image/png"
        );
        let tiff_zero_dim = ExecutionTarget::from(&sample_image_meta("DNG", false, 0, 100));
        assert_eq!(tiff_zero_dim.mime_type, "image/tiff");
        assert_eq!(tiff_zero_dim.metadata, None);

        assert_eq!(
            ExecutionTarget::from(&sample_image_meta("bin", false, 100, 0)).mime_type,
            "application/octet-stream"
        );
    }

    #[test]
    fn test_preflight_check_gps_and_format_constraints() {
        let no_gps_jpg = sample_image_meta("JPEG", false, 800, 600);
        let gps_req = InputsRequirement {
            require_gps: Some(true),
            allowed_formats: Some(vec!["jpg".to_string(), "jpeg".to_string()]),
        };
        let err_gps = preflight_check(&no_gps_jpg, &gps_req).unwrap_err();
        assert!(err_gps.contains("requires GPS"));

        let with_gps_jpg = sample_image_meta("JPEG", true, 800, 600);
        assert!(preflight_check(&with_gps_jpg, &gps_req).is_ok());

        let only_tiff_req = InputsRequirement {
            require_gps: None,
            allowed_formats: Some(vec!["tif".to_string(), "tiff".to_string()]),
        };
        let err_fmt = preflight_check(&with_gps_jpg, &only_tiff_req).unwrap_err();
        assert!(err_fmt.contains("not supported"));

        let permissive_req = InputsRequirement {
            require_gps: Some(false),
            allowed_formats: None,
        };
        assert!(preflight_check(&no_gps_jpg, &permissive_req).is_ok());
    }

    #[test]
    fn test_assemble_payload_creates_dir_and_writes_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_output = temp_dir.path().join("nested").join("run_01");
        let payload = ExecutionPayload {
            plugin_id: "veg-detector".to_string(),
            image: sample_image_meta("png", true, 1920, 1080),
            aoi: Some(Aoi {
                geojson: serde_json::json!({ "type": "Polygon", "coordinates": [] }),
            }),
            parameters: serde_json::json!({ "threshold": 0.5 }),
            output_dir: nested_output.to_string_lossy().into_owned(),
        };

        let written_path = assemble_payload(&payload, &nested_output).unwrap();
        assert_eq!(written_path, nested_output.join("payload.json"));
        let disk_val: ExecutionPayload =
            serde_json::from_str(&fs::read_to_string(&written_path).unwrap()).unwrap();
        assert_eq!(disk_val.plugin_id, "veg-detector");
        assert_eq!(disk_val.image, payload.image);
        assert_eq!(disk_val.parameters, serde_json::json!({ "threshold": 0.5 }));
        assert!(disk_val.aoi.is_some());
    }

    #[test]
    fn test_assemble_payload_errors_when_output_dir_is_existing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let blocking_file = temp_dir.path().join("not_a_dir");
        fs::write(&blocking_file, b"file").unwrap();

        let payload = ExecutionPayload {
            plugin_id: "veg-detector".to_string(),
            image: sample_image_meta("png", false, 100, 100),
            aoi: None,
            parameters: serde_json::Value::Null,
            output_dir: blocking_file.to_string_lossy().into_owned(),
        };

        let err = assemble_payload(&payload, &blocking_file).unwrap_err();
        assert!(err.contains("Failed to write payload.json"));
    }
}
