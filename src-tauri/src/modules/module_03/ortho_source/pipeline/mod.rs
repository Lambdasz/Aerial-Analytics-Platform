//! Sub-modul pipeline impor citra orthomosaic dan GeoTIFF untuk Module 03.
//!
//! Terdiri dari komponen-komponen terpisah dengan batas tanggung jawab yang jelas:
//! - [`pipeline_validation_logic`]: Pemeriksaan awal fast-fail (ukuran berkas, magic bytes, dimensi, CRS bounds, preflight disk).
//! - [`image_resampling_logic`]: Deteksi progressive JPEG, IDCT-scaling, streaming decode PNG, serta box-filter resampling.
//! - [`adaptive_encoder_io`]: Kompresi adaptif display (JPEG kualitas/tangga dimensi & banded chunked PNG) dengan pembatalan I/O.
//! - [`orchestrator_option_a`]: Orkestrasi tahapan impor citra JPG/PNG dengan georeferensi eksternal/manual.
//! - [`orchestrator_option_b`]: Orkestrasi tahapan impor citra GeoTIFF dengan dekompresi streaming IFD/tile.

pub mod adaptive_encoder_io;
pub mod image_resampling_logic;
pub mod orchestrator_option_a;
pub mod orchestrator_option_b;
pub mod pipeline_validation_logic;

// Alias kompatibilitas mundur
pub use adaptive_encoder_io as encoding;
pub use image_resampling_logic as resampling;
pub use orchestrator_option_a as orchestrator_a;
pub use orchestrator_option_b as orchestrator_b;
pub use pipeline_validation_logic as validation;

pub use adaptive_encoder_io::*;
pub use image_resampling_logic::*;
pub use orchestrator_option_a::*;
pub use orchestrator_option_b::*;
pub use pipeline_validation_logic::*;
