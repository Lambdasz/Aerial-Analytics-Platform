//! Tauri IPC commands untuk pengelolaan citra ortomosaik dan raster JPG/PNG (Module 3).

use super::types::{
    DeleteOrthoSummary, ImportImagePayload, StoredImageMetadata, UpdateOrthoImagePayload,
};
use crate::modules::module_03::state::get_global_state;
use crate::plugin_manager::error::CommandError;

/// Mengambil daftar seluruh citra orthomosaic yang tersimpan di storage Modul 3.
/// Termasuk citra dengan metadata rusak (is_corrupted = true).
#[tauri::command]
pub fn list_ortho_images() -> Result<Vec<StoredImageMetadata>, CommandError> {
    Ok(get_global_state().list_ortho_images()?)
}

/// Mengambil metadata satu citra orthomosaic berdasarkan ID.
#[tauri::command]
pub fn get_ortho_image(id: String) -> Result<StoredImageMetadata, CommandError> {
    Ok(get_global_state().get_ortho_image(&id)?)
}

/// Memperbarui nama atau koordinat bounds citra orthomosaic.
#[tauri::command]
pub fn update_ortho_image(
    payload: UpdateOrthoImagePayload,
) -> Result<StoredImageMetadata, CommandError> {
    Ok(get_global_state().update_ortho_image(payload)?)
}

/// Menghitung data dependensi cascade (layer dan anotasi terikat) sebelum penghapusan.
#[tauri::command]
pub fn get_ortho_cascade_summary(id: String) -> Result<DeleteOrthoSummary, CommandError> {
    Ok(get_global_state().get_ortho_cascade_summary(&id)?)
}

/// Menghapus satu citra orthomosaic secara tuntas beserta layer dan anotasi terikat.
#[tauri::command]
pub fn delete_ortho_image(id: String) -> Result<DeleteOrthoSummary, CommandError> {
    Ok(get_global_state().delete_ortho_image(&id)?)
}

/// Mengimpor berkas citra orthomosaic JPG/PNG beserta koordinat area WGS84 ke dalam storage Modul 3.
#[tauri::command]
pub fn import_ortho_image(
    payload: ImportImagePayload,
) -> Result<StoredImageMetadata, CommandError> {
    Ok(get_global_state().import_ortho_image(payload)?)
}

/// Menyimpan file citra ke storage dari path file yang sudah ada di sistem.
#[tauri::command]
pub fn save_image_from_path(source_path: String) -> Result<StoredImageMetadata, CommandError> {
    Ok(get_global_state().save_image_from_path(&source_path)?)
}

/// Mengambil daftar seluruh citra yang tersimpan di storage Modul 3 (legacy).
#[tauri::command]
pub fn get_saved_images() -> Result<Vec<StoredImageMetadata>, CommandError> {
    Ok(get_global_state().get_saved_images())
}

/// Menghapus citra dari storage berdasarkan kategori dan nama file / image_id (legacy).
#[tauri::command]
pub fn delete_saved_image(category: String, filename: String) -> Result<bool, CommandError> {
    Ok(get_global_state().delete_saved_image(&category, &filename)?)
}
