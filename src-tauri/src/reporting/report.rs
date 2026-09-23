//! Fungsi **bersisi efek samping**: merender [`ReportDocument`] (model murni yang
//! dibangun oleh [`crate::reporting::aggregator`]) menjadi berkas laporan di
//! disk.
//!
//! Berbeda dari `aggregator.rs`, fungsi di file ini melakukan I/O sehingga tidak
//! diuji sebagai pure unit test — cukup pastikan logika penyusunan datanya
//! (di `aggregator.rs`) sudah tertutup test, sesuai aturan testing di
//! `RULES_REPORTING.md`.
//!
//! # Keputusan yang belum diambil
//!
//! Render PDF bisa dilakukan di sisi Rust (mis. crate `printpdf`/`genpdf`) atau
//! di sisi frontend (mis. `jspdf`) dengan Rust hanya menyimpan byte hasilnya ke
//! disk. **Belum diputuskan** yang mana yang dipakai — lihat pembahasan
//! dependency Module 11. Signature di bawah ini ditulis agar cocok untuk
//! skenario pertama (render di Rust); jika tim memilih skenario kedua, command
//! di `commands.rs` yang berubah (menerima bytes dari frontend), bukan fungsi
//! di file ini.

use std::path::{Path, PathBuf};

use super::error::ReportingError;
use super::types::output::ReportDocument;

/// Render [`ReportDocument`] menjadi berkas PDF di `output_path`.
///
/// # Error
/// - [`ReportingError::ReportRenderFailed`] jika proses rendering gagal.
/// - [`ReportingError::Io`] jika berkas gagal ditulis ke disk.
pub fn render_report_pdf(
    document: &ReportDocument,
    output_path: &Path,
) -> Result<PathBuf, ReportingError> {
    let _ = (document, output_path);
    todo!(
        "M11: render ReportDocument menjadi PDF (pilih: crate PDF Rust, atau \
         terima bytes PDF yang sudah dirender frontend lalu tulis ke disk)"
    )
}
