use aerial_analytics_platform_lib::map_controller::storage::{self, ImageCategory};
use aerial_analytics_platform_lib::map_controller::types::aoi_type::{
    AoiFeature, AoiGeometry, AoiProperties,
};
use aerial_analytics_platform_lib::map_controller::types::sp_result_type::LayerPayload;

#[test]
fn test_aoi_storage_lifecycle_and_type_alias() {
    let test_id = format!(
        "test_aoi_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let aoi = AoiFeature {
        r#type: "Feature".to_string(),
        properties: AoiProperties {
            aoi_id: test_id.clone(),
            name: "Area Uji Storage".to_string(),
            aoi_type: Some("Area Pertanian".to_string()),
            calculated_area_sq_m: 5432.1,
            perimeter_m: 321.0,
            created_at: "2026-10-22T10:00:00Z".to_string(),
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

    // 1. Simpan ke storage
    let saved = storage::save_aoi(aoi.clone()).expect("Harus berhasil menyimpan AOI");
    assert_eq!(saved.properties.aoi_id, test_id);
    assert_eq!(
        saved.properties.aoi_type,
        Some("Area Pertanian".to_string())
    );

    // 2. Ambil seluruh data tersimpan
    let list = storage::get_saved_aois().expect("Harus berhasil membaca list AOI");
    assert!(list.iter().any(|item| item.properties.aoi_id == test_id));

    // 3. Uji backward/alias compatibility: tpe & tipe
    let json_tpe = format!(
        r#"{{
            "type": "Feature",
            "properties": {{
                "aoi_id": "{}_tpe",
                "name": "Alias Test",
                "tpe": "Perkebunan Sawit",
                "calculated_area_sq_m": 1000.0,
                "perimeter_m": 100.0,
                "created_at": "2026-10-22T10:00:00Z"
            }},
            "geometry": {{
                "type": "Polygon",
                "coordinates": [[[116.85, -1.22], [116.86, -1.22], [116.86, -1.23], [116.85, -1.23], [116.85, -1.22]]]
            }}
        }}"#,
        test_id
    );
    let parsed: AoiFeature = serde_json::from_str(&json_tpe).expect("Harus bisa parse alias 'tpe'");
    assert_eq!(
        parsed.properties.aoi_type,
        Some("Perkebunan Sawit".to_string())
    );

    // 4. Hapus data uji
    let deleted = storage::delete_saved_aoi(&test_id).expect("Harus berhasil menghapus");
    assert!(deleted);
}

#[test]
fn test_spatial_result_storage_lifecycle() {
    let temp = tempfile::tempdir().expect("Gagal membuat tempdir");
    let test_layer_id = "test_layer_isolated_01".to_string();

    let payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": test_layer_id.clone(),
        "layer_name": "Hasil Uji Spasial",
        "display_preference": {
            "type": "point",
            "color": "#27AE60",
            "icon": "tree"
        },
        "spatial_results": [{
            "source": "test_detector",
            "geometry": {
                "type": "Point",
                "coordinates": [116.8595, -1.2215]
            },
            "properties": {
                "tree_id": "T-TEST-01",
                "confidence": 0.99,
                "height_est_m": 15.0
            },
            "image_url": "/sample_drone.jpg"
        }]
    }))
    .unwrap();

    // 1. Simpan ke storage internal via tempdir
    let saved = aerial_analytics_platform_lib::modules::module_03::sp_result::store::save_spatial_result_in_dir(
        temp.path(),
        payload,
    )
    .expect("Harus berhasil simpan spatial result");
    assert_eq!(saved.layer_id, test_layer_id);

    // 2. Ambil list dari storage internal via tempdir
    let list = aerial_analytics_platform_lib::modules::module_03::sp_result::store::get_saved_spatial_results_in_dir(
        temp.path(),
    )
    .expect("Harus berhasil list spatial results");
    assert!(list.iter().any(|item| item.layer_id == test_layer_id));

    // 3. Hapus data uji
    let deleted = aerial_analytics_platform_lib::modules::module_03::sp_result::store::delete_saved_spatial_result_in_dir(
        temp.path(),
        &test_layer_id,
    )
    .expect("Harus berhasil delete");
    assert!(deleted);
}

#[test]
fn test_layer_payload_updated_at_backward_compatibility() {
    // 1. Data JSON lama TANPA field updated_at (mis. dari Module 10 atau file versi awal)
    let json_without_updated_at = r##"{
        "layer_id": "old_layer_001",
        "layer_name": "Layer Lama",
        "display_preference": {
            "type": "point",
            "color": "#FF0000",
            "icon": "marker"
        },
        "spatial_results": []
    }"##;

    let parsed_old: LayerPayload = serde_json::from_str(json_without_updated_at)
        .expect("Harus berhasil deserialisasi data lama");
    assert_eq!(parsed_old.layer_id, "old_layer_001");
    assert_eq!(parsed_old.updated_at, None);

    // 2. Data JSON baru DENGAN field updated_at
    let json_with_updated_at = r##"{
        "layer_id": "new_layer_002",
        "layer_name": "Layer Baru",
        "display_preference": {
            "type": "point",
            "color": "#00FF00",
            "icon": "tree"
        },
        "spatial_results": [],
        "updated_at": "2026-10-24T12:00:00Z"
    }"##;

    let parsed_new: LayerPayload =
        serde_json::from_str(json_with_updated_at).expect("Harus berhasil deserialisasi data baru");
    assert_eq!(parsed_new.layer_id, "new_layer_002");
    assert_eq!(
        parsed_new.updated_at,
        Some("2026-10-24T12:00:00Z".to_string())
    );

    // 3. Serialisasi LayerPayload tanpa updated_at tidak memunculkan field "updated_at" di JSON
    let serialized_none = serde_json::to_string(&parsed_old).unwrap();
    assert!(!serialized_none.contains("updated_at"));

    // 4. Pembaca lama (struct tanpa updated_at, seperti Module 10) tetap bisa membaca JSON ber-updated_at
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct LegacyReader {
        layer_id: String,
        layer_name: String,
    }

    let legacy_read: Result<LegacyReader, _> = serde_json::from_str(json_with_updated_at);
    assert!(
        legacy_read.is_ok(),
        "Pembaca lama harus tetap bisa membaca JSON ber-updated_at"
    );
}

#[test]
fn test_image_storage_rejection_and_png() {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let tiff_filename = format!("test_image_{}.tif", timestamp);
    let raster_filename = format!("test_image_{}.png", timestamp);

    // 1. Berkas TIFF harus ditolak
    let tiff_data = vec![0x49, 0x49, 0x2A, 0x00, 0, 0, 0, 0];
    let res_tiff = storage::save_image(&tiff_filename, &tiff_data);
    assert!(res_tiff.is_err(), "TIFF harus ditolak oleh Module 3");

    // 2. Berkas PNG sah harus diterima
    let png_img = image::RgbImage::new(1, 1);
    let mut png_bytes = Vec::new();
    png_img
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();

    let meta_raster = storage::save_image(&raster_filename, &png_bytes)
        .expect("Harus berhasil menyimpan citra Raster PNG");
    assert_eq!(meta_raster.category, ImageCategory::Raster);
    assert_eq!(meta_raster.extension, "png");

    // 3. Ambil daftar citra yang tersimpan
    let images = storage::get_saved_images().expect("Harus berhasil membaca list citra");
    assert!(images.iter().any(|img| img.id == meta_raster.id));

    // 4. Hapus data uji
    let del_raster = storage::delete_saved_image("raster", &meta_raster.id).unwrap();
    assert!(del_raster);
}
