//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk foto drone.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `get_photos`: Command untuk mengambil daftar metadata seluruh foto drone yang tersimpan.
//! - `import_photo`: Command untuk mengimpor foto drone, membaca EXIF GPS, dan menyimpannya ke disk.

use super::drone_exif_logic as pure;
use super::drone_photo_storage_io as store;
use super::drone_photo_struct::PhotoMetadata;
use crate::modules::module_03::error::MapControllerError;
use crate::plugin_manager::error::CommandError;
use std::fs;
use std::path::Path;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DATABASE_ERROR".to_string(),
        message: format!("Gagal mengakses database: {e}"),
    }
}

/// Mengambil daftar seluruh foto drone yang tersimpan di SQLite.
#[tauri::command]
pub fn get_photos(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<PhotoMetadata>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(store::get_photos(&conn)?)
}

/// Mengimpor foto drone dari path lokal, mengekstrak EXIF, dan menyimpannya ke SQLite.
#[tauri::command]
pub fn import_photo(
    state: tauri::State<'_, crate::AppState>,
    source_path: String,
) -> Result<PhotoMetadata, CommandError> {
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

    let conn = state.db.lock().map_err(lock_err)?;
    let saved = store::save_photo(&conn, photo)?;
    Ok(saved)
}
