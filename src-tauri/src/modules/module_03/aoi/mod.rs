//! Berkas ini adalah modul agregator dan re-export untuk submodul Area of Interest (AOI).
//!
//! Submodul yang terdaftar:
//! - `aoi_builder_logic`: Logika fungsional murni pembentukan GeoJSON AOI Feature.
//! - `aoi_entity_struct`: Definisi struktur data GeoJSON AOI (Coordinate, Polygon, AoiFeature).
//! - `aoi_ipc_cmd`: Endpoint Tauri Command untuk siklus hidup AOI.
//! - `aoi_ipc_struct`: Struct DTO Request/Response untuk IPC AOI.
//! - `aoi_storage_io`: Operasi I/O disk untuk persistensi atomik file JSON AOI.

pub mod aoi_builder_logic;
pub mod aoi_entity_struct;
pub mod aoi_ipc_cmd;
pub mod aoi_ipc_struct;
pub mod aoi_storage_io;

pub use aoi_builder_logic::create_aoi;
pub use aoi_entity_struct::*;
pub use aoi_ipc_cmd::{
    create_aoi_cmd, delete_saved_aoi, get_saved_aois, save_aoi, save_new_aoi_cmd,
};
pub use aoi_ipc_struct::*;

// Alias kompatibilitas mundur internal
pub use aoi_builder_logic as builder;
pub use aoi_entity_struct as types;
pub use aoi_ipc_cmd as commands;
pub use aoi_ipc_struct as dto;
pub use aoi_storage_io as store;
