//! Sub-modul pipeline impor citra orthomosaic dan GeoTIFF untuk Module 03.
//!
//! Terdiri dari komponen-komponen terpisah dengan batas tanggung jawab yang jelas:
//! - [`validation`]: Pemeriksaan awal fast-fail (ukuran berkas, magic bytes, dimensi, CRS bounds, preflight disk).
//! - [`resampling`]: Deteksi progressive JPEG, IDCT-scaling, streaming decode PNG, serta box-filter resampling.
//! - [`encoding`]: Kompresi adaptif display (JPEG kualitas/tangga dimensi & banded chunked PNG) dengan pembatalan I/O.
//! - [`orchestrator_a`]: Orkestrasi tahapan impor citra JPG/PNG dengan georeferensi eksternal/manual.
//! - [`orchestrator_b`]: Orkestrasi tahapan impor citra GeoTIFF dengan dekompresi streaming IFD/tile.

pub mod encoding;
pub mod orchestrator_a;
pub mod orchestrator_b;
pub mod resampling;
pub mod validation;

pub use encoding::*;
pub use orchestrator_a::*;
pub use orchestrator_b::*;
pub use resampling::*;
pub use validation::*;
