//! Facade kompatibilitas penyimpanan Module 3 yang mendelegasikan ke unit atomic store masing-masing.

pub use crate::modules::module_03::annotations::store::*;
pub use crate::modules::module_03::aoi::store::{delete_saved_aoi, get_saved_aois, save_aoi};
pub use crate::modules::module_03::layers::store::*;
pub use crate::modules::module_03::ortho_source::store::{
    delete_saved_image, get_saved_images, save_image, save_image_from_path,
};
pub use crate::modules::module_03::ortho_source::types::{ImageCategory, StoredImageMetadata};
pub use crate::modules::module_03::photos::store::*;
pub use crate::modules::module_03::sp_result::store::{
    count_spatial_result_dependents, delete_saved_spatial_result, get_saved_spatial_result,
    get_saved_spatial_results, get_saved_spatial_results_summary, save_spatial_result,
    update_saved_spatial_result,
};
pub use crate::modules::module_03::storage_utils::*;
