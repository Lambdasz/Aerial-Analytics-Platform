//! Berkas ini berisi dispatcher event untuk pengiriman data spatial result ke frontend.
//!
//! Komponen dan fungsi yang didefinisikan dalam berkas ini:
//! - `dispatch_spatial_result`: Memvalidasi payload dan memancarkan event Tauri `map://spatial-result` ke antarmuka Leaflet.

use super::sp_result_struct::{LayerPayload, MapLayerCommand, SPATIAL_RESULT_EVENT};
use crate::modules::module_03::error::MapControllerError;
use tauri::{AppHandle, Emitter};

/// Memvalidasi dan mengirimkan payload layer spasial ke frontend.
pub fn dispatch_spatial_result(
    app_handle: &AppHandle,
    layer_payload: LayerPayload,
) -> Result<(), MapControllerError> {
    layer_payload.validate()?;

    let command = MapLayerCommand {
        action: "load_spatial_result".to_string(),
        payload: layer_payload,
    };

    app_handle
        .emit(SPATIAL_RESULT_EVENT, command)
        .map_err(|e| MapControllerError::InvalidGeometry(format!("Gagal emit event: {}", e)))?;

    Ok(())
}
