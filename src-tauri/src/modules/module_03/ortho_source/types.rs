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

/// Parameter koordinat bounding box manual untuk impor citra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportBoundsPayload {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Payload impor citra dual-mode (Opsi A: Citra + Metadata, Opsi B: TIFF -> Konversi).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportDualModePayload {
    /// Mode impor: "A" (Citra + Metadata) atau "B" (TIFF -> Konversi)
    pub mode: String,
    /// Path berkas sumber di disk lokal
    pub file_path: String,
    /// Simpan berkas asli ke folder images/{id}/ (default true untuk Opsi A, false untuk Opsi B)
    #[serde(default)]
    pub save_original: bool,
    /// Koordinat bounding box manual jika tidak ada world file / GeoKeys
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_bounds: Option<ImportBoundsPayload>,
    /// Nama kustom opsional dari pengguna
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_filename: Option<String>,
}

/// Status dan catatan pekerjaan impor yang disimpan di map_storage/.jobs/{job_id}.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobRecord {
    pub job_id: String,
    /// "running", "completed", "failed", "cancelled", "interrupted"
    pub status: String,
    /// Tahap aktif: "validasi", "salin_asli", "dekompresi", "resampling", "loop_adaptif", "registrasi", "selesai"
    pub stage: String,
    pub progress_percent: f64,
    pub message: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_image_id: Option<String>,
}

/// Event progres impor yang dikirim via IPC `ortho_import_progress`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportJobProgressPayload {
    pub job_id: String,
    pub stage: String,
    pub progress_percent: f64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_processed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
}

/// Ringkasan metadata EXIF kamera drone (hanya titik posisi kamera, bukan bounding box).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExifSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_make: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gps_latitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gps_longitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gps_altitude_agl_m: Option<f64>,
}

/// Detail teknis file GeoTIFF yang diproses pada Opsi B.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TiffTechnicalDetails {
    pub bands: u32,
    pub bit_depth: u32,
    pub sample_format: String,
    pub compression: String,
    pub layout: String,
    pub chunk_dimensions: [u32; 2],
    pub total_chunks: u64,
    pub planar_configuration: String,
    pub photometric_interpretation: String,
    pub has_alpha: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodata_value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,
}

/// Informasi sumber batas koordinat untuk metadata.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoundsMetadata {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
    /// "world_file", "sidecar_json", "geotiff", "manual"
    pub source: String,
}

/// Berkas metadata teknis detail `metadata.json` pada folder `map_storage/images/{image_id}/`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TechnicalMetadata {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub image_id: String,
    /// "A" atau "B"
    pub import_mode: String,
    pub source_format: String,
    pub source_filename: String,
    pub source_dimensions: [u32; 2],
    pub display_dimensions: [u32; 2],
    pub source_file_size_bytes: u64,
    pub display_file_size_bytes: u64,
    pub display_format: String,
    pub bounds: BoundsMetadata,
    pub source_crs: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exif_summary: Option<ExifSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiff_technical_details: Option<TiffTechnicalDetails>,
    pub imported_at: String,
    #[serde(default)]
    pub warnings: Vec<String>,
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
    pub display_dimensions: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
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
