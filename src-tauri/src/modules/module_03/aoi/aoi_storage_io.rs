//! Berkas ini berisi fungsi-fungsi I/O penyimpanan disk untuk Area of Interest (AOI).
//!
//! Komponen dan fungsi yang didefinisikan dalam berkas ini:
//! - `get_aois_dir`: Memastikan dan mengembalikan path direktori penyimpanan AOI di disk.
//! - `save_aoi`: Menyimpan data GeoJSON AOI ke disk pada `map_storage/aois/{id}.json` secara atomik.
//! - `get_saved_aois`: Membaca seluruh data AOI yang tersimpan dari folder penyimpanan disk.
//! - `delete_saved_aoi`: Menghapus berkas AOI dari disk berdasarkan ID yang diberikan.

use std::fs;
use std::path::PathBuf;

use super::aoi_entity_struct::AoiFeature;
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
