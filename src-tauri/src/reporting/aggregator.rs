//! Fungsi **murni** (pure function): menggabungkan data dari Module 9
//! (Plot Ranking + Plot Summary) dan Module 10 (Change Visualization) menjadi
//! model laporan/ekspor siap pakai.
//!
//! Semua fungsi di file ini **wajib**:
//! - tidak melakukan I/O (baca/tulis berkas, panggil command Tauri lain, dsb);
//! - tidak `async`;
//! - deterministik — keluaran hanya bergantung pada argumen yang diberikan;
//! - punya unit test lewat `#[cfg(test)]` begitu implementasi nyata ditulis,
//!   sesuai aturan testing di `RULES_REPORTING.md`.
//!
//! Efek samping (menulis ke berkas PDF/Excel/CSV) ada di [`crate::reporting::report`]
//! dan [`crate::reporting::export`], bukan di sini.

use super::error::ReportingError;
use super::types::input::{PlotRankingResult, PlotSummary, TemporalChangeResult};
use super::types::output::{
    ExportTable, PlotReportSection, ReportDocument, ReportMetadata, TemporalReportSection,
};

/// Gabungkan ringkasan per plot (M9.7) dengan hasil peringkatnya (M9.6) menjadi
/// satu bagian laporan.
///
/// # Error
/// Mengembalikan [`ReportingError::InvalidInput`] jika `summaries` kosong, atau
/// jika ada entri di `ranking` yang `plot_id`-nya tidak ditemukan di `summaries`
/// (kontrak antara M9.6 dan M9.7 dianggap tidak konsisten).
pub fn build_plot_report_section(
    summaries: &[PlotSummary],
    ranking: &PlotRankingResult,
) -> Result<PlotReportSection, ReportingError> {
    let _ = (summaries, ranking);
    todo!(
        "M11: gabungkan `summaries` (M9.7) dengan `ranking.entries` (M9.6) \
         berdasarkan plot_id yang sama menjadi Vec<PlotReportRow>"
    )
}

/// Ubah hasil perbandingan temporal (M10.8, lewat `get_temporal_change_result()`)
/// menjadi bagian laporan ringkasan temporal (M11.5 — Temporal Summary).
pub fn build_temporal_report_section(change: &TemporalChangeResult) -> TemporalReportSection {
    let _ = change;
    todo!("M11: petakan TemporalChangeResult (M10.8) menjadi TemporalReportSection")
}

/// Rakit dokumen laporan lengkap dari bagian-bagian yang sudah dibangun oleh
/// fungsi murni lain di modul ini.
///
/// `temporal` bertipe `Option` karena laporan tetap valid dibuat untuk proyek
/// yang baru punya satu observation period (belum ada data temporal untuk
/// dibandingkan).
pub fn build_report_document(
    metadata: ReportMetadata,
    plots: PlotReportSection,
    temporal: Option<TemporalReportSection>,
) -> ReportDocument {
    let _ = (metadata, plots, temporal);
    todo!("M11: rakit ReportDocument dari bagian-bagian yang sudah dibangun")
}

/// Ubah ringkasan per plot (M9.7) menjadi tabel generik siap ekspor
/// (dikonsumsi oleh `export::export_to_csv`/`export_to_excel`).
///
/// Dipisah dari [`build_plot_report_section`] karena kebutuhan kolom untuk
/// ekspor tabular (satu baris per plot, kolom rata/min/maks per indikator)
/// berbeda bentuk dari kebutuhan tampilan laporan naratif.
pub fn build_plot_export_table(summaries: &[PlotSummary]) -> ExportTable {
    let _ = summaries;
    todo!("M11: bentuk ExportTable (headers + baris) dari Vec<PlotSummary>")
}

#[cfg(test)]
mod tests {
    // TODO(M11): tambahkan unit test murni di sini begitu fungsi di atas
    // diimplementasikan, sesuai aturan testing di RULES_REPORTING.md — fungsi di
    // file ini tidak butuh konteks Tauri sama sekali sehingga mudah diuji dengan
    // `cargo test`.
}
