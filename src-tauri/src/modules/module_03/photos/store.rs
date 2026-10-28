//! Layanan persistensi atomic untuk foto drone pada `map_storage/photos/{id}.json`.

use std::fs;
use std::path::PathBuf;

use super::types::PhotoMetadata;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename, write_atomic};

/// Mengembalikan direktori penyimpanan foto (`map_storage/photos/`).
pub fn get_photos_dir() -> PathBuf {
    let dir = get_storage_root().join("photos");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::create_dir_all(dir.join("raw"));
        let _ = fs::create_dir_all(dir.join("thumbnails"));
    }
    dir
}

/// Menyimpan metadata foto ke `map_storage/photos/{id}.json`.
pub fn save_photo(photo: PhotoMetadata) -> Result<PhotoMetadata, MapControllerError> {
    let dir = get_photos_dir();
    let safe_id = sanitize_filename(&photo.id);
    let target_file = dir.join(format!("{}.json", safe_id));

    let json_content = serde_json::to_string(&photo)?;

    if json_content.len() > 65536 {
        return Err(MapControllerError::InvalidGeometry(
            "Ukuran JSON foto melebihi batas 64 KB".to_string(),
        ));
    }

    write_atomic(&target_file, &json_content)?;

    Ok(photo)
}

/// Mengambil seluruh metadata foto dari `map_storage/photos/`.
pub fn get_photos() -> Result<Vec<PhotoMetadata>, MapControllerError> {
    let dir = get_photos_dir();
    let mut photos = Vec::new();

    if !dir.exists() {
        return Ok(photos);
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

            if let Ok(photo) = serde_json::from_str::<PhotoMetadata>(&content) {
                photos.push(photo);
            }
        }
    }

    photos.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(photos)
}

/// Menghapus foto berdasarkan ID dari `map_storage/photos/{id}.json`.
pub fn delete_photo(id: &str) -> Result<bool, MapControllerError> {
    let dir = get_photos_dir();
    let safe_id = sanitize_filename(id);
    let target_file = dir.join(format!("{}.json", safe_id));

    if target_file.exists() {
        fs::remove_file(target_file)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
