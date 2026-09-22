//! Fungsi **bersisi efek samping**: menulis [`ExportTable`] (model murni dari
//! [`crate::reporting::aggregator`]) ke berkas ekspor di disk — CSV, Excel, atau
//! GeoJSON, sesuai fitur "Data Export" pada brief Module 11 ("Export results for
//! GIS, statistical, or further research analysis").
//!
//! Sama seperti `report.rs`: fungsi di sini melakukan I/O, jadi logika
//! penyusunan tabelnya diuji lewat pure unit test di `aggregator.rs`, bukan di
//! sini.

use std::path::{Path, PathBuf};

use super::error::ReportingError;
use super::types::output::ExportTable;

/// Tulis [`ExportTable`] sebagai berkas CSV di `output_path`.
pub fn export_to_csv(table: &ExportTable, output_path: &Path) -> Result<PathBuf, ReportingError> {
    let _ = (table, output_path);
    todo!("M11: tulis ExportTable sebagai CSV, mis. dengan crate `csv`")
}

/// Tulis [`ExportTable`] sebagai berkas Excel (`.xlsx`) di `output_path`.
pub fn export_to_excel(table: &ExportTable, output_path: &Path) -> Result<PathBuf, ReportingError> {
    let _ = (table, output_path);
    todo!(
        "M11: tulis ExportTable sebagai .xlsx (pilih: crate Rust seperti \
         `rust_xlsxwriter`, atau terima bytes .xlsx yang sudah dibuat frontend \
         lalu tulis ke disk — sama seperti keputusan render PDF di report.rs)"
    )
}

/// Tulis data plot (dengan geometrinya) sebagai berkas GeoJSON di `output_path`.
///
/// Catatan: geometri plot adalah tanggung jawab Module 9 (`Plot Definition`,
/// M9.1), bukan direkonstruksi di Module 11. Fungsi ini menganggap geometri
/// sudah ikut terbawa di data plot yang diterima dari Module 9 — bentuk
/// parameter pastinya menyesuaikan begitu tipe geometri resmi Module 9 tersedia
/// (lihat catatan di `types/input.rs`).
pub fn export_to_geojson(output_path: &Path) -> Result<PathBuf, ReportingError> {
    let _ = output_path;
    todo!(
        "M11: tulis data plot (termasuk geometri dari Module 9) sebagai \
         GeoJSON FeatureCollection"
    )
}
