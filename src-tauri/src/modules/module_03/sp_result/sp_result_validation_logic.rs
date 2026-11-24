//! Berkas ini berisi fungsi pembantu validasi payload hasil analitik spasial.
//!
//! Komponen dan metode validasi:
//! - `validate_spatial_result`: Memvalidasi geometri spasial dari objek hasil analitik.
//! - `validate_layer_payload`: Memvalidasi layer name, list features, and display mode.

use super::sp_result_struct::{LayerPayload, SpatialResult};
use crate::modules::module_03::error::MapControllerError;

/// Memvalidasi satu objek hasil spasial.
pub fn validate_spatial_result(result: &SpatialResult) -> Result<(), MapControllerError> {
    result.validate()
}

/// Memvalidasi seluruh konfigurasi dan data sebelum diteruskan ke frontend.
pub fn validate_layer_payload(payload: &LayerPayload) -> Result<(), MapControllerError> {
    payload.validate()
}
