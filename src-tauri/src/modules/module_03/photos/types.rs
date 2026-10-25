//! Definisi tipe data & metadata foto drone (EXIF).

use serde::{Deserialize, Serialize};

fn default_schema_version() -> u32 {
    1
}

/// Metadata foto drone riil yang diekstrak dari EXIF foto udara asli.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotoMetadata {
    pub id: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub file_name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_agl: f64,
    pub heading_deg: f64,
    pub thumbnail_path: String,
    pub created_at: String,
}

/// Type alias untuk kompatibilitas backward DTO DroneImageMetadata.
pub type DroneImageMetadata = PhotoMetadata;
