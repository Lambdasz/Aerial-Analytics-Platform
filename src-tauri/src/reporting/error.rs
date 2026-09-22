//! Error handling `reporting`, mengikuti pola dua lapis yang sudah ditetapkan di
//! `map_controller/RULES_MAP_CONTROLLER.md`:
//!
//! 1. [`ReportingError`] — internal, dipakai di dalam Rust. `enum` supaya kompiler
//!    memaksa setiap varian ditangani secara eksplisit.
//! 2. [`CommandError`] — bentuk aman yang dikirim ke frontend lewat Serde
//!    (`code` + `message`), supaya detail internal Rust tidak bocor mentah-mentah
//!    ke TypeScript.
//!
//! Catatan: `map_controller/RULES_MAP_CONTROLLER.md` menyebutkan `CommandError`
//! sebaiknya dipindah ke `src-tauri/src/error.rs` bersama begitu lebih dari satu
//! modul membutuhkannya. `reporting` sengaja punya `CommandError` sendiri dulu
//! (bukan mengimpor dari `map_controller`) supaya tidak membuat modul ini
//! bergantung pada modul lain untuk hal yang tidak berhubungan langsung — begitu
//! sentralisasi itu benar-benar dilakukan, cukup ganti definisi lokal di file ini
//! jadi `pub use crate::error::CommandError;`, tanpa mengubah kontrak commands.rs.

use serde::Serialize;
use thiserror::Error;

/// Jenis-jenis error yang bisa terjadi di logika `reporting`.
#[derive(Error, Debug)]
pub enum ReportingError {
    /// Data dari Module 9/Module 10 yang dibutuhkan belum tersedia saat laporan
    /// diminta (mis. proyek belum punya plot, atau analisis temporal belum
    /// dijalankan).
    #[error("data input belum tersedia: {0}")]
    MissingInput(String),

    /// Data dari modul lain ada, tapi tidak valid/tidak lengkap untuk diproses
    /// jadi laporan (mis. daftar plot kosong, indikator ranking tidak dikenal).
    #[error("data input tidak valid: {0}")]
    InvalidInput(String),

    /// Gagal saat merender berkas laporan (PDF).
    #[error("gagal membuat laporan: {0}")]
    ReportRenderFailed(String),

    /// Gagal saat menulis berkas ekspor (Excel/CSV/GeoJSON).
    #[error("gagal membuat berkas ekspor: {0}")]
    ExportFailed(String),

    /// Gagal I/O generik (baca/tulis berkas ke disk).
    #[error("gagal I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl ReportingError {
    /// Kode pendek stabil untuk dicocokkan (`switch`/`match`) di sisi TypeScript.
    fn code(&self) -> &'static str {
        match self {
            Self::MissingInput(_) => "REPORTING_MISSING_INPUT",
            Self::InvalidInput(_) => "REPORTING_INVALID_INPUT",
            Self::ReportRenderFailed(_) => "REPORTING_RENDER_FAILED",
            Self::ExportFailed(_) => "REPORTING_EXPORT_FAILED",
            Self::Io(_) => "REPORTING_IO_ERROR",
        }
    }
}

/// Bentuk error yang aman dikirim ke frontend: hanya `code` (untuk logika) dan
/// `message` (untuk ditampilkan ke pengguna).
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<ReportingError> for CommandError {
    fn from(err: ReportingError) -> Self {
        // TODO(M11): tambahkan logging terpusat di sini (mis. crate `log`, yang
        // saat ini belum ada di Cargo.toml) begitu strategi logging proyek
        // disepakati — jangan panggil macro log tersebar di tiap command.
        CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}
