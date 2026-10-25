//! Tauri IPC commands untuk CRUD anotasi inspeksi peta (Module 3).

use super::pure;
use super::store;
use super::types::AnnotationFeature;
use crate::modules::module_03::types::sp_result_type::SpatialGeometry;
use crate::plugin_manager::error::CommandError;

/// Membuat annotation baru pada peta dan menyimpannya ke storage.
#[tauri::command]
pub fn create_annotation_cmd(
    annotation_id: String,
    text: String,
    geometry: SpatialGeometry,
    created_at: String,
) -> Result<AnnotationFeature, CommandError> {
    let annotation = pure::create_annotation(annotation_id, text, geometry, created_at);
    Ok(store::save_annotation(annotation)?)
}

/// Mengambil seluruh anotasi yang tersimpan di storage.
#[tauri::command]
pub fn get_saved_annotations() -> Result<Vec<AnnotationFeature>, CommandError> {
    Ok(store::get_saved_annotations()?)
}

/// Mengambil anotasi tunggal spesifik berdasarkan ID.
#[tauri::command]
pub fn get_annotation_by_id(annotation_id: String) -> Result<AnnotationFeature, CommandError> {
    Ok(store::get_annotation_by_id(&annotation_id)?)
}

/// Memperbarui isi atau geometri anotasi tersimpan.
#[tauri::command]
pub fn update_annotation_cmd(
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, CommandError> {
    Ok(store::update_annotation(annotation)?)
}

/// Menghapus anotasi dari storage berdasarkan ID.
#[tauri::command]
pub fn delete_annotation_cmd(annotation_id: String) -> Result<bool, CommandError> {
    Ok(store::delete_saved_annotation(&annotation_id)?)
}
