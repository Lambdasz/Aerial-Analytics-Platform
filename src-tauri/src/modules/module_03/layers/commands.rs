//! Tauri IPC commands untuk pembacaan konfigurasi layer peta.

use super::store;
use super::types::LayerState;
use crate::plugin_manager::error::CommandError;

/// Mengambil seluruh layer yang tersimpan di storage.
#[tauri::command]
pub fn get_saved_layers() -> Result<Vec<LayerState>, CommandError> {
    Ok(store::get_saved_layers()?)
}
