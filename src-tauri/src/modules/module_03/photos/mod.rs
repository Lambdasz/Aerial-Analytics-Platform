//! Berkas ini adalah modul agregator dan re-export untuk submodul Foto Drone & Metadata EXIF.
//!
//! Submodul yang terdaftar:
//! - `drone_exif_logic`: Logika fungsional murni parsing tag EXIF dari bytes citra.
//! - `drone_photo_ipc_cmd`: Endpoint Tauri Command untuk impor dan query foto drone.
//! - `drone_photo_storage_io`: Operasi I/O disk untuk penyimpanan foto dan thumbnail.
//! - `drone_photo_struct`: Definisi struktur data PhotoMetadata dan DroneImageMetadata.

pub mod drone_exif_logic;
pub mod drone_photo_ipc_cmd;
pub mod drone_photo_storage_io;
pub mod drone_photo_struct;

pub use drone_exif_logic::*;
pub use drone_photo_ipc_cmd::*;
pub use drone_photo_struct::*;

// Alias kompatibilitas mundur internal
pub use drone_exif_logic as pure;
pub use drone_photo_ipc_cmd as commands;
pub use drone_photo_storage_io as store;
pub use drone_photo_struct as types;
