//! Berkas ini berisi handler perintah IPC (Tauri Command) untuk Spatial Result (Modul 3).
//!
//! Daftar Fungsi Handler IPC:
//! - `save_spatial_result`: Menyimpan data Spatial Result ke berkas JSON atomik.
//! - `get_saved_spatial_results`: Mengambil seluruh data Spatial Result yang tersimpan.
//! - `get_saved_spatial_results_summary`: Mengambil ringkasan ringan Spatial Result untuk daftar/kartu UI.
//! - `get_saved_spatial_result`: Mengambil satu data Spatial Result lengkap berdasarkan layer ID.
//! - `update_saved_spatial_result`: Memperbarui atribut aman Spatial Result (nama layer, warna, icon).
//! - `count_spatial_result_dependents`: Menghitung dependensi terkait sebelum konfirmasi hapus.
//! - `delete_saved_spatial_result`: Menghapus Spatial Result dari disk secara aman.
//! - `check_spatial_result_manual_edits`: Memeriksa keberadaan edit manual pada layer spasial.

use crate::modules::module_03::sp_result::sqlite_store;
use crate::modules::module_03::sp_result::types::{
    DeleteSpatialResultSummary, LayerPayload, ManualEditsCheckResult, SpatialResultSummary,
    UpdateSpatialResultPayload,
};
use crate::plugin_manager::error::CommandError;
use tauri::Emitter;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DATABASE_ERROR".to_string(),
        message: format!("Gagal mengakses database: {e}"),
    }
}

/// Menyimpan data Spatial Result ke database SQLite.
#[tauri::command]
pub fn save_spatial_result(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    payload: LayerPayload,
) -> Result<LayerPayload, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    let saved = sqlite_store::save_spatial_result(&conn, payload)?;
    let _ = app.emit("spatial_result:saved", &saved);
    Ok(saved)
}

/// Mengambil seluruh data Spatial Result yang tersimpan di SQLite.
#[tauri::command]
pub fn get_saved_spatial_results(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<LayerPayload>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(sqlite_store::get_saved_spatial_results(&conn)?)
}

/// Mengambil daftar ringkasan Spatial Result (ringan) untuk visualisasi daftar/kartu UI.
#[tauri::command]
pub fn get_saved_spatial_results_summary(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<SpatialResultSummary>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(sqlite_store::get_saved_spatial_results_summary(&conn)?)
}

/// Mengambil satu data Spatial Result lengkap berdasarkan layer ID dari database SQLite.
#[tauri::command]
pub fn get_saved_spatial_result(
    state: tauri::State<'_, crate::AppState>,
    layer_id: String,
) -> Result<LayerPayload, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(sqlite_store::get_saved_spatial_result(&conn, &layer_id)?)
}

/// Memperbarui field aman Spatial Result (layer_name, display_color, icon, feature_edits) di SQLite.
#[tauri::command]
pub fn update_saved_spatial_result(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    payload: UpdateSpatialResultPayload,
) -> Result<LayerPayload, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    let updated = sqlite_store::update_saved_spatial_result(&conn, payload)?;
    let _ = app.emit("spatial_result:saved", &updated);
    Ok(updated)
}

/// Menghitung dependensi sebelum dialog konfirmasi hapus di SQLite.
#[tauri::command]
pub fn count_spatial_result_dependents(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> Result<DeleteSpatialResultSummary, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(sqlite_store::count_spatial_result_dependents(&conn, &id)?)
}

/// Menghapus Spatial Result yang tersimpan berdasarkan layer ID dari SQLite.
#[tauri::command]
pub fn delete_saved_spatial_result(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    layer_id: String,
) -> Result<bool, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    let deleted = sqlite_store::delete_saved_spatial_result(&conn, &layer_id)?;
    let _ = app.emit(
        "spatial_result:deleted",
        serde_json::json!({ "id": layer_id }),
    );
    Ok(deleted)
}

/// Memeriksa keberadaan edit manual pada layer spasial di SQLite (command baca-saja).
#[tauri::command]
pub fn check_spatial_result_manual_edits(
    state: tauri::State<'_, crate::AppState>,
    layer_id: String,
) -> Result<ManualEditsCheckResult, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(sqlite_store::check_spatial_result_manual_edits(
        &conn, &layer_id,
    )?)
}
