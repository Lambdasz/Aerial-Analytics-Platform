//! Tauri IPC commands untuk operasi Area of Interest (AOI).

use super::dto::{
    AoiFeatureResponse, CreateAoiRequest, DeleteAoiRequest, DeleteAoiResponse,
    GetSavedAoisResponse, SaveAoiRequest, SaveNewAoiRequest,
};
use crate::modules::module_03::aoi;
use crate::modules::module_03::geometry;
use crate::plugin_manager::error::CommandError;

/// Membuat validated AOI (Area of Interest) GeoJSON Feature secara fungsional.
#[tauri::command]
pub fn create_aoi_cmd(request: CreateAoiRequest) -> Result<AoiFeatureResponse, CommandError> {
    geometry::validate_polygon(&request.polygon)?;

    let area_sq_m = geometry::calculate_area(&request.polygon);
    let perimeter_m = geometry::calculate_perimeter(&request.polygon);

    let aoi = aoi::create_aoi(
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

/// Membuat validated AOI GeoJSON Feature dan langsung menyimpannya ke storage secara atomik.
#[tauri::command]
pub fn save_new_aoi_cmd(request: SaveNewAoiRequest) -> Result<AoiFeatureResponse, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    geometry::validate_polygon(&request.polygon)?;

    let area_sq_m = geometry::calculate_area(&request.polygon);
    let perimeter_m = geometry::calculate_perimeter(&request.polygon);
    let final_created_at = request
        .created_at
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let aoi_feature = aoi::create_aoi(
        request.aoi_id,
        request.name,
        request.aoi_type,
        request.polygon,
        final_created_at,
        area_sq_m,
        perimeter_m,
    );

    let saved = aoi::store::save_aoi(aoi_feature)?;
    Ok(AoiFeatureResponse { aoi: saved })
}

/// Menyimpan data AOI ke file JSON pada `map_storage/aois/{id}.json`.
#[tauri::command]
pub fn save_aoi(request: SaveAoiRequest) -> Result<AoiFeatureResponse, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let saved = aoi::store::save_aoi(request.aoi)?;
    Ok(AoiFeatureResponse { aoi: saved })
}

/// Mengambil seluruh data AOI yang tersimpan pada `map_storage/aois/`.
#[tauri::command]
pub fn get_saved_aois() -> Result<GetSavedAoisResponse, CommandError> {
    let aois = aoi::store::get_saved_aois()?;
    let total = aois.len();
    Ok(GetSavedAoisResponse { aois, total })
}

/// Menghapus AOI yang tersimpan berdasarkan ID.
#[tauri::command]
pub fn delete_saved_aoi(request: DeleteAoiRequest) -> Result<DeleteAoiResponse, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;
    let success = aoi::store::delete_saved_aoi(&request.aoi_id)?;
    Ok(DeleteAoiResponse {
        success,
        aoi_id: request.aoi_id,
    })
}
