//! Submodul kontrak data untuk Module 3.

pub mod aoi_type;
pub mod layer_type;
pub mod sp_result_type;

pub use aoi_type::*;
pub use layer_type::*;
pub use sp_result_type::*;

// Modul kompatibilitas ke belakang untuk kode yang mengimpor sp_measurement_type
pub mod sp_measurement_type {
    pub use super::aoi_type::*;
    pub use super::sp_result_type::*;
}
