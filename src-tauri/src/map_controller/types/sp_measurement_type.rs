//! # Spatial Data Contracts
//!
//! Module ini mendefinisikan struktur data (kontrak) yang digunakan untuk komunikasi
//! antara backend (analitik spasial) dan frontend (visualisasi peta) melalui Tauri.
//! Struktur data di sini dirancang agar kompatibel dengan format JSON yang diharapkan
//! oleh frontend dan mengikuti standar koordinat GeoJSON.
//!
//! **⚠️ STATUS KONTRAK (DRAFT 80%)**
//! Kontrak data di dalam module ini masih bersifat **80% final**. Saat ini masih
//! terdapat beberapa penyesuaian dan kendala pada modul analitik lainnya (seperti
//! Modul 4/7/8) terkait definisi pasti dari output yang akan dikirimkan ke fitur ini.
//! Struktur `SpatialProperties` atau tipe properti lainnya kemungkinan akan mengalami
//! perubahan menyesuaikan kesepakatan akhir dari tim analitik.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Nama event Tauri yang digunakan untuk mengirimkan data layer spasial.
/// Frontend harus melakukan `listen` pada event ini untuk menangkap dan merender layer.
pub const SPATIAL_RESULT_EVENT: &str = "map://spatial-result";

/// Perintah utama yang dikirim dari backend ke frontend untuk memanipulasi layer peta.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MapLayerCommand {
    /// Jenis instruksi yang harus dijalankan frontend.
    /// Nilai standar saat ini adalah `"load_spatial_result"`.
    pub action: String,

    /// Data payload yang berisi konfigurasi layer dan kumpulan fitur spasial.
    pub payload: LayerPayload,
}

/// Mewakili satu kesatuan layer pada peta yang berisi preferensi tampilan
/// dan sekumpulan hasil analitik spasial (titik atau poligon).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LayerPayload {
    /// ID unik untuk layer. Jika ID ini sudah ada di frontend,
    /// layer yang lama akan ditimpa (di-replace).
    pub layer_id: String,

    /// Nama layer yang ramah pengguna untuk ditampilkan di antarmuka (mis. Layer Control).
    pub layer_name: String,

    /// Preferensi visual untuk merender data pada layer ini.
    pub display_preference: LayerDisplayMode,

    /// Kumpulan fitur spasial (geometri dan properti) yang akan digambar di atas layer.
    pub spatial_results: Vec<SpatialResult>,
}

/// Menentukan bagaimana frontend harus menggambar (styling) layer tersebut.
///
/// Di-serialize ke JSON dengan field diskriminator `"type"` (mis. `{"type": "point", ...}`).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum LayerDisplayMode {
    /// Merender layer sebagai hamparan gambar (image overlay) di atas peta.
    #[serde(rename = "image_overlay")]
    ImageOverlay {
        /// URL atau path lokal menuju file gambar.
        image_url: String,

        /// Tingkat transparansi gambar (0.0 sangat transparan, 1.0 sangat pekat).
        opacity: f64,
    },

    /// Merender layer sebagai titik (marker) di koordinat tertentu.
    #[serde(rename = "point")]
    Point {
        /// Warna marker dalam format Hex (misal: `"#FF0000"`).
        color: String,

        /// Nama ikon atau pengenal aset ikon yang digunakan oleh frontend.
        icon: String,
    },
}

/// Merepresentasikan satu fitur/hasil analitik tunggal di atas peta.
/// Strukturnya mirip dengan definisi `Feature` pada standar GeoJSON.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpatialResult {
    /// Pengenal atau nama sistem analitik yang menghasilkan data ini
    /// (misal: `"tree_detection_v1"`).
    pub source: String,

    /// Bentuk dan posisi koordinat dari hasil analitik.
    pub geometry: SpatialGeometry,

    /// Data atau atribut tambahan yang terikat pada geometri ini.
    pub properties: SpatialProperties,
}

/// Bentuk geometri dari hasil spasial yang didukung, mengikuti struktur GeoJSON dasar.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SpatialGeometry {
    /// Geometri berupa titik tunggal.
    /// Koordinat harus selalu berurutan: `[longitude, latitude]`.
    Point { coordinates: Vec<f64> },

    /// Geometri berupa area (Poligon).
    /// Terdiri dari kumpulan *linear ring*. Tiap *ring* adalah array dari koordinat `[longitude, latitude]`.
    /// Titik awal dan akhir dalam satu *ring* harus sama agar membentuk area tertutup.
    Polygon { coordinates: Vec<Vec<Vec<f64>>> },
}

/// Atribut khusus untuk hasil deteksi pohon individu.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TreeProperties {
    /// ID unik dari pohon yang dideteksi.
    pub tree_id: String,

    /// Tingkat keyakinan model analitik (0.0 - 1.0).
    pub confidence: f64,

    /// Estimasi tinggi pohon dalam satuan meter.
    pub height_est_m: f64,
}

/// Atribut khusus untuk hasil analisis indeks vegetasi (area).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VegetationProperties {
    /// Jenis indeks vegetasi yang digunakan (misal: `"NDVI"`, `"EVI"`).
    pub vegetation_index_type: String,

    /// Skor rata-rata tingkat kehijauan pada area poligon tersebut.
    pub mean_greenness_score: f64,

    /// Luas area vegetasi yang terdeteksi dalam satuan meter persegi.
    pub area_sqm: f64,
}

/// Atribut khusus untuk hasil deteksi bangunan atau infrastruktur.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildingProperties {
    /// ID unik dari bangunan yang dideteksi.
    pub building_id: String,
}

/// Penampung fleksibel untuk segala jenis properti fitur spasial.
///
/// Menggunakan atribut `#[serde(untagged)]`, sehingga saat de-serialisasi/serialisasi JSON,
/// tipe data akan dicocokkan otomatis berdasarkan field yang tersedia (tanpa field penanda eksplisit).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum SpatialProperties {
    /// Atribut deteksi pohon.
    Tree(TreeProperties),

    /// Atribut analisis vegetasi.
    Vegetation(VegetationProperties),

    /// Atribut deteksi bangunan.
    Building(BuildingProperties),
}
