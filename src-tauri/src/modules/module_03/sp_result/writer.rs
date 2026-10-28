//! Layanan penulisan dan persistensi berkas untuk Spatial Results.
//!
//! Menangani penyimpanan atomik format dekomposisi folder (`{layer_id}/layer.json` + `features/{feature_id}.json`)
//! dan registrasi layer otomatis di `layers/{layer_id}.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::pure::{is_owned_result_photo, validate_photo_magic_bytes, validate_spatial_result_id};
use super::types::{LayerDisplayMode, LayerPayload, SpatialProperties, SpatialResult};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::layers::types::LayerState;
use crate::modules::module_03::storage_utils::{
    get_storage_root, sanitize_filename, write_atomic, write_atomic_bytes,
};

/// Metadata layer yang disimpan di `spatial_results/{layer_id}/layer.json`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StoredLayerFile {
    pub schema_version: u32,
    pub id: String,
    pub layer_name: String,
    pub display_preference: LayerDisplayMode,
    #[serde(default)]
    pub feature_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// File individual fitur yang disimpan di `spatial_results/{layer_id}/features/{feature_id}.json`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StoredFeatureFile {
    pub schema_version: u32,
    pub id: String,
    #[serde(flatten)]
    pub result: SpatialResult,
}

/// Menghasilkan ID unik yang bersih untuk setiap fitur spasial.
pub fn get_feature_id(feature: &SpatialResult, index: usize) -> String {
    match &feature.properties {
        SpatialProperties::Tree(t) => sanitize_filename(&t.tree_id),
        SpatialProperties::InspectionPath(p) => sanitize_filename(&p.path_id),
        SpatialProperties::Ortho(o) => sanitize_filename(&o.ortho_id),
        SpatialProperties::Generic(map) => {
            if let Some(id) = map.get("plot_id").and_then(|v| v.as_str()) {
                sanitize_filename(id)
            } else if let Some(id) = map.get("tree_id").and_then(|v| v.as_str()) {
                sanitize_filename(id)
            } else if let Some(id) = map.get("path_id").and_then(|v| v.as_str()) {
                sanitize_filename(id)
            } else {
                format!("feat_{:04}", index + 1)
            }
        }
        _ => format!("feat_{:04}", index + 1),
    }
}

/// Menyimpan berkas gambar baru untuk spatial result ke direktori `storage_dir/images/result_image/`.
/// Mengembalikan path URL web `/map_storage/images/result_image/<filename>`.
pub fn save_result_image(
    storage_dir: &Path,
    result_id: &str,
    source_path_str: &str,
) -> Result<String, MapControllerError> {
    let result_img_dir = storage_dir.join("images").join("result_image");
    fs::create_dir_all(&result_img_dir)?;

    let p = Path::new(source_path_str);
    if !p.is_file() {
        return Err(MapControllerError::StorageError(format!(
            "Berkas gambar sumber '{}' tidak ditemukan pada disk",
            source_path_str
        )));
    }

    let bytes = fs::read(p)?;
    let (_, ext) = validate_photo_magic_bytes(&bytes)?;

    let safe_id = sanitize_filename(result_id);
    let timestamp = chrono::Utc::now().timestamp_millis();
    let filename = format!("{}_{}.{}", safe_id, timestamp, ext);
    let target_path = result_img_dir.join(&filename);

    write_atomic_bytes(&target_path, &bytes)?;

    // Bersihkan berkas gambar lama yang khusus milik result_id ini di direktori result_image
    // (JANGAN PERNAH menyentuh gambar di luar folder result_image)
    if let Ok(entries) = fs::read_dir(&result_img_dir) {
        for entry in entries.flatten() {
            let item_path = entry.path();
            if item_path.is_file() && item_path != target_path {
                if let Some(name) = item_path.file_name().and_then(|n| n.to_str()) {
                    if is_owned_result_photo(&safe_id, name) {
                        let _ = fs::remove_file(&item_path);
                    }
                }
            }
        }
    }

    Ok(format!("/map_storage/images/result_image/{}", filename))
}

/// Menghapus berkas gambar khusus yang ada di `storage_dir/images/result_image/` milik result_id.
pub fn cleanup_result_image_file(storage_dir: &Path, result_id: &str) {
    let result_img_dir = storage_dir.join("images").join("result_image");
    if !result_img_dir.is_dir() {
        return;
    }
    let safe_id = sanitize_filename(result_id);
    if let Ok(entries) = fs::read_dir(&result_img_dir) {
        for entry in entries.flatten() {
            let item_path = entry.path();
            if item_path.is_file() {
                if let Some(name) = item_path.file_name().and_then(|n| n.to_str()) {
                    if is_owned_result_photo(&safe_id, name) {
                        let _ = fs::remove_file(&item_path);
                    }
                }
            }
        }
    }
}

/// Menyimpan LayerPayload secara atomik ke `storage_dir/spatial_results/{layer_id}/`.
pub fn save_spatial_result_in_dir(
    storage_dir: &Path,
    payload: LayerPayload,
) -> Result<LayerPayload, MapControllerError> {
    payload.validate()?;
    validate_spatial_result_id(&payload.layer_id)?;

    let root_dir = storage_dir.join("spatial_results");
    let safe_id = sanitize_filename(&payload.layer_id);
    let layer_dir = root_dir.join(&safe_id);
    let features_dir = layer_dir.join("features");

    fs::create_dir_all(&features_dir)?;

    // 1. Simpan layer metadata (layer.json) <= 64 KB
    let layer_meta = StoredLayerFile {
        schema_version: 1,
        id: payload.layer_id.clone(),
        layer_name: payload.layer_name.clone(),
        display_preference: payload.display_preference.clone(),
        feature_count: payload.spatial_results.len(),
        updated_at: payload.updated_at.clone(),
    };
    let layer_str = serde_json::to_string_pretty(&layer_meta)?;
    if layer_str.len() > 65536 {
        return Err(MapControllerError::InvalidGeometry(
            "Ukuran layer.json melebihi 64 KB".to_string(),
        ));
    }
    write_atomic(&layer_dir.join("layer.json"), &layer_str)?;

    // 2. Simpan setiap fitur sebagai berkas JSON atomik terpisah di features/{feature_id}.json
    for (i, feat) in payload.spatial_results.iter().enumerate() {
        let feat_id = get_feature_id(feat, i);
        let feat_file = StoredFeatureFile {
            schema_version: 1,
            id: feat_id.clone(),
            result: feat.clone(),
        };

        let feat_str = serde_json::to_string_pretty(&feat_file)?;
        if feat_str.len() > 65536 {
            return Err(MapControllerError::InvalidGeometry(format!(
                "Ukuran fitur '{}' melebihi batas 64 KB",
                feat_id
            )));
        }

        let target_feature = features_dir.join(format!("{}.json", feat_id));
        write_atomic(&target_feature, &feat_str)?;
    }

    // 3. Registrasi otomatis layer di storage_dir/layers/{layer_id}.json
    let (category, opacity) = match &payload.display_preference {
        LayerDisplayMode::Polygon { opacity, .. } => ("polygon".to_string(), *opacity),
        LayerDisplayMode::Point { .. } => ("point".to_string(), 1.0),
        LayerDisplayMode::Line { .. } => ("line".to_string(), 1.0),
        LayerDisplayMode::ImageOverlay { opacity, .. } => ("image_overlay".to_string(), *opacity),
    };

    let layer_state = LayerState {
        layer_id: payload.layer_id.clone(),
        layer_name: payload.layer_name.clone(),
        category,
        is_visible: true,
        opacity,
        z_index: 10,
        source_type: Some("spatial_result".to_string()),
        source_id: Some(payload.layer_id.clone()),
    };
    let layers_dir = storage_dir.join("layers");
    let _ = fs::create_dir_all(&layers_dir);
    let layer_state_str = serde_json::to_string_pretty(&layer_state)?;
    write_atomic(
        &layers_dir.join(format!("{}.json", safe_id)),
        &layer_state_str,
    )?;

    Ok(payload)
}

/// Menyimpan LayerPayload secara atomik (pembungkus publik ke storage default).
pub fn save_spatial_result(payload: LayerPayload) -> Result<LayerPayload, MapControllerError> {
    save_spatial_result_in_dir(&get_storage_root(), payload)
}
