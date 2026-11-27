//! Berkas ini berisi definisi struct dan tipe data untuk status layer peta.
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `LayerState`: Struct representasi status satu layer peta (visibilitas, transparansi opacity, z-index, kategori).
//! - `LayerError`: Enum error spesifik manipulasi layer (opacity di luar rentang, index tidak sah).
//! - `GetSavedLayersResponse`: Struct envelope respons IPC yang membungkus daftar seluruh layer dan total count.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// State/status sebuah layer pada peta Leaflet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerState {
    pub layer_id: String,
    pub layer_name: String,
    pub category: String,
    pub is_visible: bool,
    pub opacity: f64,
    pub z_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

/// Error yang dapat terjadi pada operasi layer.
#[derive(Debug, Error, PartialEq)]
pub enum LayerError {
    #[error("layer not found: {0}")]
    LayerNotFound(String),

    #[error("opacity must be between 0.0 and 1.0")]
    InvalidOpacity,

    #[error("layer index is out of range")]
    InvalidIndex,
}
