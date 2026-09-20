use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub path: String,
    pub has_gps: bool,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aoi {
    pub geojson: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionPayload {
    pub plugin_id: String,
    pub image: ImageMetadata,
    pub aoi: Option<Aoi>,
    pub parameters: serde_json::Value,
    pub output_dir: String,
}

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
                "Plugin ini membutuhkan data GPS pada gambar, tetapi gambar yang diunggah tidak memiliki data GPS."
                    .to_string(),
            );
        }
    }

    if let Some(ref formats) = inputs_req.allowed_formats {
        let ext = image_meta.format.to_lowercase();
        if !formats.iter().any(|f| f.to_lowercase() == ext) {
            return Err(format!(
                "Format gambar '{}' tidak didukung oleh plugin ini. Format yang diterima: {:?}",
                image_meta.format, formats
            ));
        }
    }

    Ok(())
}

pub fn assemble_payload(payload: &ExecutionPayload, output_dir: &Path) -> Result<PathBuf, String> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Gagal membuat folder output: {}", e))?;
    }

    let payload_file_path = output_dir.join("payload.json");
    let json_data = serde_json::to_string_pretty(payload)
        .map_err(|e| format!("Gagal memformat JSON payload: {}", e))?;

    fs::write(&payload_file_path, json_data)
        .map_err(|e| format!("Gagal menulis berkas payload.json: {}", e))?;

    Ok(payload_file_path)
}
