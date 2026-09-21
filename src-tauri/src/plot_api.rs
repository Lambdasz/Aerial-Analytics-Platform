//! API publik Modul 9 (Area & Plot Analytics) untuk komunikasi antar modul.
//!
//! Modul 9 mengelola plot (poligon batas area analisis) dan menjadi
//! penghubung antara peta (Modul 3), plugin manager (Modul 2), dan dashboard
//! (Modul 11). Berkas ini merujuk sequence diagram "Analisis Per Plot":
//!
//! 1. User menggambar plot di peta (Modul 3).
//! 2. Modul 3 -> Modul 9: data pembatas plot.
//! 3. Modul 9 -> Modul 2: menjalankan plugin per plot.
//! 4. Modul 2 -> Modul 9: hasil per plot (balasan langkah 3).
//! 5. Modul 9 -> Modul 3: layer plot (balasan langkah 2).
//! 6. Modul 9 -> Modul 11: hasil plugin dan statistik.
//!
//! Modul 4-8 tidak berkomunikasi langsung dengan Modul 9; semuanya lewat
//! Modul 2. Modul 10 tidak berhubungan dengan Modul 9.
//!
//! Fungsi yang tersedia (semuanya masih kerangka, isinya `todo!`):
//!
//! | Fungsi                 | Modul | Langkah |
//! |------------------------|-------|---------|
//! | [`define_plot`]        | 3     | 2 dan 5 |
//! | [`analyze_plot`]       | 2     | 3 dan 4 |
//! | [`get_plot_statistics`]| 11    | 6       |
//!
//! Konvensi geometri: GeoJSON `Feature<Polygon>` RFC 7946, WGS84 (EPSG:4326),
//! urutan `[lon, lat]`. Penyimpanan plot belum diputuskan; API ini tidak
//! mengasumsikan SQLite maupun file system.

use crate::plugin_manager::result_handler::ExecutionResult;
use geojson::Feature;
use serde::{Deserialize, Serialize};

/// Layer plot yang dikembalikan ke Modul 3 untuk ditampilkan di peta.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlotLayer {
    /// ID unik plot (UUID dalam bentuk string).
    pub plot_id: String,
    /// Nama plot yang ditampilkan ke pengguna.
    pub plot_name: String,
    /// Geometri plot: `Feature<Polygon>` WGS84 persis seperti
    /// `docs/examples/aoi.example.geojson`. Di `properties` dipakai nama field
    /// Modul 3: `aoi_id` (= `plot_id`) dan `name` (= `plot_name`).
    pub geometry: Feature,
}

/// Hasil analisis satu plugin pada satu plot.
#[derive(Serialize, Deserialize, Debug)]
pub struct PluginRunResult {
    /// ID plugin yang dijalankan.
    pub plugin_id: String,
    /// Hasil eksekusi dari Modul 2 (`ExecutionResult` milik plugin_manager).
    pub result: ExecutionResult,
}

/// Hasil analisis satu plot untuk semua plugin yang dijalankan.
#[derive(Serialize, Deserialize, Debug)]
pub struct PlotAnalysisResult {
    /// ID plot yang dianalisis.
    pub plot_id: String,
    /// Hasil per plugin, sesuai urutan `plugin_ids` pada permintaan.
    pub results: Vec<PluginRunResult>,
}

/// Statistik ringkas sebuah indikator pada satu plot.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndicatorStats {
    /// Rata-rata nilai.
    pub mean: Option<f64>,
    /// Nilai minimum.
    pub min: Option<f64>,
    /// Nilai maksimum.
    pub max: Option<f64>,
    /// Simpangan baku.
    pub stddev: Option<f64>,
}

/// Satu indikator hasil plugin pada sebuah plot.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlotIndicator {
    /// Nama indikator, misalnya `"vegetation_cover"`.
    pub name: String,
    /// Satuan nilai, misalnya `"%"` atau `"pohon"`.
    pub unit: String,
    /// Plugin atau modul sumber indikator.
    pub source: String,
    /// Nilai utama indikator.
    pub value: f64,
    /// Statistik tambahan, kosong bila plugin tidak menyediakannya.
    pub stats: Option<IndicatorStats>,
}

/// Statistik satu plot untuk dashboard Modul 11.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlotStatistics {
    /// ID plot.
    pub plot_id: String,
    /// Nama plot.
    pub plot_name: String,
    /// Luas dalam meter persegi (dari Spatial Measurement Modul 3).
    pub area_m2: f64,
    /// Luas dalam hektare (dari Spatial Measurement Modul 3).
    pub area_ha: f64,
    /// Keliling dalam meter (dari Spatial Measurement Modul 3).
    pub perimeter_m: f64,
    /// Daftar indikator hasil plugin pada plot ini.
    pub indicators: Vec<PlotIndicator>,
}

/// Mendefinisikan plot dari data pembatas yang digambar di Modul 3.
///
/// Modul integrasi: Modul 3
///
/// Arah: Modul 3 -> Modul 9 -> Modul 3
///
/// Langkah diagram: 2 dan 5
///
/// Deskripsi: Menerima AOI dari Modul 3, memvalidasi geometrinya dengan
/// aturan Modul 3 (poligon tertutup, minimal 3 titik, tidak
/// self-intersecting; lihat `RULES_MAP_CONTROLLER.md`), menyimpannya sebagai
/// plot, lalu mengembalikan layer plot untuk ditampilkan Modul 3. Nama plot
/// diambil dari `properties.name`.
///
/// Pemetaan field: geometri yang dikirim ke atau diterima dari Modul 3
/// memakai nama field Modul 3 di `properties`, sedangkan struct internal
/// Modul 9 tetap memakai nama sendiri: `aoi_id` = `plot_id`, `name` =
/// `plot_name`.
///
/// Modul 3 belum memiliki tipe layer
/// (`LayerConfig` belum ada di `map_controller/type.rs` maupun `src/map/types`),
/// sehingga [`PlotLayer`] dibuat sementara dan perlu diselaraskan nanti.
/// Cara penyimpanan belum diputuskan.
///
/// Argumen:
/// - `aoi`: `Feature<Polygon>` RFC 7946, WGS84, urutan `[lon, lat]`, seperti
///   `docs/examples/aoi.example.geojson`.
///
/// Output: `Result<PlotLayer, String>`. `Err` berisi pesan bila geometri
/// tidak valid.
///
/// Example Uses:
/// ```json
/// {
///   "type": "Feature",
///   "properties": {
///     "aoi_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///     "name": "Plot A"
///   },
///   "geometry": {
///     "type": "Polygon",
///     "coordinates": [[[116.833, -1.27], [116.834, -1.27], [116.834, -1.271],
///                      [116.833, -1.271], [116.833, -1.27]]]
///   }
/// }
/// ```
///
/// Example Output:
/// ```json
/// {
///   "plot_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///   "plot_name": "Plot A",
///   "geometry": {
///     "type": "Feature",
///     "properties": {
///       "aoi_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///       "name": "Plot A"
///     },
///     "geometry": {
///       "type": "Polygon",
///       "coordinates": [[[116.833, -1.27], [116.834, -1.27], [116.834, -1.271],
///                        [116.833, -1.271], [116.833, -1.27]]]
///     }
///   }
/// }
/// ```
pub fn define_plot(aoi: Feature) -> Result<PlotLayer, String> {
    let _ = aoi;
    todo!("M9.1 belum diimplementasikan")
}

/// Menjalankan plugin terpilih di dalam batas satu plot lewat Modul 2.
///
/// Modul integrasi: Modul 2
///
/// Arah: Modul 9 -> Modul 2 -> Modul 9
///
/// Langkah diagram: 3 dan 4
///
/// Deskripsi: Untuk setiap plugin, Modul 9 menyusun `ExecutionPayload`
/// (`plugin_manager::payload`) dengan geometri plot sebagai AOI, memanggil
/// `plugin_manager::commands::validate_and_create_payload`, lalu setelah
/// plugin selesai membaca hasilnya lewat
/// `plugin_manager::commands::process_execution_result`. Geometri plot
/// dikirim sebagai AOI di execution payload Modul 2 (`aoi`; pada
/// `schemas/execution_payload.schema.json` bentuknya
/// `{ "type": "geojson_polygon", "geometry": ... }`). Modul 4-8 tidak
/// dipanggil langsung; semuanya lewat Modul 2. Kegagalan satu plugin tidak
/// boleh menghentikan aplikasi.
///
/// Argumen:
/// - `plot_id`: ID plot (UUID string) yang sudah didefinisikan.
/// - `plugin_ids`: daftar ID plugin yang dijalankan pada plot ini.
/// - `image_path`: path absolut citra sumber (`target.image_path` pada
///   payload Modul 2). Kontrak Modul 1 belum menyediakan `image_id`.
///
/// Output: `Result<PlotAnalysisResult, String>`; hasil tiap plugin berupa
/// `ExecutionResult` milik Modul 2.
///
/// Example Uses:
/// ```json
/// {
///   "plot_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///   "plugin_ids": ["vegetation_index_v1"],
///   "image_path": "C:/data/DJI_0042.JPG"
/// }
/// ```
///
/// Example Output:
/// ```json
/// {
///   "plot_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///   "results": [
///     {
///       "plugin_id": "vegetation_index_v1",
///       "result": {
///         "status": "success",
///         "metrics": { "primary_metric": 68.45 },
///         "output_files": ["C:/cache/mask_dji_0042.png"],
///         "error_message": null
///       }
///     }
///   ]
/// }
/// ```
pub fn analyze_plot(
    plot_id: String,
    plugin_ids: Vec<String>,
    image_path: String,
) -> Result<PlotAnalysisResult, String> {
    let _ = (plot_id, plugin_ids, image_path);
    todo!("M9.2 belum diimplementasikan")
}

/// Memberikan hasil plugin dan statistik per plot untuk dashboard.
///
/// Modul integrasi: Modul 11
///
/// Arah: Modul 9 -> Modul 11
///
/// Langkah diagram: 6
///
/// Deskripsi: Mengumpulkan semua plot dalam satu proyek beserta luas,
/// keliling, dan indikator hasil plugin. Luas dan keliling dihitung memakai
/// Spatial Measurement milik Modul 3 (fungsinya belum ada di
/// `map_controller`), bukan perhitungan sendiri. Modul 11 membandingkan dan
/// mengurutkan plot sendiri dari hasil ini. Cara penyimpanan belum diputuskan.
///
/// Argumen:
/// - `project_id`: ID proyek yang plotnya diminta.
///
/// Output: `Result<Vec<PlotStatistics>, String>`.
///
/// Example Uses:
/// ```json
/// { "project_id": "proj_001" }
/// ```
///
/// Example Output:
/// ```json
/// [
///   {
///     "plot_id": "3f2b8c1e-5a7d-4e2f-9c1a-0b6d4e8f1a23",
///     "plot_name": "Plot A",
///     "area_m2": 12500.5,
///     "area_ha": 1.25005,
///     "perimeter_m": 500.0,
///     "indicators": [
///       {
///         "name": "vegetation_cover",
///         "unit": "%",
///         "source": "vegetation_index_v1",
///         "value": 68.45,
///         "stats": { "mean": 0.61, "min": 0.12, "max": 0.93, "stddev": 0.08 }
///       }
///     ]
///   }
/// ]
/// ```
pub fn get_plot_statistics(project_id: String) -> Result<Vec<PlotStatistics>, String> {
    let _ = project_id;
    todo!("M9.7 belum diimplementasikan")
}
