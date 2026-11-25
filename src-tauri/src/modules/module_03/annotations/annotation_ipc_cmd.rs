//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk catatan anotasi peta.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `create_annotation_cmd`: Command untuk membuat anotasi baru dan menyimpannya ke SQLite.
//! - `get_saved_annotations`: Command untuk mengambil daftar seluruh catatan anotasi yang tersimpan di SQLite.
//! - `get_annotation_by_id`: Command untuk mengambil satu anotasi spesifik berdasarkan ID dari SQLite.
//! - `update_annotation_cmd`: Command untuk memperbarui teks atau geometri anotasi di SQLite.
//! - `delete_annotation_cmd`: Command untuk menghapus anotasi berdasarkan ID dari SQLite.

use super::annotation_builder_logic;
use super::annotation_entity_struct::AnnotationFeature;
use super::annotation_storage_io;
use crate::modules::module_03::types::geometry_type::SpatialGeometry;
use crate::plugin_manager::error::CommandError;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DATABASE_ERROR".to_string(),
        message: format!("Gagal mengakses database: {e}"),
    }
}

/// Membuat annotation baru pada peta dan menyimpannya ke SQLite database.
#[tauri::command]
pub fn create_annotation_cmd(
    state: tauri::State<'_, crate::AppState>,
    annotation_id: String,
    text: String,
    geometry: SpatialGeometry,
    created_at: String,
) -> Result<AnnotationFeature, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let annotation =
        annotation_builder_logic::create_annotation(annotation_id, text, geometry, created_at);
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(annotation_storage_io::save_annotation(&conn, annotation)?)
}

/// Mengambil seluruh anotasi yang tersimpan di SQLite database.
#[tauri::command]
pub fn get_saved_annotations(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<AnnotationFeature>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(annotation_storage_io::get_saved_annotations(&conn)?)
}

/// Mengambil anotasi tunggal spesifik berdasarkan ID dari SQLite.
#[tauri::command]
pub fn get_annotation_by_id(
    state: tauri::State<'_, crate::AppState>,
    annotation_id: String,
) -> Result<AnnotationFeature, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(annotation_storage_io::get_annotation_by_id(
        &conn,
        &annotation_id,
    )?)
}

/// Memperbarui isi atau geometri anotasi tersimpan di SQLite.
#[tauri::command]
pub fn update_annotation_cmd(
    state: tauri::State<'_, crate::AppState>,
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(annotation_storage_io::update_annotation(&conn, annotation)?)
}

/// Menghapus anotasi dari SQLite berdasarkan ID.
#[tauri::command]
pub fn delete_annotation_cmd(
    state: tauri::State<'_, crate::AppState>,
    annotation_id: String,
) -> Result<bool, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(annotation_storage_io::delete_saved_annotation(
        &conn,
        &annotation_id,
    )?)
}
