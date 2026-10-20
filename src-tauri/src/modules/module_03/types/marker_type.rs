//! Kontrak data metadata marker foto udara (drone image).

use serde::{Deserialize, Serialize};

/// Metadata foto drone untuk titik pengambilan sampel gambar udara pada peta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DroneImageMetadata {
    pub image_id: String,
    pub file_name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_agl: f64,
    pub heading_deg: f64,
    pub thumbnail_url: String,
}
