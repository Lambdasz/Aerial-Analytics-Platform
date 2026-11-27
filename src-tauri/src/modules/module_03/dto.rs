//! Berkas ini merupakan payung re-export Data Transfer Object (DTO) untuk Modul 03.
//!
//! Seluruh DTO mengimplementasikan serialisasi dan deserialisasi JSON dengan konvensi `camelCase`
//! agar selaras dengan kontrak IPC Tauri antara frontend TypeScript dan backend Rust.

pub use super::annotations::annotation_ipc_struct::*;
pub use super::aoi::aoi_ipc_struct::*;
pub use super::geometry::geometry_ipc_struct::*;
pub use super::ortho_source::ortho_import_ipc_struct::*;
