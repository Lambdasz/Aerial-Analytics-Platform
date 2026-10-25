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

fn default_opacity() -> f64 {
    0.6
}

fn default_line_weight() -> f64 {
    3.0
}

fn default_field_visible() -> bool {
    true
}

/// Definisi kolom/field presentasi untuk popup dan tabel hasil spasial.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default = "default_field_visible")]
    pub visible: bool,
    #[serde(default)]
    pub custom: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>, // "text" | "number" | "boolean"
}

/// Mewakili satu kesatuan layer pada peta yang berisi preferensi tampilan
/// dan sekumpulan hasil analitik spasial.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LayerPayload {
    pub layer_id: String,
    pub layer_name: String,
    pub display_preference: LayerDisplayMode,
    pub spatial_results: Vec<SpatialResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency_token: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Menentukan bagaimana frontend harus menggambar (styling) layer tersebut.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum LayerDisplayMode {
    #[serde(rename = "polygon")]
    Polygon {
        color: String,
        #[serde(default = "default_opacity")]
        opacity: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<FieldDef>>,
    },
    #[serde(rename = "point")]
    Point {
        color: String,
        #[serde(default)]
        icon: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<FieldDef>>,
    },
    #[serde(rename = "line")]
    Line {
        color: String,
        #[serde(default = "default_line_weight")]
        weight: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<FieldDef>>,
    },
    #[serde(rename = "image_overlay")]
    ImageOverlay {
        image_url: String,
        opacity: f64,
        bounds: [[f64; 2]; 2],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<FieldDef>>,
    },
}

/// Merepresentasikan satu fitur/hasil analitik tunggal di atas peta.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpatialResult {
    pub source: String,
    pub geometry: SpatialGeometry,
    pub properties: SpatialProperties,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_fields: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Atribut khusus untuk hasil analisis indeks vegetasi (Modul 4).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VegetationProperties {
    pub vegetation_index_type: String,
    pub mean_greenness_score: f64,
    pub area_sqm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Atribut khusus untuk hasil klasifikasi tutupan lahan (Modul 8).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LandCoverProperties {
    pub land_cover_class: String,
    pub confidence: f64,
    pub area_sqm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Atribut khusus untuk jalur inspeksi.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct InspectionPathProperties {
    pub path_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_type: Option<String>,
    pub length_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Atribut khusus untuk data citra orthomosaic GeoTIFF.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OrthoProperties {
    pub ortho_id: String,
    pub crs: String,
    pub resolution_cm: f64,
    pub area_sqm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Penampung polimorfik untuk berbagai properti fitur spasial analitik.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SpatialProperties {
    Tree(TreeProperties),
    Vegetation(VegetationProperties),
    LandCover(LandCoverProperties),
    InspectionPath(InspectionPathProperties),
    Ortho(OrthoProperties),
    Generic(serde_json::Map<String, serde_json::Value>),
}
