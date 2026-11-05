//! Kontrak data & tipe domain untuk spatial results (Module 3).

use serde::{Deserialize, Serialize};

// Re-export tipe domain LayerPayload, FieldDef, dsb. untuk kompatibilitas plugin eksternal / Module 10
pub use crate::modules::module_03::types::sp_result_type::*;

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
