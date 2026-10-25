//! Fungsi murni untuk manipulasi state layer peta.

use super::types::{LayerError, LayerState};

/// Mengubah status visibility sebuah layer secara fungsional murni.
pub fn toggle_layer_visibility(
    layer: LayerState,
    is_visible: bool,
) -> Result<LayerState, LayerError> {
    Ok(LayerState {
        is_visible,
        ..layer
    })
}

/// Mengubah opacity sebuah layer secara fungsional murni.
pub fn set_layer_opacity(layer: LayerState, opacity: f64) -> Result<LayerState, LayerError> {
    if !(0.0..=1.0).contains(&opacity) {
        return Err(LayerError::InvalidOpacity);
    }

    Ok(LayerState { opacity, ..layer })
}

/// Mengubah urutan layer pada stack secara fungsional.
pub fn reorder_layer_stack(
    layers: Vec<LayerState>,
    from_index: usize,
    to_index: usize,
) -> Result<Vec<LayerState>, LayerError> {
    if from_index >= layers.len() || to_index >= layers.len() {
        return Err(LayerError::InvalidIndex);
    }

    let mut working = layers;
    let layer = working.remove(from_index);
    working.insert(to_index, layer);

    let reordered = working
        .into_iter()
        .enumerate()
        .map(|(index, l)| LayerState {
            z_index: index,
            ..l
        })
        .collect();

    Ok(reordered)
}

/// Mengambil layer berdasarkan kategori secara murni.
pub fn filter_layers_by_category(layers: &[LayerState], category: &str) -> Vec<LayerState> {
    layers
        .iter()
        .filter(|layer| layer.category == category)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_layer(
        id: &str,
        name: &str,
        category: &str,
        visible: bool,
        opacity: f64,
        z_index: usize,
    ) -> LayerState {
        LayerState {
            layer_id: id.to_string(),
            layer_name: name.to_string(),
            category: category.to_string(),
            is_visible: visible,
            opacity,
            z_index,
            source_type: None,
            source_id: None,
        }
    }

    #[test]
    fn test_toggle_layer_visibility() {
        let layer = create_test_layer("layer_sat_01", "Satellite Layer", "satelite", true, 1.0, 10);
        let result = toggle_layer_visibility(layer, false);
        assert!(result.is_ok());
        let updated_layer = result.unwrap();
        assert_eq!(updated_layer.layer_id, "layer_sat_01");
        assert!(!updated_layer.is_visible);
    }

    #[test]
    fn test_set_layer_opacity() {
        let layer = create_test_layer("layer_ortho_01", "Ortho Layer", "orthomosaic", true, 1.0, 5);
        let result = set_layer_opacity(layer, 0.5);
        assert!(result.is_ok());
        let updated_layer = result.unwrap();
        assert_eq!(updated_layer.opacity, 0.5);
    }

    #[test]
    fn test_set_layer_opacity_invalid() {
        let layer = create_test_layer("layer_ortho_01", "Ortho Layer", "orthomosaic", true, 1.0, 5);
        assert_eq!(
            set_layer_opacity(layer.clone(), -0.1),
            Err(LayerError::InvalidOpacity)
        );
        assert_eq!(
            set_layer_opacity(layer, 1.1),
            Err(LayerError::InvalidOpacity)
        );
    }

    #[test]
    fn test_reorder_layer_stack() {
        let l1 = create_test_layer("l1", "Layer 1", "cat", true, 1.0, 0);
        let l2 = create_test_layer("l2", "Layer 2", "cat", true, 1.0, 1);
        let l3 = create_test_layer("l3", "Layer 3", "cat", true, 1.0, 2);
        let layers = vec![l1, l2, l3];

        let result = reorder_layer_stack(layers, 0, 2).unwrap();
        assert_eq!(result[0].layer_id, "l2");
        assert_eq!(result[1].layer_id, "l3");
        assert_eq!(result[2].layer_id, "l1");
        assert_eq!(result[0].z_index, 0);
        assert_eq!(result[1].z_index, 1);
        assert_eq!(result[2].z_index, 2);
    }

    #[test]
    fn test_reorder_layer_stack_invalid_index() {
        let l1 = create_test_layer("l1", "Layer 1", "cat", true, 1.0, 0);
        let layers = vec![l1];
        assert_eq!(
            reorder_layer_stack(layers, 0, 5),
            Err(LayerError::InvalidIndex)
        );
    }

    #[test]
    fn test_filter_layers_by_category() {
        let l1 = create_test_layer("l1", "Sat", "satellite", true, 1.0, 0);
        let l2 = create_test_layer("l2", "Ortho", "ortho", true, 1.0, 1);
        let l3 = create_test_layer("l3", "Sat 2", "satellite", true, 1.0, 2);
        let layers = vec![l1, l2, l3];

        let filtered = filter_layers_by_category(&layers, "satellite");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].layer_id, "l1");
        assert_eq!(filtered[1].layer_id, "l3");
    }

    #[test]
    fn test_filter_layers_by_category_empty() {
        let l1 = create_test_layer("l1", "Sat", "satellite", true, 1.0, 0);
        let filtered = filter_layers_by_category(&[l1], "unknown");
        assert!(filtered.is_empty());
    }
}
