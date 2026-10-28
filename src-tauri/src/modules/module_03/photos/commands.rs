//! Tauri IPC commands untuk ekstraksi dan query foto drone (Module 3).

use super::pure;
use super::store;
use super::types::PhotoMetadata;
use crate::modules::module_03::error::MapControllerError;
use crate::plugin_manager::error::CommandError;
use std::fs;
use std::path::Path;

/// Mengambil daftar seluruh foto drone yang tersimpan di `map_storage/photos/`.
#[tauri::command]
pub fn get_photos() -> Result<Vec<PhotoMetadata>, CommandError> {
    Ok(store::get_photos()?)
}

/// Mengimpor foto drone dari path lokal, mengekstrak EXIF, dan menyimpannya.
#[tauri::command]
pub fn import_photo(source_path: String) -> Result<PhotoMetadata, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let path = Path::new(&source_path);
    if !path.exists() {
        return Err(CommandError {
            code: "FILE_NOT_FOUND".to_string(),
            message: format!("File tidak ditemukan: {}", source_path),
        });
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("photo.jpg");

    let bytes = fs::read(path).map_err(MapControllerError::from)?;
    let photo = pure::parse_exif(&bytes, file_name)?;
    let saved = store::save_photo(photo)?;
    Ok(saved)
}
