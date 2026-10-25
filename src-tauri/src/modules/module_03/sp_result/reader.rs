//! Pembaca data hasil analitik spasial (Dual-Reader: Decomposed Folder & Flat File).
//!
//! Mendukung pembacaan koeksistensi format:
//! 1. Dekomposisi atomik folder: `{layer_id}/layer.json` + `features/{feature_id}.json`
//! 2. Berkas flat JSON: `{layer_id}.json`

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::pure::validate_spatial_result_id;
use super::types::{LayerDisplayMode, LayerPayload, SpatialProperties, SpatialResultSummary};
use super::writer::{StoredFeatureFile, StoredLayerFile};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename};
use crate::modules::module_03::types::sp_result_type::MapLayerCommand;

/// Memuat dan mem-parsing data dummy hasil spasial yang di-embed saat compile time.
#[tauri::command]
pub fn get_dummy_spatial_layers() -> Result<Vec<MapLayerCommand>, String> {
    Ok(vec![])
}

pub mod dummy {
    pub use super::get_dummy_spatial_layers;
}

/// Menghitung token konkurensi (updated_at string mentah atau {size}_{mtime_millis}).
pub fn compute_concurrency_token(path: &Path, updated_at: Option<&str>) -> String {
    if let Some(ua) = updated_at {
        let trimmed = ua.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(metadata) = path.metadata() {
        let len = metadata.len();
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("{}_{}", len, mtime)
    } else {
        "0_0".to_string()
    }
}

/// Ekstraksi URL thumbnail foto sesuai hierarki prioritas:
/// 1. Prioritas 1: display_preference.image_url pada level layer
/// 2. Prioritas 2: image_url pada fitur pertama yang memilikinya (feature.image_url atau feature.properties.image_url)
/// 3. Prioritas 3: None (menampilkan placeholder "No Image")
pub fn extract_thumbnail_url(payload: &LayerPayload) -> Option<String> {
    // 1. Prioritas 1: Layer-Level image_url
    match &payload.display_preference {
        LayerDisplayMode::Polygon { image_url, .. }
        | LayerDisplayMode::Point { image_url, .. }
        | LayerDisplayMode::Line { image_url, .. } => {
            if let Some(url) = image_url {
                let trimmed = url.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        LayerDisplayMode::ImageOverlay { image_url, .. } => {
            let trimmed = image_url.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // 2. Prioritas 2: Fitur Pertama
    for feat in &payload.spatial_results {
        if let Some(url) = &feat.image_url {
            let trimmed = url.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        let prop_img: Option<&str> = match &feat.properties {
            SpatialProperties::Tree(p) => p
                .image_url
                .as_deref()
                .or_else(|| p.extra.get("image_url").and_then(|v| v.as_str())),
            SpatialProperties::Vegetation(p) => p
                .image_url
                .as_deref()
                .or_else(|| p.extra.get("image_url").and_then(|v| v.as_str())),
            SpatialProperties::LandCover(p) => p
                .image_url
                .as_deref()
                .or_else(|| p.extra.get("image_url").and_then(|v| v.as_str())),
            SpatialProperties::InspectionPath(p) => p
                .image_url
                .as_deref()
                .or_else(|| p.extra.get("image_url").and_then(|v| v.as_str())),
            SpatialProperties::Ortho(p) => p
                .image_url
                .as_deref()
                .or_else(|| p.extra.get("image_url").and_then(|v| v.as_str())),
            SpatialProperties::Generic(map) => map.get("image_url").and_then(|v| v.as_str()),
        };
        if let Some(url) = prop_img {
            let trimmed = url.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

/// Menentukan geom_type representatif untuk kartu ringkasan UI:
/// "Point" | "Polygon" | "Line" | "Overlay" | "Mixed"
pub fn determine_geom_type(payload: &LayerPayload) -> String {
    if matches!(
        payload.display_preference,
        LayerDisplayMode::ImageOverlay { .. }
    ) {
        return "Overlay".to_string();
    }

    if payload.spatial_results.is_empty() {
        return match &payload.display_preference {
            LayerDisplayMode::Polygon { .. } => "Polygon".to_string(),
            LayerDisplayMode::Point { .. } => "Point".to_string(),
            LayerDisplayMode::Line { .. } => "Line".to_string(),
            LayerDisplayMode::ImageOverlay { .. } => "Overlay".to_string(),
        };
    }

    let mut has_point = false;
    let mut has_polygon = false;
    let mut has_line = false;

    for feat in &payload.spatial_results {
        match feat.geometry {
            super::types::SpatialGeometry::Point { .. } => has_point = true,
            super::types::SpatialGeometry::Polygon { .. } => has_polygon = true,
            super::types::SpatialGeometry::LineString { .. } => has_line = true,
        }
    }

    let count = (has_point as u8) + (has_polygon as u8) + (has_line as u8);
    if count > 1 {
        "Mixed".to_string()
    } else if has_polygon {
        "Polygon".to_string()
    } else if has_point {
        "Point".to_string()
    } else if has_line {
        "Line".to_string()
    } else {
        "Mixed".to_string()
    }
}

/// Ekstraksi warna tampilan dan ikon dari display_preference.
pub fn extract_presentation(payload: &LayerPayload) -> (String, Option<String>) {
    match &payload.display_preference {
        LayerDisplayMode::Polygon { color, .. } => (color.clone(), None),
        LayerDisplayMode::Point { color, icon, .. } => {
            let clean_icon = if icon.trim().is_empty() {
                None
            } else {
                Some(icon.clone())
            };
            (color.clone(), clean_icon)
        }
        LayerDisplayMode::Line { color, .. } => (color.clone(), None),
        LayerDisplayMode::ImageOverlay { .. } => ("#3498DB".to_string(), None),
    }
}

/// Memeriksa apakah payload memiliki kustomisasi manual (custom fields atau edited_fields).
pub fn has_manual_edits_in_payload(payload: &LayerPayload) -> bool {
    let has_custom_fields = match &payload.display_preference {
        LayerDisplayMode::Polygon { fields, .. }
        | LayerDisplayMode::Point { fields, .. }
        | LayerDisplayMode::Line { fields, .. }
        | LayerDisplayMode::ImageOverlay { fields, .. } => fields
            .as_ref()
            .map(|f| f.iter().any(|x| x.custom))
            .unwrap_or(false),
    };
    let has_edited_features = payload.spatial_results.iter().any(|f| {
        f.edited_fields
            .as_ref()
            .map(|e| !e.is_empty())
            .unwrap_or(false)
    });
    has_custom_fields || has_edited_features
}

/// Membaca layer dari format folder dekomposisi (`{id}/layer.json` + `features/*.json`).
pub fn read_decomposed_layer(
    folder_path: &Path,
    official_id: &str,
) -> Result<LayerPayload, MapControllerError> {
    let layer_file = folder_path.join("layer.json");
    if !layer_file.is_file() {
        return Err(MapControllerError::NotFound(format!(
            "Berkas layer.json tidak ditemukan di folder '{}'",
            official_id
        )));
    }

    let layer_content = fs::read_to_string(&layer_file)?;
    let layer_meta: StoredLayerFile = serde_json::from_str(&layer_content)?;

    let mut features = Vec::new();
    let features_dir = folder_path.join("features");
    if features_dir.is_dir() {
        let mut feat_entries = fs::read_dir(&features_dir)?.flatten().collect::<Vec<_>>();
        feat_entries.sort_by_key(|e| e.file_name());

        for f_entry in feat_entries {
            let f_path = f_entry.path();
            if f_path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(f_content) = fs::read_to_string(&f_path) {
                    if let Ok(stored_feat) = serde_json::from_str::<StoredFeatureFile>(&f_content) {
                        features.push(stored_feat.result);
                    }
                }
            }
        }
    }

    let token = compute_concurrency_token(&layer_file, layer_meta.updated_at.as_deref());

    Ok(LayerPayload {
        layer_id: official_id.to_string(),
        layer_name: layer_meta.layer_name,
        display_preference: layer_meta.display_preference,
        spatial_results: features,
        updated_at: layer_meta.updated_at,
        concurrency_token: Some(token),
        extra: Default::default(),
    })
}

/// Membaca layer dari format berkas flat JSON (`{id}.json`).
/// ID resmi selalu diselaraskan dengan nama berkas tanpa ".json" (persis apa adanya).
pub fn read_flat_layer(
    file_path: &Path,
    official_id: &str,
) -> Result<LayerPayload, MapControllerError> {
    let content = fs::read_to_string(file_path)?;
    let mut payload: LayerPayload = serde_json::from_str(&content)?;
    let token = compute_concurrency_token(file_path, payload.updated_at.as_deref());
    payload.layer_id = official_id.to_string();
    payload.concurrency_token = Some(token);
    Ok(payload)
}

/// Mengambil satu Spatial Result lengkap berdasarkan `layer_id` via Dual-Reader.
pub fn get_saved_spatial_result_in_dir(
    storage_dir: &Path,
    layer_id: &str,
) -> Result<LayerPayload, MapControllerError> {
    validate_spatial_result_id(layer_id)?;

    let root_dir = storage_dir.join("spatial_results");
    let safe_id = sanitize_filename(layer_id);

    // Prioritas 1: Folder dekomposisi
    let folder_path = root_dir.join(&safe_id);
    if folder_path.is_dir() && folder_path.join("layer.json").is_file() {
        return read_decomposed_layer(&folder_path, layer_id);
    }

    // Prioritas 2: Flat file JSON
    let flat_path = root_dir.join(format!("{}.json", safe_id));
    if flat_path.is_file() {
        return read_flat_layer(&flat_path, layer_id);
    }

    Err(MapControllerError::NotFound(format!(
        "Spatial result dengan ID '{}' tidak ditemukan di storage",
        layer_id
    )))
}

/// Mengambil seluruh data Spatial Result dari `storage_dir/spatial_results/` via Dual-Reader.
pub fn get_saved_spatial_results_in_dir(
    storage_dir: &Path,
) -> Result<Vec<LayerPayload>, MapControllerError> {
    let root_dir = storage_dir.join("spatial_results");
    if !root_dir.exists() {
        return Ok(Vec::new());
    }

    let mut layers = Vec::new();
    let mut folder_ids = HashSet::new();

    let mut entries = fs::read_dir(&root_dir)?.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|e| e.file_name());

    // 1. Proses folder dekomposisi terlebih dahulu
    for entry in &entries {
        let path = entry.path();
        if path.is_dir() {
            let folder_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            if folder_name.starts_with('.') || validate_spatial_result_id(&folder_name).is_err() {
                continue;
            }

            if path.join("layer.json").is_file() {
                folder_ids.insert(folder_name.clone());
                if let Ok(layer) = read_decomposed_layer(&path, &folder_name) {
                    layers.push(layer);
                }
            }
        }
    }

    // 2. Proses flat files dengan resolusi bentrok
    for entry in &entries {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            if stem.starts_with('.') || validate_spatial_result_id(&stem).is_err() {
                continue;
            }

            // Abaikan flat file bayangan jika folder dengan ID yang sama sudah ada
            if folder_ids.contains(&stem) {
                eprintln!(
                    "Warning: Flat file bayangan '{}.json' diabaikan karena folder dekomposisi mendahului.",
                    stem
                );
                continue;
            }

            if let Ok(layer) = read_flat_layer(&path, &stem) {
                layers.push(layer);
            }
        }
    }

    layers.sort_by(|a, b| a.layer_name.cmp(&b.layer_name));
    Ok(layers)
}

/// Mengambil daftar ringkasan Spatial Result (ringan) dari `storage_dir/spatial_results/`.
/// Menghasilkan format fallback graceful jika berkas JSON korup (`is_corrupted = true`).
pub fn get_saved_spatial_results_summary_in_dir(
    storage_dir: &Path,
) -> Result<Vec<SpatialResultSummary>, MapControllerError> {
    let root_dir = storage_dir.join("spatial_results");
    if !root_dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();
    let mut folder_ids = HashSet::new();

    let mut entries = fs::read_dir(&root_dir)?.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|e| e.file_name());

    // 1. Scan folder dekomposisi
    for entry in &entries {
        let path = entry.path();
        if path.is_dir() {
            let folder_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            if folder_name.starts_with('.') || validate_spatial_result_id(&folder_name).is_err() {
                continue;
            }

            folder_ids.insert(folder_name.clone());

            let layer_file = path.join("layer.json");
            if !layer_file.is_file() {
                // Folder rusak tanpa layer.json
                summaries.push(SpatialResultSummary {
                    id: folder_name.clone(),
                    layer_name: format!("{} (layer.json hilang)", folder_name),
                    feature_count: 0,
                    geom_type: "Unknown".to_string(),
                    display_color: "#E74C3C".to_string(),
                    icon: None,
                    thumbnail_url: None,
                    format_type: "decomposed".to_string(),
                    is_corrupted: true,
                    updated_at: None,
                    has_manual_edits: false,
                    concurrency_token: "0_0".to_string(),
                });
                continue;
            }

            match read_decomposed_layer(&path, &folder_name) {
                Ok(layer) => {
                    let geom_type = determine_geom_type(&layer);
                    let (display_color, icon) = extract_presentation(&layer);
                    let thumbnail_url = extract_thumbnail_url(&layer);
                    let has_manual_edits = has_manual_edits_in_payload(&layer);
                    let token = layer.concurrency_token.clone().unwrap_or_else(|| {
                        compute_concurrency_token(&layer_file, layer.updated_at.as_deref())
                    });

                    summaries.push(SpatialResultSummary {
                        id: folder_name,
                        layer_name: layer.layer_name,
                        feature_count: layer.spatial_results.len(),
                        geom_type,
                        display_color,
                        icon,
                        thumbnail_url,
                        format_type: "decomposed".to_string(),
                        is_corrupted: false,
                        updated_at: layer.updated_at,
                        has_manual_edits,
                        concurrency_token: token,
                    });
                }
                Err(_) => {
                    summaries.push(SpatialResultSummary {
                        id: folder_name.clone(),
                        layer_name: format!("{} (Metadata rusak)", folder_name),
                        feature_count: 0,
                        geom_type: "Unknown".to_string(),
                        display_color: "#E74C3C".to_string(),
                        icon: None,
                        thumbnail_url: None,
                        format_type: "decomposed".to_string(),
                        is_corrupted: true,
                        updated_at: None,
                        has_manual_edits: false,
                        concurrency_token: "0_0".to_string(),
                    });
                }
            }
        }
    }

    // 2. Scan flat files
    for entry in &entries {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            if stem.starts_with('.') || validate_spatial_result_id(&stem).is_err() {
                continue;
            }

            if folder_ids.contains(&stem) {
                // Abaikan flat file bayangan
                continue;
            }

            match read_flat_layer(&path, &stem) {
                Ok(layer) => {
                    let geom_type = determine_geom_type(&layer);
                    let (display_color, icon) = extract_presentation(&layer);
                    let thumbnail_url = extract_thumbnail_url(&layer);
                    let has_manual_edits = has_manual_edits_in_payload(&layer);
                    let token = layer.concurrency_token.clone().unwrap_or_else(|| {
                        compute_concurrency_token(&path, layer.updated_at.as_deref())
                    });

                    summaries.push(SpatialResultSummary {
                        id: stem,
                        layer_name: layer.layer_name,
                        feature_count: layer.spatial_results.len(),
                        geom_type,
                        display_color,
                        icon,
                        thumbnail_url,
                        format_type: "flat".to_string(),
                        is_corrupted: false,
                        updated_at: layer.updated_at,
                        has_manual_edits,
                        concurrency_token: token,
                    });
                }
                Err(_) => {
                    // Fallback graceful untuk flat file rusak
                    summaries.push(SpatialResultSummary {
                        id: stem.clone(),
                        layer_name: format!("{} (Metadata rusak)", stem),
                        feature_count: 0,
                        geom_type: "Unknown".to_string(),
                        display_color: "#E74C3C".to_string(),
                        icon: None,
                        thumbnail_url: None,
                        format_type: "flat".to_string(),
                        is_corrupted: true,
                        updated_at: None,
                        has_manual_edits: false,
                        concurrency_token: "0_0".to_string(),
                    });
                }
            }
        }
    }

    // Urutkan alfabetis berdasarkan layer_name
    summaries.sort_by(|a, b| a.layer_name.cmp(&b.layer_name));
    Ok(summaries)
}

/// Mengambil seluruh data Spatial Result yang tersimpan (pembungkus publik ke storage default).
pub fn get_saved_spatial_results() -> Result<Vec<LayerPayload>, MapControllerError> {
    get_saved_spatial_results_in_dir(&get_storage_root())
}

/// Mengambil daftar ringkasan Spatial Result (pembungkus publik ke storage default).
pub fn get_saved_spatial_results_summary() -> Result<Vec<SpatialResultSummary>, MapControllerError>
{
    get_saved_spatial_results_summary_in_dir(&get_storage_root())
}

/// Mengambil satu Spatial Result lengkap (pembungkus publik ke storage default).
pub fn get_saved_spatial_result(layer_id: &str) -> Result<LayerPayload, MapControllerError> {
    get_saved_spatial_result_in_dir(&get_storage_root(), layer_id)
}
