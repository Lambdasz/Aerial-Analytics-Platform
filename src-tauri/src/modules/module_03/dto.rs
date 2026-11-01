//! Pusat Data Transfer Objects (DTO) untuk Modul 03 (Aerial Image Map Explorer).
//!
//! Seluruh DTO mengimplementasikan serialisasi dan deserialisasi JSON dengan konvensi `camelCase`
//! agar selaras dengan kontrak IPC Tauri antara frontend TypeScript dan backend Rust.

pub use super::annotations::dto::*;
pub use super::aoi::dto::*;
pub use super::geometry::dto::*;
pub use super::ortho_source::dto::*;
