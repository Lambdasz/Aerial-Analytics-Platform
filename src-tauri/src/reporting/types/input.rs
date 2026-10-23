//! Kontrak data masuk dari Module 9 (Area & Plot Analytics) dan Module 10
//! (Simple Temporal Change Analysis).
//!
//! # ⚠️ Catatan penting: ini adalah tipe SEMENTARA
//!
//! Module 9 dan Module 10 **belum punya kode Rust sama sekali** di repo ini saat
//! modul ini ditulis. Supaya `reporting` tetap bisa dikompilasi dan
//! didokumentasikan sekarang (bukan menunggu dua modul lain selesai), tipe-tipe di
//! bawah ini adalah **replika lokal** dari kontrak yang sudah disepakati tim
//! (lihat tabel M9.1–M9.7 dan kontrak `get_temporal_change_result()` di
//! dokumentasi proyek), bukan tipe asli milik Module 9/10.
//!
//! **Begitu Module 9 dan Module 10 mem-publish tipe resmi mereka**
//! (mis. `crate::plot_analytics::types::PlotSummary`,
//! `crate::temporal_analysis::types::TemporalChangeResult`), tipe-tipe di file ini
//! **wajib dihapus** dan diganti `use` langsung ke tipe asli tersebut — bukan
//! dipertahankan sebagai duplikat. Sampai saat itu tiba, field di bawah ini boleh
//! berubah mengikuti diskusi lanjutan dengan pemilik Module 9/Module 10; jangan
//! anggap ini kontrak final.
//!
//! Module 11 **tidak pernah mengubah file milik Module 9/Module 10** untuk
//! menyesuaikan kontrak ini — sinkronisasi selalu satu arah dari sana ke sini.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Module 9.6 — Plot Ranking
// Input (bagi Module 9): statistik plot + indikator + arah urutan.
// Output (bagi Module 11): daftar plot terurut beserta peringkatnya.
// ---------------------------------------------------------------------------

/// Arah pengurutan yang dipilih pengguna saat me-ranking plot (bagian dari input
/// M9.6, ikut disertakan di sini supaya hasil ranking bisa ditampilkan dengan
/// konteks yang benar di laporan — "diurutkan dari tertinggi/terendah").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingDirection {
    Ascending,
    Descending,
}

/// Satu baris hasil peringkat plot untuk satu indikator (keluaran M9.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotRankingEntry {
    pub plot_id: String,
    pub plot_name: String,
    /// Nilai indikator yang dipakai sebagai dasar ranking (mis. rata-rata
    /// vegetation coverage %).
    pub value: f64,
    /// Peringkat, 1 = teratas sesuai `direction`.
    pub rank: u32,
}

/// Keluaran lengkap M9.6 untuk satu indikator terpilih.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotRankingResult {
    /// Nama/kode indikator yang dipakai untuk ranking (mis. "vegetation_coverage_pct").
    pub indicator: String,
    pub direction: RankingDirection,
    pub entries: Vec<PlotRankingEntry>,
}

// ---------------------------------------------------------------------------
// Module 9.7 — Plot Summary
// Input (bagi Module 9): seluruh hasil M9.2–M9.6.
// Output (bagi Module 11): ringkasan JSON terstruktur per plot.
// ---------------------------------------------------------------------------

/// Statistik satu indikator untuk satu plot (hasil M9.4: rata-rata, min, maks,
/// simpangan baku).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotIndicatorStat {
    pub indicator: String,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub std_dev: f64,
}

/// Ringkasan terstruktur satu plot (keluaran M9.7) — inilah bentuk data utama
/// yang dikonsumsi Module 11 untuk bagian "Analysis Result Summary" dan
/// "Plot Comparison Dashboard" pada laporan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotSummary {
    pub plot_id: String,
    pub plot_name: String,
    /// Luas plot dalam meter persegi (hasil M9.2).
    pub area_m2: f64,
    /// Luas plot dalam hektare (hasil M9.2).
    pub area_ha: f64,
    /// Keliling plot dalam meter (hasil M9.2).
    pub perimeter_m: f64,
    /// Statistik per indikator (hasil M9.4), setelah nilainya dipotong ke batas
    /// plot (hasil M9.3).
    pub indicators: Vec<PlotIndicatorStat>,
}

// ---------------------------------------------------------------------------
// Module 10.8 — Change Visualization, diakses lewat get_temporal_change_result()
// Output: data perubahan antar observation period, dikonsumsi Module 11 (M11.5
// Temporal Summary) untuk membuat ringkasan perubahan berdasarkan periode
// pengamatan.
// ---------------------------------------------------------------------------

/// Satu titik waktu akuisisi (before/after) yang dibandingkan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationPeriod {
    pub label: String,
    pub acquisition_date: String,
    /// Referensi ke citra (path/id) untuk periode ini, dipakai ulang di laporan
    /// tanpa perlu me-render ulang peta.
    pub image_ref: String,
}

/// Satu lokasi/area yang terdeteksi mengalami perubahan signifikan
/// ("Change Area Detection" pada brief Module 10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeAreaEntry {
    pub area_id: String,
    /// Deskripsi singkat perubahan (mis. "kehilangan vegetasi").
    pub description: String,
    /// Besaran perubahan pada area ini ("Change Magnitude Analysis").
    pub magnitude: f64,
}

/// Hasil lengkap perbandingan dua observation period, dikembalikan oleh
/// `get_temporal_change_result()` milik Module 10.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalChangeResult {
    pub period_before: ObservationPeriod,
    pub period_after: ObservationPeriod,
    /// Perubahan persentase tutupan vegetasi antar periode (poin persentase).
    pub vegetation_coverage_change_pct: f64,
    /// Perubahan jumlah pohon terdeteksi antar periode, jika Module 7 aktif
    /// untuk kedua periode.
    pub tree_count_change: Option<i64>,
    pub change_areas: Vec<ChangeAreaEntry>,
}
