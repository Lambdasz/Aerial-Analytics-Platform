use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::sp_measurement_type::SpatialGeometry;

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

/// Mengubah status visibility sebuah layer.
///
/// Fungsi ini tidak mengubah layer secara langsung.
/// Fungsi menerima LayerState dan mengembalikan state baru.
pub fn toggle_layer_visibility(
    mut layer: LayerState,
    is_visible: bool,
) -> Result<LayerState, LayerError> {
    layer.is_visible = is_visible;
    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_layer_visibility() {
        let layer = LayerState {
            layer_id: "layer_sat_01".to_string(),
            layer_name: "Satellite Layer".to_string(),
            category: "satelite".to_string(),
            is_visible: true,
            opacity: 1.0,
            z_index: 10,
        };

        let result = toggle_layer_visibility(layer, false);

        assert!(result.is_ok());

        let updated_layer = result.unwrap();

        assert_eq!(updated_layer.layer_id, "layer_sat_01");
        assert!(!updated_layer.is_visible);
    }
}