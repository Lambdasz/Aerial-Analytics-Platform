//! Dispatcher hasil analitik spasial ke antarmuka Leaflet frontend melalui Tauri event.

use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::types::sp_result_type::{
    LayerPayload, MapLayerCommand, SPATIAL_RESULT_EVENT,
};
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
