//! Berkas ini adalah modul agregator dan re-export untuk submodul Catatan Anotasi Peta.
//!
//! Submodul yang terdaftar:
//! - `annotation_builder_logic`: Logika fungsional murni pembentukan AnnotationFeature.
//! - `annotation_entity_struct`: Definisi struktur data GeoJSON AnnotationFeature.
//! - `annotation_ipc_cmd`: Endpoint Tauri Command untuk CRUD anotasi inspeksi peta.
//! - `annotation_ipc_struct`: Struct DTO Request/Response untuk IPC anotasi.
//! - `annotation_storage_io`: Operasi I/O disk untuk persistensi file JSON anotasi di `map_storage/annotations/`.

pub mod annotation_builder_logic;
pub mod annotation_entity_struct;
pub mod annotation_ipc_cmd;
pub mod annotation_ipc_struct;
pub mod annotation_storage_io;

pub use annotation_builder_logic::*;
pub use annotation_entity_struct::*;
pub use annotation_ipc_cmd::*;
pub use annotation_ipc_struct::*;

// Alias kompatibilitas mundur internal
pub use annotation_builder_logic as pure;
pub use annotation_entity_struct as types;
pub use annotation_ipc_cmd as commands;
pub use annotation_ipc_struct as dto;
pub use annotation_storage_io as store;
