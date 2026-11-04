//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk catatan anotasi peta.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `create_annotation_cmd`: Command untuk membuat anotasi baru dan menyimpannya ke disk.
//! - `get_saved_annotations`: Command untuk mengambil daftar seluruh catatan anotasi yang tersimpan.
//! - `get_annotation_by_id`: Command untuk mengambil satu anotasi spesifik berdasarkan ID.
//! - `update_annotation_cmd`: Command untuk memperbarui teks atau geometri anotasi.
//! - `delete_annotation_cmd`: Command untuk menghapus anotasi berdasarkan ID.

use super::annotation_builder_logic;
use super::annotation_entity_struct::AnnotationFeature;
use super::annotation_storage_io;
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
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let annotation =
        annotation_builder_logic::create_annotation(annotation_id, text, geometry, created_at);
    Ok(annotation_storage_io::save_annotation(annotation)?)
}

/// Mengambil seluruh anotasi yang tersimpan di storage.
#[tauri::command]
pub fn get_saved_annotations() -> Result<Vec<AnnotationFeature>, CommandError> {
    Ok(annotation_storage_io::get_saved_annotations()?)
}

/// Mengambil anotasi tunggal spesifik berdasarkan ID.
#[tauri::command]
pub fn get_annotation_by_id(annotation_id: String) -> Result<AnnotationFeature, CommandError> {
    Ok(annotation_storage_io::get_annotation_by_id(&annotation_id)?)
}

/// Memperbarui isi atau geometri anotasi tersimpan.
#[tauri::command]
pub fn update_annotation_cmd(
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    Ok(annotation_storage_io::update_annotation(annotation)?)
}

/// Menghapus anotasi dari storage berdasarkan ID.
#[tauri::command]
pub fn delete_annotation_cmd(annotation_id: String) -> Result<bool, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    Ok(annotation_storage_io::delete_saved_annotation(
        &annotation_id,
    )?)
}
