//! Submodul kontrak data untuk Module 3.

pub use crate::modules::module_03::annotations::annotation_entity_struct as marker_type;
pub use crate::modules::module_03::aoi::aoi_entity_struct as aoi_type;
pub use crate::modules::module_03::layers::layer_state_struct as layer_type;

pub mod sp_result_type;
pub use sp_result_type::*;

// Modul kompatibilitas ke belakang untuk kode yang mengimpor sp_measurement_type
pub mod sp_measurement_type {
    pub use super::aoi_type::*;
    pub use super::sp_result_type::*;
}
