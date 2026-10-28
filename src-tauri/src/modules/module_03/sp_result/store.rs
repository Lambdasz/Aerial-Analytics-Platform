//! Facade persistensi untuk Spatial Results (`map_storage/spatial_results/`).
//!
//! Modul ini telah direfaktor dan didekomposisi ke dalam submodul terpisah:
//! - [`reader`](super::reader): Dual-reader engine (folder dekomposisi & flat file JSON), listing, query, dan ekstraktor metadata.
//! - [`writer`](super::writer): Penulisan atomik format dekomposisi (`StoredLayerFile`, `StoredFeatureFile`) dan penanganan gambar.
//! - [`patch`](super::patch): Pembaruan semantik field aman (nama layer, warna, icon, custom fields, feature edits) & audit edit manual.
//! - [`quarantine`](super::quarantine): Penghapusan transaksional 2-fase, karantina sementara, rollback otomatis, dan pemulihan berkas.
//!
//! Seluruh fungsi publik tetap di-reexport di sini untuk menjaga kompatibilitas 100% dengan
//! modul-modul lain (Module 10, Tauri command handlers, dan test suite).

pub use super::patch::*;
pub use super::quarantine::*;
pub use super::reader::*;
pub use super::writer::*;
