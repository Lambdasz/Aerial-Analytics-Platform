//! Tauri IPC commands for Layer Management & Annotation (Module 3 — Person 3).
//!
//! Each command is thin: receive input -> delegate to `layer` -> return result.
//! Business logic remains in `map_controller::layer`.

use crate::map_controller::layer::{self, AnnotationFeature, LayerState};
use crate::map_controller::types::sp_measurement_type::SpatialGeometry;
use crate::plugin_manager::error::CommandError;

impl From<layer::LayerError> for CommandError {
    fn from(err: layer::LayerError) -> Self {
        let (code, message) = match err {
            layer::LayerError::LayerNotFound(layer_id) => (
                "LAYER_NOT_FOUND",
                format!("layer not found: {}", layer_id),
            ),
            layer::LayerError::InvalidOpacity => (
                "INVALID_OPACITY",
                "opacity must be between 0.0 and 1.0".to_string(),
            ),
            layer::LayerError::InvalidIndex => (
                "INVALID_LAYER_INDEX",
                "layer index is out of range".to_string(),
            ),
        };

        CommandError {
            code: code.to_string(),
            message,
        }
    }
}

/// Mengubah visibility sebuah layer.
#[tauri::command]
pub fn toggle_layer_visibility_cmd(
    layer: LayerState,
    is_visible: bool,
) -> Result<LayerState, CommandError> {
    Ok(layer::toggle_layer_visibility(layer, is_visible)?)
}

/// Mengubah opacity sebuah layer.
#[tauri::command]
pub fn set_layer_opacity_cmd(
    layer: LayerState,
    opacity: f64,
) -> Result<LayerState, CommandError> {
    Ok(layer::set_layer_opacity(layer, opacity)?)
}

/// Mengubah urutan layer pada stack.
#[tauri::command]
pub fn reorder_layer_stack_cmd(
    layers: Vec<LayerState>,
    from_index: usize,
    to_index: usize,
) -> Result<Vec<LayerState>, CommandError> {
    Ok(layer::reorder_layer_stack(layers, from_index, to_index)?)
}

/// Mengambil layer berdasarkan kategori.
#[tauri::command]
pub fn filter_layers_by_category_cmd(
    layers: Vec<LayerState>,
    category: String,
) -> Vec<LayerState> {
    layer::filter_layers_by_category(&layers, &category)
}

/// Membuat annotation baru pada peta.
#[tauri::command]
pub fn create_annotation_cmd(
    annotation_id: String,
    text: String,
    geometry: SpatialGeometry,
    created_at: String,
) -> AnnotationFeature {
    layer::create_annotation(annotation_id, text, geometry, created_at)
}