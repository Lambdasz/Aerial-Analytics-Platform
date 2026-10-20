//! Kontrak data status layer peta dan anotasi.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::sp_result_type::SpatialGeometry;

/// State/status sebuah layer pada peta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerState {
    pub layer_id: String,
    pub layer_name: String,
    pub category: String,
    pub is_visible: bool,
    pub opacity: f64,
    pub z_index: usize,
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

/// Representasi anotasi yang dibuat pada peta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnnotationFeature {
    pub annotation_id: String,
    pub text: String,
    pub geometry: SpatialGeometry,
    pub created_at: String,
}
