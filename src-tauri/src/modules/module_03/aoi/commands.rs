//! Tauri IPC commands untuk operasi Area of Interest (AOI).

use crate::modules::module_03::aoi;
use crate::modules::module_03::geometry;
use crate::modules::module_03::types::aoi_type::{AoiFeature, Polygon};
use crate::plugin_manager::error::CommandError;

/// Membuat validated AOI (Area of Interest) GeoJSON Feature secara fungsional.
#[tauri::command]
pub fn create_aoi_cmd(
    aoi_id: String,
    name: String,
    aoi_type: Option<String>,
    polygon: Polygon,
    created_at: String,
) -> Result<AoiFeature, CommandError> {
    geometry::validate_polygon(&polygon)?;

    let area_sq_m = geometry::calculate_area(&polygon);
    let perimeter_m = geometry::calculate_perimeter(&polygon);

    Ok(aoi::create_aoi(
        aoi_id,
        name,
        aoi_type,
        polygon,
        created_at,
        area_sq_m,
        perimeter_m,
    ))
}

/// Membuat validated AOI GeoJSON Feature dan langsung menyimpannya ke storage secara atomik.
#[tauri::command]
pub fn save_new_aoi_cmd(
    aoi_id: String,
    name: String,
    aoi_type: Option<String>,
    polygon: Polygon,
    created_at: Option<String>,
) -> Result<AoiFeature, CommandError> {
    geometry::validate_polygon(&polygon)?;

    let area_sq_m = geometry::calculate_area(&polygon);
    let perimeter_m = geometry::calculate_perimeter(&polygon);
    let final_created_at = created_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let aoi_feature = aoi::create_aoi(
        aoi_id,
        name,
        aoi_type,
        polygon,
        final_created_at,
        area_sq_m,
        perimeter_m,
    );

    Ok(aoi::store::save_aoi(aoi_feature)?)
}

/// Menyimpan data AOI ke file JSON pada `map_storage/aois/{id}.json`.
#[tauri::command]
pub fn save_aoi(aoi: AoiFeature) -> Result<AoiFeature, CommandError> {
    Ok(aoi::store::save_aoi(aoi)?)
}

/// Mengambil seluruh data AOI yang tersimpan pada `map_storage/aois/`.
#[tauri::command]
pub fn get_saved_aois() -> Result<Vec<AoiFeature>, CommandError> {
    Ok(aoi::store::get_saved_aois()?)
}

/// Menghapus AOI yang tersimpan berdasarkan ID.
#[tauri::command]
pub fn delete_saved_aoi(aoi_id: String) -> Result<bool, CommandError> {
    Ok(aoi::store::delete_saved_aoi(&aoi_id)?)
}
