use aerial_analytics_platform_lib::map_controller::annotations::types::AnnotationFeature;
use aerial_analytics_platform_lib::map_controller::aoi::types::{
    AoiFeature, AoiGeometry, AoiProperties,
};
use aerial_analytics_platform_lib::map_controller::state::Module03State;
use aerial_analytics_platform_lib::map_controller::types::sp_result_type::{
    LayerPayload, SpatialGeometry,
};
use tempfile::tempdir;

#[test]
fn test_storage_crud_empty_state_is_valid() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let state = Module03State::new(temp.path().to_path_buf());

    assert!(state.get_saved_annotations().is_empty());
    assert!(state.get_saved_aois().is_empty());
}

#[test]
fn test_annotation_crud_lifecycle_and_write_through() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let state = Module03State::new(temp.path().to_path_buf());

    let anno_id = "550e8400-e29b-41d4-a716-446655440003".to_string();

    let annotation = AnnotationFeature {
        id: anno_id.clone(),
        schema_version: 1,
        text: "Pohon kelapa sawit terindikasi ganoderma".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.8595, -1.2215],
        },
        created_at: "2026-10-23T10:30:00Z".to_string(),
        image_id: None,
    };

    // 1. CREATE (Write-Through)
    let saved = state
        .save_annotation(annotation.clone())
        .expect("Harus berhasil menyimpan anotasi");
    assert_eq!(saved.id, anno_id);

    // Verifikasi disk file
    let disk_file = temp
        .path()
        .join("annotations")
        .join(format!("{}.json", anno_id));
    assert!(
        disk_file.exists(),
        "Berkas JSON anotasi harus tertulis di disk"
    );

    // 2. READ ALL
    let list = state.get_saved_annotations();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, anno_id);
    assert_eq!(list[0].text, "Pohon kelapa sawit terindikasi ganoderma");

    // 3. READ SINGLE BY ID
    let single = state
        .get_annotation_by_id(&anno_id)
        .expect("Harus bisa mengambil anotasi by ID");
    assert_eq!(single.text, "Pohon kelapa sawit terindikasi ganoderma");

    // 4. UPDATE
    let mut updated = single;
    updated.text = "Pohon sawit telah diberi fungisida".to_string();
    let res_update = state
        .update_annotation(updated)
        .expect("Harus berhasil memperbarui anotasi");
    assert_eq!(res_update.text, "Pohon sawit telah diberi fungisida");

    let single_after_update = state.get_annotation_by_id(&anno_id).unwrap();
    assert_eq!(
        single_after_update.text,
        "Pohon sawit telah diberi fungisida"
    );

    // 5. DELETE
    let deleted = state
        .delete_annotation(&anno_id)
        .expect("Harus berhasil menghapus");
    assert!(deleted);
    assert!(
        !disk_file.exists(),
        "Berkas JSON pada disk harus sudah terhapus"
    );
    assert!(state.get_saved_annotations().is_empty());
}

#[test]
fn test_annotation_64kb_limit_enforced() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let state = Module03State::new(temp.path().to_path_buf());

    let massive_text = "A".repeat(70000); // 70 KB text
    let annotation = AnnotationFeature {
        id: "oversized_anno".to_string(),
        schema_version: 1,
        text: massive_text,
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.85, -1.22],
        },
        created_at: "2026-10-23T10:00:00Z".to_string(),
        image_id: None,
    };

    let result = state.save_annotation(annotation);
    assert!(result.is_err(), "Anotasi > 64 KB harus ditolak");
}

#[test]
fn test_corrupt_or_mismatched_json_skipped_gracefully() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let anno_dir = temp.path().join("annotations");
    std::fs::create_dir_all(&anno_dir).unwrap();

    // 1. Tulis file JSON corrupt (bukan JSON valid)
    std::fs::write(anno_dir.join("corrupt.json"), "{ invalid json...").unwrap();

    // 2. Tulis file dengan ID tidak cocok dengan stem filename
    let mismatched = r#"{"id":"actual_id_123","schema_version":1,"text":"test","geometry":{"type":"Point","coordinates":[116.85,-1.22]},"created_at":"2026-10-23T00:00:00Z"}"#;
    std::fs::write(anno_dir.join("different_stem.json"), mismatched).unwrap();

    // 3. Tulis file dengan schema_version tidak didukung
    let invalid_schema = r#"{"id":"schema_99","schema_version":99,"text":"test","geometry":{"type":"Point","coordinates":[116.85,-1.22]},"created_at":"2026-10-23T00:00:00Z"}"#;
    std::fs::write(anno_dir.join("schema_99.json"), invalid_schema).unwrap();

    // Inisialisasi state baru yang membaca dari folder tersebut
    let state = Module03State::new(temp.path().to_path_buf());

    // Ketiga file di atas wajib di-skip tanpa panic/error
    assert!(state.get_saved_annotations().is_empty());
}

#[test]
fn test_aoi_crud_lifecycle() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let state = Module03State::new(temp.path().to_path_buf());

    let aoi_id = "aoi_test_crud_01".to_string();
    let aoi = AoiFeature {
        r#type: "Feature".to_string(),
        properties: AoiProperties {
            aoi_id: aoi_id.clone(),
            name: "Lahan Sawit Blok A".to_string(),
            aoi_type: Some("Pertanian".to_string()),
            calculated_area_sq_m: 10000.0,
            perimeter_m: 400.0,
            created_at: "2026-10-23T11:00:00Z".to_string(),
        },
        geometry: AoiGeometry {
            r#type: "Polygon".to_string(),
            coordinates: vec![vec![
                [116.85, -1.22],
                [116.86, -1.22],
                [116.86, -1.23],
                [116.85, -1.23],
                [116.85, -1.22],
            ]],
        },
    };

    let saved = state.save_aoi(aoi).expect("Harus berhasil menyimpan AOI");
    assert_eq!(saved.properties.aoi_id, aoi_id);

    let list = state.get_saved_aois();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].properties.aoi_id, aoi_id);

    let deleted = state
        .delete_saved_aoi(&aoi_id)
        .expect("Harus berhasil menghapus AOI");
    assert!(deleted);
    assert!(state.get_saved_aois().is_empty());
}

#[test]
fn test_spatial_results_atomic_decomposition_and_size_limits() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());
    let state = Module03State::new(temp.path().to_path_buf());

    let layer_id = "test_sr_decomp_01".to_string();
    let payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": layer_id.clone(),
        "layer_name": "Deteksi Pohon Kelapa Sawit",
        "display_preference": {
            "type": "point",
            "color": "#2ECC71",
            "icon": "tree"
        },
        "spatial_results": [
            {
                "source": "module_04",
                "geometry": { "type": "Point", "coordinates": [116.858, -1.221] },
                "properties": {
                    "tree_id": "T-001",
                    "confidence": 0.98,
                    "height_est_m": 12.5
                }
            },
            {
                "source": "module_04",
                "geometry": { "type": "Point", "coordinates": [116.859, -1.222] },
                "properties": {
                    "tree_id": "T-002",
                    "confidence": 0.95,
                    "height_est_m": 14.0
                }
            }
        ]
    }))
    .unwrap();

    // 1. Simpan Spatial Result
    let saved = state
        .save_spatial_result(payload)
        .expect("Harus berhasil menyimpan hasil spasial");
    assert_eq!(saved.layer_id, layer_id);

    // 2. Verifikasi dekomposisi struktur atomik di disk:
    // spatial_results/{layer_id}/layer.json
    let layer_json_path = temp
        .path()
        .join("spatial_results")
        .join(&layer_id)
        .join("layer.json");
    assert!(layer_json_path.exists(), "layer.json harus dibuat");
    let layer_size = std::fs::metadata(&layer_json_path).unwrap().len();
    assert!(layer_size <= 65536, "layer.json melebihi 64 KB");

    // spatial_results/{layer_id}/features/{feature_id}.json
    let feat1_path = temp
        .path()
        .join("spatial_results")
        .join(&layer_id)
        .join("features")
        .join("T-001.json");
    let feat2_path = temp
        .path()
        .join("spatial_results")
        .join(&layer_id)
        .join("features")
        .join("T-002.json");
    assert!(feat1_path.exists(), "Feature file T-001.json harus ada");
    assert!(feat2_path.exists(), "Feature file T-002.json harus ada");

    assert!(std::fs::metadata(&feat1_path).unwrap().len() <= 65536);
    assert!(std::fs::metadata(&feat2_path).unwrap().len() <= 65536);

    // 3. Verifikasi registrasi layer otomatis di layers/{layer_id}.json
    let auto_layer_path = temp
        .path()
        .join("layers")
        .join(format!("{}.json", layer_id));
    assert!(
        auto_layer_path.exists(),
        "Layer otomatis harus dibuat di layers/{layer_id}.json"
    );

    // 4. Verifikasi read all
    let all = state.get_saved_spatial_results();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].layer_id, layer_id);
    assert_eq!(all[0].spatial_results.len(), 2);

    // 5. Verifikasi delete
    let deleted = state
        .delete_saved_spatial_result(&layer_id)
        .expect("Harus berhasil menghapus");
    assert!(deleted);
    assert!(!temp.path().join("spatial_results").join(&layer_id).exists());
    assert!(state.get_saved_spatial_results().is_empty());
}

#[test]
fn test_json_storage_size_limits_across_all_entities() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let state = Module03State::new(temp.path().to_path_buf());

    // Buat entitas anotasi
    let _ = state.save_annotation(AnnotationFeature {
        id: "anno_limit_test".to_string(),
        schema_version: 1,
        text: "Test limit".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.85, -1.22],
        },
        created_at: "2026-10-23T00:00:00Z".to_string(),
        image_id: None,
    });

    // Buat entitas AOI
    let _ = state.save_aoi(AoiFeature {
        r#type: "Feature".to_string(),
        properties: AoiProperties {
            aoi_id: "aoi_limit_test".to_string(),
            name: "AOI Limit".to_string(),
            aoi_type: None,
            calculated_area_sq_m: 500.0,
            perimeter_m: 100.0,
            created_at: "2026-10-23T00:00:00Z".to_string(),
        },
        geometry: AoiGeometry {
            r#type: "Polygon".to_string(),
            coordinates: vec![vec![
                [116.85, -1.22],
                [116.86, -1.22],
                [116.86, -1.23],
                [116.85, -1.22],
            ]],
        },
    });

    // Pindai semua berkas .json di tempdir dan assert <= 64 KB
    fn scan_json_files(dir: &std::path::Path) {
        if !dir.exists() {
            return;
        }
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                scan_json_files(&path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let size = std::fs::metadata(&path).unwrap().len();
                assert!(
                    size <= 65536,
                    "File {:?} melebihi batas 64 KB: {} bytes",
                    path,
                    size
                );
            }
        }
    }

    scan_json_files(temp.path());
}
