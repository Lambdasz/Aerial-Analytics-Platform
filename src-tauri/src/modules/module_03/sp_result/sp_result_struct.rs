//! Berkas ini berisi definisi struct entitas dan DTO hasil analitik spasial (Spatial Results).
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `LayerPayload`: Wadah layer peta analitik lengkap.
//! - `LayerDisplayMode`: Mode rendering dan preferensi styling di atas peta.
//! - `SpatialResult`: Fitur/objek spasial analitik individual.
//! - `SpatialProperties`: Enum polimorfik properti fitur AI (Tree, Vegetation, LandCover, dll).
//! - `SpatialResultSummary`: Struct ringkasan metadata spatial result tanpa memuat geometri GeoJSON penuh.
//! - `UpdateSpatialResultPayload`: Payload pembaruan atribut spatial result.
//! - `DeleteSpatialResultSummary`: Ringkasan konfirmasi penghapusan hasil spasial.
//! - `ManualEditsCheckResult`: Hasil audit keberadaan edit manual pengguna.

use serde::{Deserialize, Serialize};

use crate::modules::module_03::error::MapControllerError;
pub use crate::modules::module_03::types::geometry_type::SpatialGeometry;

/// Nama event Tauri yang digunakan untuk mengirimkan data layer spasial.
pub const SPATIAL_RESULT_EVENT: &str = "map://spatial-result";

/// Perintah utama yang dikirim dari backend ke frontend untuk memanipulasi layer peta.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MapLayerCommand {
    pub action: String,
    pub payload: LayerPayload,
}

/// Returns the default layer opacity (0.6).
fn default_opacity() -> f64 {
    0.6
}

/// Returns the default vector line weight (3.0).
fn default_line_weight() -> f64 {
    3.0
}

/// Returns the default field visibility flag (true).
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

impl LayerPayload {
    /// Memvalidasi seluruh konfigurasi dan data sebelum diteruskan ke frontend.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        if let LayerDisplayMode::ImageOverlay { opacity, .. } = &self.display_preference {
            if *opacity < 0.0 || *opacity > 1.0 {
                return Err(MapControllerError::InvalidGeometry(
                    "Opacity ImageOverlay harus di antara 0.0 hingga 1.0".to_string(),
                ));
            }
        }

        for result in &self.spatial_results {
            result.validate()?;
        }
        Ok(())
    }
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

impl SpatialResult {
    /// Mengeksekusi validasi pada satu objek hasil spasial.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        self.geometry.validate()?;
        Ok(())
    }
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

/// Penampung polimorfik untuk berbagai properti fitur spasial analitik.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SpatialProperties {
    Tree(TreeProperties),
    Vegetation(VegetationProperties),
    LandCover(LandCoverProperties),
    InspectionPath(InspectionPathProperties),
    Generic(serde_json::Map<String, serde_json::Value>),
}

/// Ringkasan ringan spatial result untuk daftar list (tanpa memuat geometri GeoJSON berat).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpatialResultSummary {
    pub id: String,
    pub layer_name: String,
    pub feature_count: usize,
    pub geom_type: String, // "Point" | "Polygon" | "Line" | "Overlay" | "Mixed"
    pub display_color: String,
    pub icon: Option<String>,
    pub thumbnail_url: Option<String>,
    pub format_type: String, // "flat" | "decomposed"
    pub is_corrupted: bool,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub has_manual_edits: bool,
    #[serde(default)]
    pub concurrency_token: String,
}

/// Edit parsial untuk satu fitur spesifik berdasarkan indeks array fitur.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FeatureEdit {
    pub index: usize,
    #[serde(default)]
    pub set: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub remove: Vec<String>,
}

/// Payload input untuk pembaruan aman field presentasi spatial result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateSpatialResultPayload {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub image_action: Option<String>, // "keep" | "delete" | "replace"
    #[serde(default)]
    pub image_path: Option<String>, // Path berkas gambar lokal di disk
    #[serde(default)]
    pub fields: Option<Vec<FieldDef>>,
    #[serde(default)]
    pub feature_edits: Option<Vec<FeatureEdit>>,
    #[serde(default)]
    pub expected_updated_at: Option<String>,
}

/// Ringkasan dependensi dan entitas yang dibersihkan saat penghapusan spatial result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteSpatialResultSummary {
    pub id: String,
    pub deleted_from_disk: bool,
    pub layers_removed: usize,
    pub feature_count: usize,
    #[serde(default)]
    pub photos_count: usize,
}

/// Hasil pemeriksaan keberadaan edit manual pada layer spasial.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualEditsCheckResult {
    pub layer_id: String,
    pub exists: bool,
    pub has_manual_edits: bool,
    pub edited_features_count: usize,
    pub custom_fields_count: usize,
}
