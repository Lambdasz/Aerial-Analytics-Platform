//! Test karakterisasi untuk Module 3 (Tahap 1).
//! Memastikan seluruh fungsi geometri, layer, dan kontrak data berjalan sesuai spesifikasi.

use aerial_analytics_platform_lib::map_controller::layer::{
    create_annotation, filter_layers_by_category, reorder_layer_stack, set_layer_opacity,
    toggle_layer_visibility, LayerState,
};
use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::{
    calculate_area, calculate_distance, calculate_perimeter, create_aoi, validate_coordinate,
    validate_polygon,
};
use aerial_analytics_platform_lib::map_controller::types::sp_measurement_type::{
    LayerPayload, MapLayerCommand, SpatialGeometry,
};

#[test]
fn test_characterization_geometry_distance() {
    let point_a = [116.855973, -1.224311];
    let point_b = [116.863203, -1.218630];

    let dist = calculate_distance(point_a, point_b);
    assert!(dist > 1000.0 && dist < 1200.0, "Jarak harus ~1050m");

    let dist_same = calculate_distance(point_a, point_a);
    assert_eq!(dist_same, 0.0);
}

#[test]
fn test_characterization_geometry_area_and_perimeter() {
    let polygon = vec![vec![
        [116.8575, -1.2210],
        [116.8582, -1.2208],
        [116.8590, -1.2212],
        [116.8592, -1.2220],
        [116.8586, -1.2228],
        [116.8578, -1.2225],
        [116.8570, -1.2218],
        [116.8575, -1.2210],
    ]];

    assert!(validate_polygon(&polygon).is_ok());

    let area = calculate_area(&polygon);
    assert!(area > 1000.0, "Luas area harus positif dan realistis");

    let perimeter = calculate_perimeter(&polygon);
    assert!(perimeter > 100.0, "Keliling poligon harus positif");

    let aoi = create_aoi(
        "aoi_001".to_string(),
        "Area Uji".to_string(),
        None,
        polygon,
        "2026-10-20T00:00:00Z".to_string(),
        area,
        perimeter,
    );
    assert_eq!(aoi.properties.aoi_id, "aoi_001");
    assert_eq!(aoi.properties.name, "Area Uji");
    assert_eq!(aoi.geometry.r#type, "Polygon");
}

#[test]
fn test_characterization_coordinate_validation() {
    assert!(validate_coordinate(116.85, -1.22).is_ok());
    assert!(validate_coordinate(181.0, 0.0).is_err());
    assert!(validate_coordinate(0.0, 95.0).is_err());
}

#[test]
fn test_characterization_layer_operations() {
    let layer = LayerState {
        layer_id: "ortho_01".to_string(),
        layer_name: "Orthomosaic".to_string(),
        category: "Drone Imagery".to_string(),
        is_visible: true,
        opacity: 1.0,
        z_index: 10,
        source_type: None,
        source_id: None,
    };

    // Toggle
    let toggled = toggle_layer_visibility(layer.clone(), false).unwrap();
    assert!(!toggled.is_visible);

    // Set Opacity
    let faded = set_layer_opacity(layer.clone(), 0.5).unwrap();
    assert_eq!(faded.opacity, 0.5);
    assert!(set_layer_opacity(layer.clone(), 1.5).is_err());

    // Reorder
    let layers = vec![
        layer.clone(),
        LayerState {
            layer_id: "base_01".to_string(),
            layer_name: "Base".to_string(),
            category: "Base Maps".to_string(),
            is_visible: true,
            opacity: 1.0,
            z_index: 0,
            source_type: None,
            source_id: None,
        },
    ];
    let reordered = reorder_layer_stack(layers.clone(), 0, 1).unwrap();
    assert_eq!(reordered[0].layer_id, "base_01");
    assert_eq!(reordered[1].layer_id, "ortho_01");
    assert_eq!(reordered[0].z_index, 0);
    assert_eq!(reordered[1].z_index, 1);

    // Filter
    let filtered = filter_layers_by_category(&layers, "Base Maps");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].layer_id, "base_01");

    // Anotasi
    let anno = create_annotation(
        "anno_1".to_string(),
        "Pohon Sakit".to_string(),
        SpatialGeometry::Point {
            coordinates: vec![116.85, -1.22],
        },
        "2026-10-20T00:00:00Z".to_string(),
    );
    assert_eq!(anno.id, "anno_1");
    assert_eq!(anno.text, "Pohon Sakit");
}

#[test]
fn test_characterization_deserialize_sp_result() {
    let json_str = r###"[
        {
            "action": "load_spatial_result",
            "payload": {
                "layer_id": "result_polygon_001",
                "layer_name": "Vegetasi Poligon",
                "display_preference": {
                    "type": "polygon",
                    "color": "#2ECC71",
                    "opacity": 0.6
                },
                "spatial_results": [
                    {
                        "source": "module_04_vegetation",
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [[[116.85, -1.22], [116.86, -1.22], [116.86, -1.23], [116.85, -1.22]]]
                        },
                        "properties": {
                            "vegetation_index_type": "VARI",
                            "mean_greenness_score": 0.72,
                            "area_sqm": 1250.0
                        }
                    }
                ]
            }
        }
    ]"###;
    let commands: Result<Vec<MapLayerCommand>, _> = serde_json::from_str(json_str);
    assert!(
        commands.is_ok(),
        "MapLayerCommand harus dapat di-deserialize"
    );
    let cmds = commands.unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].payload.layer_id, "result_polygon_001");
}

#[test]
fn test_characterization_module_10_compatibility() {
    // Literal JSON payload bergaya Module 10 (get_change_area_result dengan type: polygon)
    let module_10_json = r##"{
        "layer_id": "temporal_change_01",
        "layer_name": "Area Perubahan Tutupan Lahan",
        "display_preference": {
            "type": "polygon",
            "color": "#FF0000"
        },
        "spatial_results": [
            {
                "source": "module_10_temporal",
                "geometry": {
                    "type": "Point",
                    "coordinates": [116.8595, -1.2215]
                },
                "properties": {
                    "vegetation_index_type": "NDVI_DELTA",
                    "mean_greenness_score": -0.45,
                    "area_sqm": 250.0
                }
            }
        ]
    }"##;

    let parsed: Result<LayerPayload, _> = serde_json::from_str(module_10_json);
    assert!(
        parsed.is_ok(),
        "Payload Module 10 (type: polygon) harus dapat di-parse oleh LayerPayload Module 3"
    );
    let payload = parsed.unwrap();
    assert_eq!(payload.layer_id, "temporal_change_01");
    assert_eq!(payload.spatial_results.len(), 1);
}
