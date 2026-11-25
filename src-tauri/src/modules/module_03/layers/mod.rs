//! Berkas ini adalah modul agregator dan re-export untuk submodul Pengelolaan Layer Peta.
//!
//! Submodul yang terdaftar:
//! - `layer_ipc_cmd`: Endpoint Tauri Command untuk pembacaan konfigurasi layer.
//! - `layer_state_logic`: Logika fungsional murni manipulasi state layer (opacity, visibility, z-index).
//! - `layer_state_struct`: Definisi struktur data LayerState dan LayerError.
//! - `layer_storage_io`: Operasi I/O disk untuk persistensi konfigurasi layer di `map_storage/layers/`.

pub mod layer_ipc_cmd;
pub mod layer_state_logic;
pub mod layer_state_struct;
pub mod layer_storage_io;

pub use layer_ipc_cmd::*;
pub use layer_state_logic::*;
pub use layer_state_struct::*;

// Alias kompatibilitas mundur internal
pub use layer_ipc_cmd as commands;
pub use layer_state_logic as pure;
pub use layer_state_struct as types;
pub use layer_storage_io as store;
