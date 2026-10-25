//! Definisi tipe data & kontrak citra ortomosaik / raster JPG & PNG.

use serde::{Deserialize, Serialize};

/// Kategori citra yang didukung oleh sistem penyimpanan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageCategory {
    /// Citra Orthomosaic Berkoordinat (WGS84)
    #[serde(rename = "ortho")]
    Ortho,
    /// Citra Raster Standar
    #[serde(rename = "raster")]
    Raster,
    /// Backward-compatibility fallback jika ada entitas lama
    #[serde(rename = "tiff")]
    Tiff,
}

/// Parameter input impor citra orthomosaic baru (JPG/PNG + Bounding Box WGS84).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportImagePayload {
    pub file_path: String,
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Metadata citra yang berhasil disimpan di dalam storage atomic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredImageMetadata {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    pub filename: String,
    pub category: ImageCategory,
    pub extension: String,
    pub file_path: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[[f64; 2]; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_corrupted: Option<bool>,
}

/// Parameter pembaruan metadata citra orthomosaic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrthoImagePayload {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub west: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub south: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub north: Option<f64>,
}

/// Ringkasan dampak penghapusan data cascade citra orthomosaic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteOrthoSummary {
    pub id: String,
    pub image_deleted: bool,
    pub layer_deleted: bool,
    pub annotations_deleted_count: usize,
}

fn default_schema_version() -> u32 {
    1
}
