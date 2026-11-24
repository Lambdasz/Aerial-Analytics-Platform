//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk manajemen layer peta.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `get_saved_layers`: Command untuk mengambil seluruh konfigurasi layer yang tersimpan di SQLite.

use super::layer_state_struct::LayerState;
use super::layer_storage_io;
use crate::plugin_manager::error::CommandError;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DATABASE_ERROR".to_string(),
        message: format!("Gagal mengakses database: {e}"),
    }
}

/// Mengambil seluruh layer yang tersimpan di storage SQLite.
#[tauri::command]
pub fn get_saved_layers(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<LayerState>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(layer_storage_io::get_saved_layers(&conn)?)
}
