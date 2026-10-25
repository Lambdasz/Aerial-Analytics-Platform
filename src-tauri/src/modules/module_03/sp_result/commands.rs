//! Tauri IPC commands untuk operasi spatial results (Module 3).

use crate::modules::module_03::sp_result::types::{
    DeleteSpatialResultSummary, LayerPayload, ManualEditsCheckResult, SpatialResultSummary,
    UpdateSpatialResultPayload,
};
use crate::modules::module_03::state::get_global_state;
use crate::plugin_manager::error::CommandError;
use tauri::Emitter;

/// Menyimpan data Spatial Result ke file JSON atomik pada `map_storage/spatial_results/{layer_id}/`.
#[tauri::command]
pub fn save_spatial_result(
    app: tauri::AppHandle,
    payload: LayerPayload,
) -> Result<LayerPayload, CommandError> {
    let saved = get_global_state().save_spatial_result(payload)?;
    let _ = app.emit("spatial_result:saved", &saved);
    Ok(saved)
}

/// Mengambil seluruh data Spatial Result yang tersimpan pada `map_storage/spatial_results/`.
#[tauri::command]
pub fn get_saved_spatial_results() -> Result<Vec<LayerPayload>, CommandError> {
    Ok(get_global_state().get_saved_spatial_results())
}

/// Mengambil daftar ringkasan Spatial Result (ringan) untuk visualisasi daftar/kartu UI.
#[tauri::command]
pub fn get_saved_spatial_results_summary() -> Result<Vec<SpatialResultSummary>, CommandError> {
    Ok(get_global_state().get_saved_spatial_results_summary()?)
}

/// Mengambil satu data Spatial Result lengkap berdasarkan layer ID (via Dual-Reader).
#[tauri::command]
pub fn get_saved_spatial_result(layer_id: String) -> Result<LayerPayload, CommandError> {
    Ok(get_global_state().get_saved_spatial_result(&layer_id)?)
}

/// Memperbarui field aman Spatial Result (layer_name, display_color, icon).
#[tauri::command]
pub fn update_saved_spatial_result(
    app: tauri::AppHandle,
    payload: UpdateSpatialResultPayload,
) -> Result<LayerPayload, CommandError> {
    let updated = get_global_state().update_spatial_result(payload)?;
    let _ = app.emit("spatial_result:saved", &updated);
    Ok(updated)
}

/// Menghitung dependensi sebelum dialog konfirmasi hapus.
#[tauri::command]
pub fn count_spatial_result_dependents(
    id: String,
) -> Result<DeleteSpatialResultSummary, CommandError> {
    Ok(get_global_state().count_spatial_result_dependents(&id)?)
}

/// Menghapus Spatial Result yang tersimpan berdasarkan layer ID.
#[tauri::command]
pub fn delete_saved_spatial_result(
    app: tauri::AppHandle,
    layer_id: String,
) -> Result<bool, CommandError> {
    let deleted = get_global_state().delete_saved_spatial_result(&layer_id)?;
    let _ = app.emit(
        "spatial_result:deleted",
        serde_json::json!({ "id": layer_id }),
    );
    Ok(deleted)
}

/// Memeriksa keberadaan edit manual pada layer spasial (command baca-saja).
#[tauri::command]
pub fn check_spatial_result_manual_edits(
    layer_id: String,
) -> Result<ManualEditsCheckResult, CommandError> {
    Ok(get_global_state().check_spatial_result_manual_edits(&layer_id)?)
}
