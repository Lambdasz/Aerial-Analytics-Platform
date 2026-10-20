pub use super::types::layer_type::*;
use super::types::sp_result_type::SpatialGeometry;

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

/// Mengambil layer berdasarkan kategori.
pub fn filter_layers_by_category(layers: &[LayerState], category: &str) -> Vec<LayerState> {
    layers
        .iter()
        .filter(|layer| layer.category == category)
        .cloned()
        .collect()
}

/// Membuat annotation feature baru.
///
/// Fungsi ini membentuk data anotasi dari parameter yang diberikan.
pub fn create_annotation(
    annotation_id: String,
    text: String,
    geometry: SpatialGeometry,
    created_at: String,
) -> AnnotationFeature {
    AnnotationFeature {
        annotation_id,
        text,
        geometry,
        created_at,
    }
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
        }
    }

    // =========================================================
    // TEST 1: toggle_layer_visibility
    // =========================================================

    #[test]
    fn test_toggle_layer_visibility() {
        let layer = create_test_layer("layer_sat_01", "Satellite Layer", "satelite", true, 1.0, 10);

        let result = toggle_layer_visibility(layer, false);

        assert!(result.is_ok());

        let updated_layer = result.unwrap();

        assert_eq!(updated_layer.layer_id, "layer_sat_01");
        assert!(!updated_layer.is_visible);
    }

    // =========================================================
    // TEST 2: set_layer_opacity
    // =========================================================

    #[test]
    fn test_set_layer_opacity() {
        let layer = create_test_layer("layer_heat_02", "Heatmap Layer", "analysis", true, 1.0, 20);

        let result = set_layer_opacity(layer, 0.75);

        assert!(result.is_ok());

        let updated_layer = result.unwrap();

        assert_eq!(updated_layer.layer_id, "layer_heat_02");
        assert_eq!(updated_layer.opacity, 0.75);
    }

    #[test]
    fn test_set_layer_opacity_invalid() {
        let layer = create_test_layer("layer_heat_02", "Heatmap Layer", "analysis", true, 1.0, 20);

        let result = set_layer_opacity(layer, 1.5);

        assert_eq!(result, Err(LayerError::InvalidOpacity));
    }

    // =========================================================
    // TEST 3: reorder_layer_stack
    // =========================================================

    #[test]
    fn test_reorder_layer_stack() {
        let layers = vec![
            create_test_layer("L1", "Layer 1", "base", true, 1.0, 0),
            create_test_layer("L2", "Layer 2", "drone", true, 1.0, 1),
            create_test_layer("L3", "Layer 3", "analysis", true, 1.0, 2),
        ];

        let result = reorder_layer_stack(layers, 0, 2);

        assert!(result.is_ok());

        let reordered = result.unwrap();

        assert_eq!(reordered[0].layer_id, "L2");
        assert_eq!(reordered[1].layer_id, "L3");
        assert_eq!(reordered[2].layer_id, "L1");

        assert_eq!(reordered[0].z_index, 0);
        assert_eq!(reordered[1].z_index, 1);
        assert_eq!(reordered[2].z_index, 2);
    }

    #[test]
    fn test_reorder_layer_stack_invalid_index() {
        let layers = vec![
            create_test_layer("L1", "Layer 1", "base", true, 1.0, 0),
            create_test_layer("L2", "Layer 2", "drone", true, 1.0, 1),
        ];

        let result = reorder_layer_stack(layers, 0, 5);

        assert_eq!(result, Err(LayerError::InvalidIndex));
    }

    // =========================================================
    // TEST 4: filter_layers_by_category
    // =========================================================

    #[test]
    fn test_filter_layers_by_category() {
        let layers = vec![
            create_test_layer("L1", "Satellite Layer", "satelite", true, 1.0, 0),
            create_test_layer("L2", "Drone Layer", "drone", true, 1.0, 1),
            create_test_layer("L3", "Satellite Analysis", "satelite", true, 0.5, 2),
        ];

        let result = filter_layers_by_category(&layers, "satelite");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].layer_id, "L1");
        assert_eq!(result[1].layer_id, "L3");
    }

    #[test]
    fn test_filter_layers_by_category_empty() {
        let layers = vec![
            create_test_layer("L1", "Satellite Layer", "satelite", true, 1.0, 0),
            create_test_layer("L2", "Drone Layer", "drone", true, 1.0, 1),
        ];

        let result = filter_layers_by_category(&layers, "analysis");

        assert!(result.is_empty());
    }

    // =========================================================
    // TEST 5: create_annotation
    // =========================================================

    #[test]
    fn test_create_annotation() {
        let geometry = SpatialGeometry::Point {
            coordinates: vec![116.833, -1.270],
        };

        let annotation = create_annotation(
            "anno_99".to_string(),
            "Pohon terindikasi penyakit menguning".to_string(),
            geometry.clone(),
            "2026-10-06T10:00:00Z".to_string(),
        );

        assert_eq!(annotation.annotation_id, "anno_99");
        assert_eq!(annotation.text, "Pohon terindikasi penyakit menguning");
        assert_eq!(annotation.geometry, geometry);
        assert_eq!(annotation.created_at, "2026-10-06T10:00:00Z");
    }
}
