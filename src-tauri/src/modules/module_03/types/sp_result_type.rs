//! Kontrak data hasil analitik spasial (Modul 4/7/8/10).

use serde::{Deserialize, Serialize};

/// Nama event Tauri yang digunakan untuk mengirimkan data layer spasial.
pub const SPATIAL_RESULT_EVENT: &str = "map://spatial-result";

/// Perintah utama yang dikirim dari backend ke frontend untuk memanipulasi layer peta.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MapLayerCommand {
    pub action: String,
    pub payload: LayerPayload,
}

/// Mewakili satu kesatuan layer pada peta yang berisi preferensi tampilan
/// dan sekumpulan hasil analitik spasial.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LayerPayload {
    pub layer_id: String,
    pub layer_name: String,
    pub display_preference: LayerDisplayMode,
    pub spatial_results: Vec<SpatialResult>,
}

/// Menentukan bagaimana frontend harus menggambar (styling) layer tersebut.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum LayerDisplayMode {
    #[serde(rename = "image_overlay")]
    ImageOverlay {
        image_url: String,
        opacity: f64,
        bounds: [[f64; 2]; 2],
    },
    #[serde(rename = "point")]
    Point { color: String, icon: String },
}

/// Merepresentasikan satu fitur/hasil analitik tunggal di atas peta.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpatialResult {
    pub source: String,
    pub geometry: SpatialGeometry,
    pub properties: SpatialProperties,
}

/// Bentuk geometri dari hasil spasial yang didukung (GeoJSON RFC 7946).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum SpatialGeometry {
    Point { coordinates: Vec<f64> },
    Polygon { coordinates: Vec<Vec<Vec<f64>>> },
    LineString { coordinates: Vec<Vec<f64>> },
}

/// Atribut khusus untuk hasil deteksi pohon individu (Modul 7).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TreeProperties {
    pub tree_id: String,
    pub confidence: f64,
    pub height_est_m: f64,
}

/// Atribut khusus untuk hasil analisis indeks vegetasi (Modul 4).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VegetationProperties {
    pub vegetation_index_type: String,
    pub mean_greenness_score: f64,
    pub area_sqm: f64,
}

/// Atribut khusus untuk hasil klasifikasi tutupan lahan (Modul 8).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LandCoverProperties {
    pub land_cover_class: String,
    pub confidence: f64,
    pub area_sqm: f64,
}

/// Atribut khusus untuk jalur inspeksi.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct InspectionPathProperties {
    pub path_id: String,
    pub r#type: String,
    pub length_m: f64,
}

/// Penampung polimorfik untuk berbagai properti fitur spasial analitik.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SpatialProperties {
    Tree(TreeProperties),
    Vegetation(VegetationProperties),
    LandCover(LandCoverProperties),
    InspectionPath(InspectionPathProperties),
}
