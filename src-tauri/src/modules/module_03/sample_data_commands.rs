//! Tauri IPC commands untuk mengakses data sampel terpusat Module 3.

use crate::modules::module_03::types::aoi_type::AoiFeature;
use crate::modules::module_03::types::layer_type::{AnnotationFeature, LayerState};
use crate::modules::module_03::types::marker_type::DroneImageMetadata;

const MARKERS_JSON: &str = include_str!("../../../resources/sample-data/module3/markers.json");
const LAYERS_JSON: &str = include_str!("../../../resources/sample-data/module3/layers.json");
const AOI_JSON: &str = include_str!("../../../resources/sample-data/module3/aoi.json");
const ANNOTATIONS_JSON: &str =
    include_str!("../../../resources/sample-data/module3/annotations.json");

/// Mengambil daftar marker foto drone deterministik pada area orthomosaic Kariangau.
#[tauri::command]
pub fn get_project_image_markers() -> Result<Vec<DroneImageMetadata>, String> {
    serde_json::from_str(MARKERS_JSON)
        .map_err(|e| format!("Gagal parsing sample markers.json: {e}"))
}

/// Mengambil konfigurasi daftar layer awal peta.
#[tauri::command]
pub fn get_initial_map_layers() -> Result<Vec<LayerState>, String> {
    serde_json::from_str(LAYERS_JSON).map_err(|e| format!("Gagal parsing sample layers.json: {e}"))
}

/// Mengambil contoh Area of Interest (AOI) valid pada bounds Kariangau.
#[tauri::command]
pub fn get_sample_aoi() -> Result<Vec<AoiFeature>, String> {
    serde_json::from_str(AOI_JSON).map_err(|e| format!("Gagal parsing sample aoi.json: {e}"))
}

/// Mengambil contoh anotasi peta spasial pada bounds Kariangau.
#[tauri::command]
pub fn get_sample_annotations() -> Result<Vec<AnnotationFeature>, String> {
    serde_json::from_str(ANNOTATIONS_JSON)
        .map_err(|e| format!("Gagal parsing sample annotations.json: {e}"))
}
