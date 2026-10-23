//! Batas Tauri (`#[tauri::command]`) untuk Report + Export.
//!
//! Setiap command di sini mengorkestrasi tiga tahap, dan **tidak boleh berisi
//! logika penggabungan data sendiri** — logika itu ada di
//! [`crate::reporting::aggregator`]:
//!
//! 1. **Ambil data** dari Module 9 (`get_plot_ranking`, `get_plot_summary` — nama
//!    tentatif, lihat catatan di `types/input.rs`) dan Module 10
//!    (`get_temporal_change_result`).
//! 2. **Gabungkan** lewat fungsi murni di `aggregator.rs`.
//! 3. **Render/tulis berkas** lewat fungsi bersisi efek samping di `report.rs`
//!    atau `export.rs`.
//!
//! # Belum didaftarkan di `invoke_handler!`
//!
//! Command di file ini **sengaja belum ditambahkan** ke
//! `tauri::generate_handler![...]` pada `src-tauri/src/lib.rs`. Itu langkah
//! terakhir yang dilakukan begitu tahap 1 (pemanggilan fungsi Module 9/10 asli)
//! benar-benar bisa dikompilasi — mendaftarkannya sekarang hanya akan membuat
//! command yang bisa dipanggil dari frontend tapi selalu panic (`todo!()`).

use super::error::CommandError;
use super::types::output::ExportFormat;

/// Generate laporan (PDF) untuk satu proyek, mencakup ringkasan per plot
/// (M9.6 + M9.7) dan ringkasan temporal (M10.8) jika tersedia.
///
/// Mengembalikan path absolut berkas PDF yang dihasilkan.
#[tauri::command]
pub async fn generate_report(project_id: String) -> Result<String, CommandError> {
    let _ = project_id;
    todo!(
        "M11: (1) ambil PlotSummary+PlotRankingResult dari Module 9 dan \
         TemporalChangeResult dari Module 10 lewat get_temporal_change_result(); \
         (2) susun lewat aggregator::build_plot_report_section + \
         build_temporal_report_section + build_report_document; \
         (3) tulis lewat report::render_report_pdf; kembalikan path-nya."
    )
}

/// Ekspor data plot satu proyek ke format yang dipilih pengguna
/// (CSV/Excel/GeoJSON — "Data Export" pada brief Module 11).
///
/// Mengembalikan path absolut berkas ekspor yang dihasilkan.
#[tauri::command]
pub async fn export_data(project_id: String, format: ExportFormat) -> Result<String, CommandError> {
    let _ = (project_id, format);
    todo!(
        "M11: (1) ambil Vec<PlotSummary> dari Module 9; (2) bentuk tabel lewat \
         aggregator::build_plot_export_table; (3) tulis lewat \
         export::export_to_csv / export_to_excel / export_to_geojson sesuai \
         `format`."
    )
}
