//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk manajemen layer peta.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `get_saved_layers`: Command untuk mengambil seluruh konfigurasi layer yang tersimpan di disk.

use super::layer_state_struct::LayerState;
use super::layer_storage_io;
use crate::plugin_manager::error::CommandError;

/// Mengambil seluruh layer yang tersimpan di storage.
#[tauri::command]
pub fn get_saved_layers() -> Result<Vec<LayerState>, CommandError> {
    Ok(layer_storage_io::get_saved_layers()?)
}
