//! Re-export facade untuk command storage (AOI, Spatial Results, dan Citra).

pub use crate::modules::module_03::aoi::commands::{delete_saved_aoi, get_saved_aois, save_aoi};
pub use crate::modules::module_03::ortho_source::commands::{
    delete_saved_image, get_saved_images, import_ortho_image, save_image_from_path,
};
pub use crate::modules::module_03::sp_result::commands::{
    check_spatial_result_manual_edits, count_spatial_result_dependents,
    delete_saved_spatial_result, get_saved_spatial_result, get_saved_spatial_results,
    get_saved_spatial_results_summary, save_spatial_result, update_saved_spatial_result,
};

/// Mengembalikan canonical path absolut direktori map_storage untuk frontend resolveImageSrc.
#[tauri::command]
pub fn get_storage_root_path() -> Result<String, String> {
    let path = crate::modules::module_03::storage_utils::get_storage_root();
    let abs = std::fs::canonicalize(&path).unwrap_or(path);
    let s = abs.to_string_lossy().to_string();
    let clean = s.strip_prefix(r"\\?\").unwrap_or(&s).replace('\\', "/");
    Ok(clean)
}
