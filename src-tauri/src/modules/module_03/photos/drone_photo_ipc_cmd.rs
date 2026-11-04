//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk foto drone.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `get_photos`: Command untuk mengambil daftar metadata seluruh foto drone yang tersimpan.
//! - `import_photo`: Command untuk mengimpor foto drone, membaca EXIF GPS, dan menyimpannya ke disk.

use super::drone_exif_logic;
use super::drone_photo_storage_io;
use super::drone_photo_struct::PhotoMetadata;
use crate::modules::module_03::error::MapControllerError;
use crate::plugin_manager::error::CommandError;
use std::fs;
use std::path::Path;

/// Mengambil daftar seluruh foto drone yang tersimpan di `map_storage/photos/`.
#[tauri::command]
pub fn get_photos() -> Result<Vec<PhotoMetadata>, CommandError> {
    Ok(drone_photo_storage_io::get_photos()?)
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
    let photo = drone_exif_logic::parse_exif(&bytes, file_name)?;
    let saved = drone_photo_storage_io::save_photo(photo)?;
    Ok(saved)
}
