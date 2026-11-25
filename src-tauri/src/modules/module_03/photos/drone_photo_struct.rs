//! Berkas ini berisi definisi struct metadata foto drone hasil ekstraksi EXIF.
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `PhotoMetadata`: Struct metadata foto drone (ID, nama file, koordinat GPS, altitude AGL, heading derajat, thumbnail path).
//! - `DroneImageMetadata`: Type alias untuk kompatibilitas DTO foto drone ke frontend.

use serde::{Deserialize, Serialize};

/// Returns the default schema version (1) for drone photo records.
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
