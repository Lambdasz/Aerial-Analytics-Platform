//! Pembaca data hasil analitik spasial (dummy/template).

use crate::modules::module_03::types::sp_result_type::MapLayerCommand;

/// Memuat dan mem-parsing data dummy hasil spasial yang di-embed saat compile time.
#[tauri::command]
pub fn get_dummy_spatial_layers() -> Result<Vec<MapLayerCommand>, String> {
    const SPATIAL_JSON: &str =
        include_str!("../../../../resources/sample-data/module3/spatial_results.json");
    serde_json::from_str(SPATIAL_JSON)
        .map_err(|e| format!("Gagal parsing sample spatial_results.json: {e}"))
}

pub mod dummy {
    pub use super::get_dummy_spatial_layers;
}
