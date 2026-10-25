//! Layanan persistensi atomic untuk entitas AOI pada `map_storage/aois/{id}.json`.

use std::fs;
use std::path::PathBuf;

use super::types::AoiFeature;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::state::get_global_state;
use crate::modules::module_03::storage_utils::get_storage_root;

/// Mengembalikan direktori penyimpanan AOI (`map_storage/aois/`).
pub fn get_aois_dir() -> PathBuf {
    let dir = get_storage_root().join("aois");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

/// Menyimpan entitas AOI ke dalam file JSON di `map_storage/aois/{id}.json` secara atomik (write-through).
pub fn save_aoi(aoi: AoiFeature) -> Result<AoiFeature, MapControllerError> {
    get_global_state().save_aoi(aoi)
}

/// Mengambil seluruh data AOI yang tersimpan di direktori `map_storage/aois/`.
pub fn get_saved_aois() -> Result<Vec<AoiFeature>, MapControllerError> {
    Ok(get_global_state().get_saved_aois())
}

/// Menghapus file AOI berdasarkan ID dari `map_storage/aois/{id}.json` dan memory cache.
pub fn delete_saved_aoi(aoi_id: &str) -> Result<bool, MapControllerError> {
    get_global_state().delete_saved_aoi(aoi_id)
}
