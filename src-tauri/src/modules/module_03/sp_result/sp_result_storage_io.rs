//! Berkas ini bertindak sebagai fasad I/O penyimpanan Spatial Result (`map_storage/spatial_results/`).
//!
//! Daftar Komponen / Fasad I/O:
//! - [`sp_result_reader_io`]: Dual-reader engine (folder dekomposisi & flat file JSON), listing, query, dan ekstraktor metadata.
//! - [`sp_result_writer_io`]: Penulisan atomik format dekomposisi (`StoredLayerFile`, `StoredFeatureFile`) dan penanganan gambar.
//! - [`sp_result_patch_io`]: Pembaruan semantik field aman (nama layer, warna, icon, custom fields, feature edits) & audit edit manual.
//! - [`sp_result_quarantine_io`]: Penghapusan transaksional 2-fase, karantina sementara, rollback otomatis, dan pemulihan berkas.

pub use super::sp_result_patch_io::*;
pub use super::sp_result_quarantine_io::*;
pub use super::sp_result_reader_io::*;
pub use super::sp_result_writer_io::*;
