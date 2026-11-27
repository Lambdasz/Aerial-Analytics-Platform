//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk Area of Interest (AOI).
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `create_aoi_cmd`: Command untuk memvalidasi dan membuat objek AOI Feature di memori.
//! - `save_new_aoi_cmd`: Command untuk memvalidasi, membuat, dan menyimpan AOI baru ke disk secara atomik.
//! - `save_aoi`: Command untuk menyimpan pembaruan objek AOI yang ada ke disk.
//! - `get_saved_aois`: Command untuk mengambil daftar seluruh AOI yang tersimpan di disk.
//! - `delete_saved_aoi`: Command untuk menghapus AOI tertentu dari disk berdasarkan ID.

use super::aoi_builder_logic;
use super::aoi_ipc_struct::{
    AoiFeatureResponse, CreateAoiRequest, DeleteAoiRequest, DeleteAoiResponse,
    GetSavedAoisResponse, SaveAoiRequest, SaveNewAoiRequest,
};
use super::aoi_storage_io;
use crate::modules::module_03::geometry;
use crate::plugin_manager::error::CommandError;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DATABASE_ERROR".to_string(),
        message: format!("Gagal mengakses database: {e}"),
    }
}

/// Membuat validated AOI (Area of Interest) GeoJSON Feature secara fungsional.
#[tauri::command]
pub fn create_aoi_cmd(request: CreateAoiRequest) -> Result<AoiFeatureResponse, CommandError> {
    geometry::validate_polygon(&request.polygon)?;

    let area_sq_m = geometry::calculate_area(&request.polygon);
    let perimeter_m = geometry::calculate_perimeter(&request.polygon);

    let aoi = aoi_builder_logic::create_aoi(
        request.aoi_id,
        request.name,
        request.aoi_type,
        request.polygon,
        request.created_at,
        area_sq_m,
        perimeter_m,
    );

    Ok(AoiFeatureResponse { aoi })
}

/// Membuat validated AOI GeoJSON Feature dan langsung menyimpannya ke database SQLite secara atomik.
#[tauri::command]
pub fn save_new_aoi_cmd(
    state: tauri::State<'_, crate::AppState>,
    request: SaveNewAoiRequest,
) -> Result<AoiFeatureResponse, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    geometry::validate_polygon(&request.polygon)?;

    let area_sq_m = geometry::calculate_area(&request.polygon);
    let perimeter_m = geometry::calculate_perimeter(&request.polygon);
    let final_created_at = request
        .created_at
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let aoi_feature = aoi_builder_logic::create_aoi(
        request.aoi_id,
        request.name,
        request.aoi_type,
        request.polygon,
        final_created_at,
        area_sq_m,
        perimeter_m,
    );

    let conn = state.db.lock().map_err(lock_err)?;
    let saved = aoi_storage_io::save_aoi(&conn, aoi_feature)?;
    Ok(AoiFeatureResponse { aoi: saved })
}

/// Menyimpan data AOI ke database SQLite.
#[tauri::command]
pub fn save_aoi(
    state: tauri::State<'_, crate::AppState>,
    request: SaveAoiRequest,
) -> Result<AoiFeatureResponse, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    let saved = aoi_storage_io::save_aoi(&conn, request.aoi)?;
    Ok(AoiFeatureResponse { aoi: saved })
}

/// Mengambil seluruh data AOI yang tersimpan pada SQLite.
#[tauri::command]
pub fn get_saved_aois(
    state: tauri::State<'_, crate::AppState>,
) -> Result<GetSavedAoisResponse, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    let aois = aoi_storage_io::get_saved_aois(&conn)?;
    let total = aois.len();
    Ok(GetSavedAoisResponse { aois, total })
}

/// Menghapus AOI yang tersimpan berdasarkan ID dari SQLite.
#[tauri::command]
pub fn delete_saved_aoi(
    state: tauri::State<'_, crate::AppState>,
    request: DeleteAoiRequest,
) -> Result<DeleteAoiResponse, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let conn = state.db.lock().map_err(lock_err)?;
    let success = aoi_storage_io::delete_saved_aoi(&conn, &request.aoi_id)?;
    Ok(DeleteAoiResponse {
        success,
        aoi_id: request.aoi_id,
    })
}
