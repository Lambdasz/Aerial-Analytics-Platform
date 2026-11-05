//! Layanan persistensi atomic untuk konfigurasi layer pada `map_storage/layers/{id}.json`.

use std::fs;
use std::path::PathBuf;

use super::types::LayerState;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename, write_atomic};

/// Mengembalikan direktori penyimpanan layer (`map_storage/layers/`).
pub fn get_layers_dir() -> PathBuf {
    let dir = get_storage_root().join("layers");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

/// Menyimpan konfigurasi layer ke `map_storage/layers/{id}.json` secara atomik.
pub fn save_layer(layer: LayerState) -> Result<LayerState, MapControllerError> {
    let dir = get_layers_dir();
    let safe_id = sanitize_filename(&layer.layer_id);
    let target_file = dir.join(format!("{}.json", safe_id));

    let json_content = serde_json::to_string(&layer)?;

    if json_content.len() > 65536 {
        return Err(MapControllerError::InvalidGeometry(
            "Ukuran JSON layer melebihi batas 64 KB".to_string(),
        ));
    }

    write_atomic(&target_file, &json_content)?;

    Ok(layer)
}

/// Membaca seluruh definisi layer dari `map_storage/layers/`.
pub fn get_saved_layers() -> Result<Vec<LayerState>, MapControllerError> {
    let dir = get_layers_dir();
    let mut layers = Vec::new();

    if !dir.exists() {
        return Ok(layers);
    }

    let entries = fs::read_dir(&dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Ok(layer) = serde_json::from_str::<LayerState>(&content) {
                layers.push(layer);
            }
        }
    }

    layers.sort_by_key(|l| l.z_index);

    Ok(layers)
}

/// Menghapus file layer berdasarkan ID dari `map_storage/layers/{id}.json`.
pub fn delete_saved_layer(layer_id: &str) -> Result<bool, MapControllerError> {
    let dir = get_layers_dir();
    let safe_id = sanitize_filename(layer_id);
    let target_file = dir.join(format!("{}.json", safe_id));

    if target_file.exists() {
        fs::remove_file(target_file)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
