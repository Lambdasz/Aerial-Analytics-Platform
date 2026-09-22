//! Model Report + Export — tipe-tipe ini **milik Module 11 sendiri**, tidak
//! merepresentasikan kontrak modul lain, jadi bebas berubah sesuai kebutuhan
//! implementasi report/export tanpa perlu koordinasi lintas modul.

use serde::{Deserialize, Serialize};

use super::input::{ChangeAreaEntry, ObservationPeriod, PlotIndicatorStat};

/// Format berkas ekspor yang didukung ("Data Export" pada brief Module 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Xlsx,
    GeoJson,
}

/// Metadata umum satu laporan (identitas proyek, waktu dibuat, dsb).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub project_id: String,
    pub project_name: String,
    /// Timestamp ISO 8601 saat laporan dibuat.
    pub generated_at: String,
}

/// Baris ringkasan satu plot pada bagian laporan — gabungan dari [`PlotSummary`]
/// (M9.7) dan peringkatnya di [`PlotRankingEntry`] (M9.6), sudah dalam bentuk
/// siap ditampilkan/dicetak.
///
/// [`PlotSummary`]: super::input::PlotSummary
/// [`PlotRankingEntry`]: super::input::PlotRankingEntry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotReportRow {
    pub plot_id: String,
    pub plot_name: String,
    pub area_ha: f64,
    pub indicators: Vec<PlotIndicatorStat>,
    /// `None` jika plot ini tidak termasuk dalam indikator yang sedang
    /// diperingkat.
    pub rank: Option<u32>,
}

/// Bagian laporan yang merangkum seluruh plot — hasil gabungan M9.6 + M9.7.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotReportSection {
    /// Indikator yang dipakai sebagai dasar ranking pada bagian ini (dari
    /// [`PlotRankingResult::indicator`]).
    ///
    /// [`PlotRankingResult::indicator`]: super::input::PlotRankingResult
    pub ranking_indicator: String,
    pub rows: Vec<PlotReportRow>,
}

/// Bagian laporan untuk ringkasan perubahan temporal (M11.5 — Temporal
/// Summary), dibangun dari keluaran `get_temporal_change_result()` (M10.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalReportSection {
    pub period_before: ObservationPeriod,
    pub period_after: ObservationPeriod,
    pub vegetation_coverage_change_pct: f64,
    pub tree_count_change: Option<i64>,
    pub significant_changes: Vec<ChangeAreaEntry>,
}

/// Model laporan lengkap, siap dirender jadi berkas oleh [`crate::reporting::report`].
///
/// Ini murni struct data (dibangun oleh fungsi murni di
/// [`crate::reporting::aggregator`]) — tidak tahu-menahu soal PDF, file, atau I/O.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDocument {
    pub metadata: ReportMetadata,
    pub plots: PlotReportSection,
    pub temporal: Option<TemporalReportSection>,
}

/// Satu baris data tabular generik, dipakai sebagai representasi perantara
/// sebelum ditulis ke CSV/Excel oleh [`crate::reporting::export`].
///
/// Menyimpan nilai sebagai `String` yang sudah diformat (bukan `f64` mentah)
/// supaya fungsi penulis berkas di `export.rs` tidak perlu tahu aturan
/// formatting/pembulatan tiap indikator — itu tanggung jawab
/// [`crate::reporting::aggregator`] yang membangun tabel ini.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRow {
    pub cells: Vec<String>,
}

/// Tabel data siap ekspor — hasil dari fungsi murni di `aggregator.rs`,
/// dikonsumsi oleh fungsi bersisi efek samping di `export.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTable {
    pub headers: Vec<String>,
    pub rows: Vec<ExportRow>,
}
