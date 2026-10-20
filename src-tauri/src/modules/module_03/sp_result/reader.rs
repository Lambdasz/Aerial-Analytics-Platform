//! Pembaca data hasil analitik spasial (dummy/template).

use crate::modules::module_03::types::sp_result_type::MapLayerCommand;

/// Memuat dan mem-parsing data dummy hasil spasial yang di-embed saat compile time.
#[tauri::command]
pub fn get_dummy_spatial_layers() -> Result<Vec<MapLayerCommand>, String> {
    const DUMMY_JSON: &str = include_str!("../example_data/dummy_sp_result.json");
    serde_json::from_str(DUMMY_JSON).map_err(|e| format!("Gagal parsing dummy_sp_result.json: {e}"))
}

pub mod dummy {
    pub use super::get_dummy_spatial_layers;
}
