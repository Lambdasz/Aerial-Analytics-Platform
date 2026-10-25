//! Test suite untuk operasi baca (T2) Spatial Results (Dual-Reader: Flat & Decomposed).
//!
//! Mencakup:
//! 1. Dual-reader koeksistensi flat file dan folder dekomposisi
//! 2. Penentuan ID murni dari nama file/folder (abaikan mismatched internal layer_id)
//! 3. Resolusi bentrok ID (folder mendahului flat file)
//! 4. Penanganan graceful untuk berkas/folder korup (is_corrupted = true)
//! 5. Validasi ID allowlist dan penolakan path traversal
//! 6. Hierarki resolusi thumbnail foto (Opsi 1)
//! 7. Pembacaan detail tunggal (get_saved_spatial_result) dan penanganan NotFound

use aerial_analytics_platform_lib::map_controller::error::MapControllerError;
use aerial_analytics_platform_lib::map_controller::sp_result::pure::{
    is_owned_result_photo, validate_custom_field_key, validate_custom_fields_count,
    validate_field_label, validate_field_unit, validate_hex_color, validate_property_value_type,
    validate_spatial_result_id,
};
use aerial_analytics_platform_lib::map_controller::sp_result::store::{
    check_spatial_result_manual_edits_in_dir, cleanup_orphaned_trash,
    count_spatial_result_dependents_in_dir, delete_saved_spatial_result_in_dir,
    extract_thumbnail_url, get_saved_spatial_result_in_dir, get_saved_spatial_results_in_dir,
    get_saved_spatial_results_summary_in_dir, update_spatial_result_in_dir, StoredFeatureFile,
    StoredLayerFile,
};
use aerial_analytics_platform_lib::map_controller::sp_result::types::{
    DeleteSpatialResultSummary, FeatureEdit, FieldDef, UpdateSpatialResultPayload,
};
use aerial_analytics_platform_lib::map_controller::types::sp_result_type::LayerPayload;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dual_reader_flat_and_decomposed() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // 1. Siapkan 1 file flat: result_trees_01.json
    let flat_payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "result_trees_01",
        "layer_name": "Deteksi Pohon Kelapa Sawit",
        "display_preference": {
            "type": "point",
            "color": "#2ECC71",
            "icon": "tree"
        },
        "spatial_results": [{
            "source": "plugin_yolo",
            "geometry": {
                "type": "Point",
                "coordinates": [116.85, -1.22]
            },
            "properties": {
                "tree_id": "T-001",
                "confidence": 0.94,
                "height_est_m": 12.5
            }
        }]
    }))
    .unwrap();
    fs::write(
        sr_dir.join("result_trees_01.json"),
        serde_json::to_string_pretty(&flat_payload).unwrap(),
    )
    .unwrap();

    // 2. Siapkan 1 folder dekomposisi: result_landcover_01
    let decomp_dir = sr_dir.join("result_landcover_01");
    let features_dir = decomp_dir.join("features");
    fs::create_dir_all(&features_dir).unwrap();

    let layer_meta: StoredLayerFile = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "id": "result_landcover_01",
        "layer_name": "Klasifikasi Tutupan Lahan",
        "display_preference": {
            "type": "polygon",
            "color": "#E67E22",
            "opacity": 0.65
        },
        "feature_count": 1,
        "updated_at": "2026-10-24T10:00:00Z"
    }))
    .unwrap();
    fs::write(
        decomp_dir.join("layer.json"),
        serde_json::to_string_pretty(&layer_meta).unwrap(),
    )
    .unwrap();

    let feature_meta: StoredFeatureFile = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "id": "feat_0001",
        "source": "plugin_landcover",
        "geometry": {
            "type": "Polygon",
            "coordinates": [[
                [116.85, -1.22],
                [116.86, -1.22],
                [116.86, -1.23],
                [116.85, -1.22]
            ]]
        },
        "properties": {
            "land_cover_class": "Vegetasi Rapat",
            "confidence": 0.88,
            "area_sqm": 15400.0
        }
    }))
    .unwrap();
    fs::write(
        features_dir.join("feat_0001.json"),
        serde_json::to_string_pretty(&feature_meta).unwrap(),
    )
    .unwrap();

    // 3. Baca via get_saved_spatial_results_in_dir (full payloads)
    let full_results =
        get_saved_spatial_results_in_dir(temp.path()).expect("Harus berhasil memuat kedua layer");
    assert_eq!(full_results.len(), 2, "Harus memuat tepat 2 layer");

    let ids: Vec<&str> = full_results.iter().map(|l| l.layer_id.as_str()).collect();
    assert!(ids.contains(&"result_trees_01"));
    assert!(ids.contains(&"result_landcover_01"));

    // 4. Baca via get_saved_spatial_results_summary_in_dir (ringkasan ringan)
    let summaries = get_saved_spatial_results_summary_in_dir(temp.path())
        .expect("Harus berhasil memuat ringkasan");
    assert_eq!(summaries.len(), 2, "Harus memuat tepat 2 ringkasan");

    let tree_summary = summaries
        .iter()
        .find(|s| s.id == "result_trees_01")
        .expect("Ringkasan result_trees_01 harus ditemukan");
    assert_eq!(tree_summary.format_type, "flat");
    assert_eq!(tree_summary.geom_type, "Point");
    assert_eq!(tree_summary.feature_count, 1);
    assert!(!tree_summary.is_corrupted);

    let lc_summary = summaries
        .iter()
        .find(|s| s.id == "result_landcover_01")
        .expect("Ringkasan result_landcover_01 harus ditemukan");
    assert_eq!(lc_summary.format_type, "decomposed");
    assert_eq!(lc_summary.geom_type, "Polygon");
    assert_eq!(lc_summary.feature_count, 1);
    assert_eq!(
        lc_summary.updated_at.as_deref(),
        Some("2026-10-24T10:00:00Z")
    );
    assert!(!lc_summary.is_corrupted);
}

#[test]
fn test_id_determination_and_mismatched_layer_id() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // Buat flat file result_survey_alpha.json yang di dalamnya memiliki layer_id berbeda
    let mismatched_json = r##"{
        "layer_id": "totally_different_internal_id",
        "layer_name": "Survei Area Alpha",
        "display_preference": {
            "type": "polygon",
            "color": "#9B59B6",
            "opacity": 0.5
        },
        "spatial_results": []
    }"##;

    fs::write(sr_dir.join("result_survey_alpha.json"), mismatched_json).unwrap();

    // 1. Baca detail tunggal
    let detail = get_saved_spatial_result_in_dir(temp.path(), "result_survey_alpha")
        .expect("Harus berhasil membaca berkas");
    assert_eq!(
        detail.layer_id, "result_survey_alpha",
        "ID resmi HARUS diselaraskan dengan nama file tanpa .json, mengabaikan data internal"
    );

    // 2. Baca ringkasan
    let summaries = get_saved_spatial_results_summary_in_dir(temp.path()).unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "result_survey_alpha");
}

#[test]
fn test_id_collision_folder_precedence() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // 1. Buat folder dekomposisi "item_conflict"
    let decomp_dir = sr_dir.join("item_conflict");
    fs::create_dir_all(&decomp_dir).unwrap();
    let folder_meta: StoredLayerFile = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "id": "item_conflict",
        "layer_name": "Versi Folder Dekomposisi",
        "display_preference": {
            "type": "point",
            "color": "#1ABC9C",
            "icon": "pin"
        },
        "feature_count": 0
    }))
    .unwrap();
    fs::write(
        decomp_dir.join("layer.json"),
        serde_json::to_string_pretty(&folder_meta).unwrap(),
    )
    .unwrap();

    // 2. Buat flat file bayangan "item_conflict.json"
    let flat_meta: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "item_conflict",
        "layer_name": "Versi Flat Bayangan",
        "display_preference": {
            "type": "polygon",
            "color": "#E74C3C",
            "opacity": 0.5
        },
        "spatial_results": []
    }))
    .unwrap();
    fs::write(
        sr_dir.join("item_conflict.json"),
        serde_json::to_string_pretty(&flat_meta).unwrap(),
    )
    .unwrap();

    // 3. Baca daftar dan pastikan folder dekomposisi menang & tidak ada duplikasi
    let layers = get_saved_spatial_results_in_dir(temp.path()).unwrap();
    assert_eq!(layers.len(), 1, "Hanya satu entri yang boleh muncul");
    assert_eq!(layers[0].layer_id, "item_conflict");
    assert_eq!(layers[0].layer_name, "Versi Folder Dekomposisi");

    let summaries = get_saved_spatial_results_summary_in_dir(temp.path()).unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "item_conflict");
    assert_eq!(summaries[0].format_type, "decomposed");
    assert_eq!(summaries[0].layer_name, "Versi Folder Dekomposisi");
}

#[test]
fn test_corrupted_flat_file_graceful_handling() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // 1. Berkas JSON korup / terpotong
    fs::write(
        sr_dir.join("corrupted_flat.json"),
        "{ this is invalid syntax JSON: [",
    )
    .unwrap();

    // 2. Folder dekomposisi rusak tanpa layer.json
    fs::create_dir_all(sr_dir.join("broken_folder")).unwrap();

    // Pastikan get_saved_spatial_results_summary_in_dir TIDAK crash atau error fatal
    let summaries = get_saved_spatial_results_summary_in_dir(temp.path())
        .expect("Harus mengembalikan daftar ringkasan dengan fallback graceful");

    assert_eq!(summaries.len(), 2, "Harus memuat kedua entri rusak");

    let corrupted_flat = summaries
        .iter()
        .find(|s| s.id == "corrupted_flat")
        .expect("corrupted_flat harus ada dalam daftar");
    assert!(corrupted_flat.is_corrupted);
    assert_eq!(corrupted_flat.display_color, "#E74C3C");
    assert_eq!(corrupted_flat.geom_type, "Unknown");

    let broken_folder = summaries
        .iter()
        .find(|s| s.id == "broken_folder")
        .expect("broken_folder harus ada dalam daftar");
    assert!(broken_folder.is_corrupted);
    assert_eq!(broken_folder.display_color, "#E74C3C");
}

#[test]
fn test_id_allowlist_and_path_traversal_rejection() {
    let temp = tempdir().expect("Gagal membuat tempdir");

    // 1. Path traversal attacks
    let long_id = "a".repeat(65);
    let malicious_ids = vec![
        "../../etc/passwd",
        r"..\..\windows\system32\cmd.exe",
        "../escape",
        "folder/subfile",
        r"folder\subfile",
        "",
        long_id.as_str(), // > 64 chars
        "invalid@char",
        "has space",
        "test#id",
    ];

    for bad_id in malicious_ids {
        assert!(
            validate_spatial_result_id(bad_id).is_err(),
            "ID '{}' seharusnya ditolak oleh validator",
            bad_id
        );

        let read_res = get_saved_spatial_result_in_dir(temp.path(), bad_id);
        assert!(
            read_res.is_err(),
            "get_saved_spatial_result_in_dir harus menolak ID '{}'",
            bad_id
        );
    }

    // 2. Valid IDs
    let valid_ids = vec![
        "result_trees_001",
        "result-veg-analysis",
        "survey_2026_10_24",
        "ORTHO_LAYER_01",
        "simple",
    ];

    for good_id in valid_ids {
        assert!(
            validate_spatial_result_id(good_id).is_ok(),
            "ID '{}' seharusnya valid",
            good_id
        );
    }
}

#[test]
fn test_thumbnail_url_resolution_hierarchy() {
    // 1. Prioritas 1: display_preference ImageOverlay memiliki image_url
    let overlay_payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "test_overlay",
        "layer_name": "Overlay Test",
        "display_preference": {
            "type": "image_overlay",
            "image_url": "/photos/raw/ortho_highres.jpg",
            "bounds": [[-1.25, 116.89], [-1.24, 116.90]],
            "opacity": 0.8
        },
        "spatial_results": [{
            "source": "drone",
            "geometry": { "type": "Point", "coordinates": [116.89, -1.24] },
            "properties": {
                "tree_id": "T-99",
                "confidence": 0.9,
                "height_est_m": 10.0,
                "image_url": "/photos/raw/feature_ignore.jpg"
            },
            "image_url": "/photos/raw/feature_ignore.jpg"
        }]
    }))
    .unwrap();
    assert_eq!(
        extract_thumbnail_url(&overlay_payload),
        Some("/photos/raw/ortho_highres.jpg".to_string()),
        "Prioritas 1 (ImageOverlay layer-level) harus diutamakan"
    );

    // 2. Prioritas 2: display_preference Point, fitur pertama memiliki feature.image_url
    let feat_img_payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "test_feat_img",
        "layer_name": "Fitur Berfoto",
        "display_preference": {
            "type": "point",
            "color": "#3498DB",
            "icon": "camera"
        },
        "spatial_results": [
            {
                "source": "drone",
                "geometry": { "type": "Point", "coordinates": [116.89, -1.24] },
                "properties": {
                    "path_id": "P-01",
                    "type": "Soil",
                    "length_m": 120.0,
                    "image_url": "/photos/raw/prop_fallback.jpg"
                },
                "image_url": "/photos/raw/first_feature_photo.jpg"
            },
            {
                "source": "drone",
                "geometry": { "type": "Point", "coordinates": [116.89, -1.24] },
                "properties": {
                    "path_id": "P-02",
                    "type": "Soil",
                    "length_m": 80.0
                },
                "image_url": "/photos/raw/second_feature_photo.jpg"
            }
        ]
    }))
    .unwrap();
    assert_eq!(
        extract_thumbnail_url(&feat_img_payload),
        Some("/photos/raw/first_feature_photo.jpg".to_string()),
        "Prioritas 2 (feature.image_url pada fitur pertama) harus terpilih"
    );

    // 3. Prioritas 3: feature.image_url adalah None, tetapi properties.image_url ada
    let prop_img_payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "test_prop_img",
        "layer_name": "Properties Berfoto",
        "display_preference": {
            "type": "polygon",
            "color": "#27AE60",
            "opacity": 0.7
        },
        "spatial_results": [{
            "source": "drone",
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[116.89, -1.24], [116.90, -1.24]]]
            },
            "properties": {
                "vegetation_index_type": "NDVI",
                "mean_greenness_score": 0.81,
                "area_sqm": 4500.0,
                "image_url": "/photos/thumbnails/veg_thumb.jpg"
            }
        }]
    }))
    .unwrap();
    assert_eq!(
        extract_thumbnail_url(&prop_img_payload),
        Some("/photos/thumbnails/veg_thumb.jpg".to_string()),
        "Prioritas 3 (properties.image_url pada fitur pertama) harus terpilih saat feature.image_url None"
    );

    // 4. Prioritas 4 (Fallback): Tidak ada foto
    let no_img_payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "test_no_img",
        "layer_name": "Tanpa Foto",
        "display_preference": {
            "type": "polygon",
            "color": "#95A5A6",
            "opacity": 0.5
        },
        "spatial_results": []
    }))
    .unwrap();
    assert_eq!(
        extract_thumbnail_url(&no_img_payload),
        None,
        "Prioritas 4 harus menghasilkan None jika tidak ada URL foto"
    );
}

#[test]
fn test_get_single_saved_spatial_result_detail() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // 1. Simpan satu flat result
    let payload: LayerPayload = serde_json::from_value(serde_json::json!({
        "layer_id": "result_test_detail",
        "layer_name": "Detail Uji Coba",
        "display_preference": {
            "type": "point",
            "color": "#F39C12",
            "icon": "dot"
        },
        "spatial_results": [{
            "source": "drone",
            "geometry": { "type": "Point", "coordinates": [116.89, -1.24] },
            "properties": {
                "tree_id": "T-01",
                "confidence": 0.98,
                "height_est_m": 14.0
            }
        }],
        "updated_at": "2026-10-24T12:00:00Z"
    }))
    .unwrap();
    fs::write(
        sr_dir.join("result_test_detail.json"),
        serde_json::to_string_pretty(&payload).unwrap(),
    )
    .unwrap();

    // 2. Ambil detail yang ada
    let detail = get_saved_spatial_result_in_dir(temp.path(), "result_test_detail")
        .expect("Harus berhasil mengambil detail");
    assert_eq!(detail.layer_id, "result_test_detail");
    assert_eq!(detail.layer_name, "Detail Uji Coba");
    assert_eq!(detail.spatial_results.len(), 1);
    assert_eq!(detail.updated_at.as_deref(), Some("2026-10-24T12:00:00Z"));

    // 3. Ambil ID yang tidak ada
    let err = get_saved_spatial_result_in_dir(temp.path(), "non_existent_id")
        .expect_err("Harus error NotFound");
    match err {
        MapControllerError::NotFound(msg) => {
            assert!(msg.contains("non_existent_id"));
        }
        _ => panic!("Tipe error harus NotFound, didapat: {:?}", err),
    }
}

#[test]
fn test_safe_update_semantic_identity() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // 1. Siapkan berkas flat dengan koordinat presisi tinggi dan custom properties
    let original_json = r##"{
        "layer_id": "result_complex_survey",
        "layer_name": "Survei Geometri Kompleks",
        "display_preference": {
            "type": "point",
            "color": "#3498DB",
            "icon": "pin"
        },
        "spatial_results": [
            {
                "source": "drone_uav_2026",
                "geometry": {
                    "type": "Point",
                    "coordinates": [116.891234567, -1.245678912]
                },
                "properties": {
                    "tree_id": "T-ALPHA-99",
                    "confidence": 0.987654,
                    "height_est_m": 15.4321
                }
            }
        ]
    }"##;

    let target_file = sr_dir.join("result_complex_survey.json");
    fs::write(&target_file, original_json).unwrap();
    let original_val: serde_json::Value = serde_json::from_str(original_json).unwrap();

    // 2. Lakukan update aman (hanya ganti layer_name, display_color, icon)
    let update_payload = UpdateSpatialResultPayload {
        id: "result_complex_survey".to_string(),
        name: Some("Survei Kompleks (Telah Direvisi)".to_string()),
        display_color: Some("#E74C3C".to_string()),
        icon: Some("tree".to_string()),
        ..Default::default()
    };

    let updated = update_spatial_result_in_dir(temp.path(), update_payload)
        .expect("Update harus berhasil dilakukan");

    assert_eq!(updated.layer_name, "Survei Kompleks (Telah Direvisi)");
    assert!(updated.updated_at.is_some());

    // 3. Baca ulang JSON mentah dari disk untuk verifikasi semantik
    let updated_content = fs::read_to_string(&target_file).unwrap();
    let updated_val: serde_json::Value = serde_json::from_str(&updated_content).unwrap();

    // Verifikasi field yang diubah
    assert_eq!(
        updated_val["layer_name"],
        "Survei Kompleks (Telah Direvisi)"
    );
    assert_eq!(updated_val["display_preference"]["color"], "#E74C3C");
    assert_eq!(updated_val["display_preference"]["icon"], "tree");

    // BUKTI IDENTIK SECARA SEMANTIK:
    // Seluruh array spatial_results (koordinat, geometri, properties) terbukti Value-nya sama persis!
    assert_eq!(
        updated_val["spatial_results"], original_val["spatial_results"],
        "Seluruh data spasial (koordinat, geometri, properties) HARUS terbukti identik secara semantik"
    );
}

#[test]
fn test_color_hex_regex_validation() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    let dummy_json = r##"{
        "layer_id": "result_hex_test",
        "layer_name": "Test Hex",
        "display_preference": {
            "type": "point",
            "color": "#2ECC71",
            "icon": ""
        },
        "spatial_results": []
    }"##;
    fs::write(sr_dir.join("result_hex_test.json"), dummy_json).unwrap();

    // 1. HEX valid (3 digit & 6 digit, case insensitive)
    let valid_hex_colors = vec!["#2ECC71", "#fff", "#1A2B3C", "#ABC", "#123456", "#a1b2c3"];
    for color in valid_hex_colors {
        assert!(
            validate_hex_color(color).is_ok(),
            "Warna '{}' seharusnya valid",
            color
        );

        let res = update_spatial_result_in_dir(
            temp.path(),
            UpdateSpatialResultPayload {
                id: "result_hex_test".to_string(),
                name: None,
                display_color: Some(color.to_string()),
                icon: None,
                ..Default::default()
            },
        );
        assert!(res.is_ok(), "Update dengan warna '{}' harus sukses", color);
    }

    // 2. HEX tidak valid
    let invalid_hex_colors = vec![
        "red", "#12345", "123456", "#gggggg", "", "#1234567", "#12", " #2ECC71",
    ];
    for bad_color in invalid_hex_colors {
        assert!(
            validate_hex_color(bad_color).is_err(),
            "Warna '{}' seharusnya ditolak",
            bad_color
        );

        let res = update_spatial_result_in_dir(
            temp.path(),
            UpdateSpatialResultPayload {
                id: "result_hex_test".to_string(),
                name: None,
                display_color: Some(bad_color.to_string()),
                icon: None,
                ..Default::default()
            },
        );
        assert!(
            res.is_err(),
            "Update dengan warna tidak valid '{}' harus ditolak",
            bad_color
        );
    }
}

#[test]
fn test_delete_preserves_referenced_image_url() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    let photos_dir = temp.path().join("photos").join("raw");
    fs::create_dir_all(&sr_dir).unwrap();
    fs::create_dir_all(&photos_dir).unwrap();

    // 1. Buat file citra fisik bersama di folder photos/raw/
    let shared_photo = photos_dir.join("sample_drone_survey.jpg");
    fs::write(&shared_photo, b"FAKE_JPEG_IMAGE_DATA_12345").unwrap();
    assert!(shared_photo.is_file());

    // 2. Buat spatial result yang merujuk foto tersebut
    let layer_json = r##"{
        "layer_id": "result_with_photo",
        "layer_name": "Layer Berfoto Bersama",
        "display_preference": {
            "type": "point",
            "color": "#3498DB",
            "icon": "camera"
        },
        "spatial_results": [
            {
                "source": "drone",
                "geometry": { "type": "Point", "coordinates": [116.89, -1.24] },
                "properties": {
                    "tree_id": "T-1",
                    "confidence": 0.9,
                    "height_est_m": 10.0,
                    "image_url": "/photos/raw/sample_drone_survey.jpg"
                },
                "image_url": "/photos/raw/sample_drone_survey.jpg"
            }
        ]
    }"##;
    fs::write(sr_dir.join("result_with_photo.json"), layer_json).unwrap();

    // 3. Jalankan penghapusan spatial result
    let deleted = delete_saved_spatial_result_in_dir(temp.path(), "result_with_photo")
        .expect("Penghapusan spatial result harus sukses");
    assert!(deleted);

    // 4. Verifikasi bahwa file spatial result terhapus
    assert!(!sr_dir.join("result_with_photo.json").exists());

    // 5. ATURAN KRUSIAL: Berkas citra fisik bersama TIDAK PERNAH DISENTUH ATAU DIHAPUS!
    assert!(
        shared_photo.is_file(),
        "Berkas foto bersama yang dirujuk image_url HARUS tetap utuh di disk!"
    );
    assert_eq!(
        fs::read(&shared_photo).unwrap(),
        b"FAKE_JPEG_IMAGE_DATA_12345"
    );
}

#[test]
fn test_count_spatial_result_dependents() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    let layers_dir = temp.path().join("layers");
    fs::create_dir_all(&sr_dir).unwrap();
    fs::create_dir_all(&layers_dir).unwrap();

    // 1. Siapkan folder dekomposisi dengan 2 fitur
    let decomp_dir = sr_dir.join("result_dep_test");
    let feat_dir = decomp_dir.join("features");
    fs::create_dir_all(&feat_dir).unwrap();
    fs::write(
        decomp_dir.join("layer.json"),
        r##"{"schema_version":1,"id":"result_dep_test","layer_name":"Dep Test","display_preference":{"type":"point","color":"#fff","icon":""},"feature_count":2}"##,
    )
    .unwrap();
    fs::write(feat_dir.join("f1.json"), "{}").unwrap();
    fs::write(feat_dir.join("f2.json"), "{}").unwrap();

    // 2. Siapkan registrasi layer di layers/
    fs::write(
        layers_dir.join("result_dep_test.json"),
        r#"{"layer_id":"result_dep_test","layer_name":"Dep Test","category":"point","is_visible":true,"opacity":1.0,"z_index":10}"#,
    )
    .unwrap();

    // 3. Hitung dependensi
    let count_summary: DeleteSpatialResultSummary =
        count_spatial_result_dependents_in_dir(temp.path(), "result_dep_test")
            .expect("Harus berhasil menghitung dependensi");
    assert_eq!(count_summary.id, "result_dep_test");
    assert_eq!(count_summary.feature_count, 2);
    assert_eq!(count_summary.layers_removed, 1);
    assert!(count_summary.deleted_from_disk);
}

#[test]
fn test_transactional_2phase_delete_and_rollback() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    let layers_dir = temp.path().join("layers");
    fs::create_dir_all(&sr_dir).unwrap();
    fs::create_dir_all(&layers_dir).unwrap();

    // Buat flat file
    fs::write(
        sr_dir.join("item_to_delete.json"),
        r##"{"layer_id":"item_to_delete","layer_name":"Item To Delete","display_preference":{"type":"polygon","color":"#000","opacity":0.5},"spatial_results":[]}"##,
    )
    .unwrap();

    // Buat layer registration
    fs::write(
        layers_dir.join("item_to_delete.json"),
        r#"{"layer_id":"item_to_delete","layer_name":"Item To Delete","category":"polygon","is_visible":true,"opacity":0.5,"z_index":10}"#,
    )
    .unwrap();

    // Hapus normal (sukses 2 fase)
    let res = delete_saved_spatial_result_in_dir(temp.path(), "item_to_delete").unwrap();
    assert!(res);
    assert!(!sr_dir.join("item_to_delete.json").exists());
    assert!(!layers_dir.join("item_to_delete.json").exists());

    // Pastikan tidak ada sisa folder karantina .trash_sr_*
    for entry in fs::read_dir(&sr_dir).unwrap().filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.starts_with(".trash_sr_"),
            "Folder karantina harus sudah terhapus bersih, didapat: {}",
            name
        );
    }
}

#[test]
fn test_cleanup_orphaned_trash() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // Buat 1 file normal
    fs::write(sr_dir.join("normal_item.json"), "{}").unwrap();

    // Buat sampah karantina sisa crash
    let trash_folder = sr_dir.join(".trash_sr_1792812345_orphan");
    fs::create_dir_all(&trash_folder).unwrap();
    fs::write(trash_folder.join("leftover.json"), "{}").unwrap();

    let trash_file = sr_dir.join(".trash_sr_1792812345_orphan_file.json");
    fs::write(&trash_file, "{}").unwrap();

    assert!(trash_folder.exists());
    assert!(trash_file.exists());

    // Jalankan pembersihan sisa karantina
    cleanup_orphaned_trash(temp.path());

    // Folder sampah harus bersih
    assert!(!trash_folder.exists());
    assert!(!trash_file.exists());

    // File normal tetap utuh
    assert!(sr_dir.join("normal_item.json").exists());
}

#[test]
fn test_update_spatial_result_image_replace_and_delete() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    // 1. Siapkan file spatial result awal dengan image_url bawaan
    let initial_json = r##"{
        "layer_id": "result_photo_test_01",
        "layer_name": "Test Layer Foto",
        "display_preference": {
            "type": "point",
            "color": "#2ECC71",
            "icon": "tree"
        },
        "spatial_results": [
            {
                "source": "tree_detection_v1",
                "geometry": { "type": "Point", "coordinates": [116.85, -1.25] },
                "properties": {
                    "tree_id": "T-100",
                    "confidence": 0.9,
                    "height_est_m": 8.0
                }
            }
        ]
    }"##;

    let target_file = sr_dir.join("result_photo_test_01.json");
    fs::write(&target_file, initial_json).unwrap();

    // Siapkan file JPEG dummy dengan magic bytes sah (FF D8 FF)
    let dummy_img_path = temp.path().join("new_photo.jpg");
    let jpeg_bytes = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
    ];
    fs::write(&dummy_img_path, &jpeg_bytes).unwrap();

    // 2. Lakukan replace image ke /map_storage/images/result_image/
    let replace_payload = UpdateSpatialResultPayload {
        id: "result_photo_test_01".to_string(),
        name: None,
        display_color: None,
        icon: None,
        image_action: Some("replace".to_string()),
        image_path: Some(dummy_img_path.to_string_lossy().to_string()),
        ..Default::default()
    };

    let updated = update_spatial_result_in_dir(temp.path(), replace_payload)
        .expect("Update replace image harus berhasil");

    // Periksa thumbnail URL yang diekstraksi
    let thumb = extract_thumbnail_url(&updated);
    assert!(thumb.is_some(), "Thumbnail URL harus ada setelah diganti");
    let thumb_url = thumb.unwrap();
    assert!(
        thumb_url.starts_with("/map_storage/images/result_image/result_photo_test_01_"),
        "URL harus merujuk ke folder result_image: {}",
        thumb_url
    );
    assert!(thumb_url.ends_with(".jpg"));

    // Periksa file fisik tersimpan di disk pada images/result_image/
    let result_img_dir = temp.path().join("images").join("result_image");
    assert!(result_img_dir.is_dir());
    let mut saved_files = Vec::new();
    for entry in fs::read_dir(&result_img_dir)
        .unwrap()
        .filter_map(|e| e.ok())
    {
        saved_files.push(entry.file_name().to_string_lossy().to_string());
    }
    assert_eq!(
        saved_files.len(),
        1,
        "Harus ada tepat 1 berkas gambar di result_image/"
    );

    // 3. Lakukan delete image (menjadi 'no-image')
    let delete_payload = UpdateSpatialResultPayload {
        id: "result_photo_test_01".to_string(),
        name: None,
        display_color: None,
        icon: None,
        image_action: Some("delete".to_string()),
        image_path: None,
        ..Default::default()
    };

    let cleared = update_spatial_result_in_dir(temp.path(), delete_payload)
        .expect("Update delete image harus berhasil");

    // Thumbnail URL harus None ("no-image")
    assert_eq!(
        extract_thumbnail_url(&cleared),
        None,
        "Harus menjadi No Image"
    );

    // Berkas fisik di result_image/ harus terhapus bersih
    let remaining_files = fs::read_dir(&result_img_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .count();
    assert_eq!(
        remaining_files, 0,
        "Berkas gambar di result_image/ harus dibersihkan"
    );
}

#[test]
fn test_roundtrip_templates_and_real_files() {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| "src-tauri".to_string()),
    );
    let find_path = |rel: &str| {
        let p = manifest_dir.join(rel);
        if p.exists() {
            return p;
        }
        let p_parent = manifest_dir.join("..").join(rel);
        if p_parent.exists() {
            return p_parent;
        }
        let p_direct = std::path::Path::new(rel);
        if p_direct.exists() {
            return p_direct.to_path_buf();
        }
        panic!(
            "Path berkas tidak ditemukan di lokasi mana pun: {} (manifest: {:?})",
            rel, manifest_dir
        );
    };

    // 1. Uji round-trip 4 berkas template data bawaan
    let template_paths = [
        "src/modules/module_03/data_template/template_crop_damage.json",
        "src/modules/module_03/data_template/template_inspection_path.json",
        "src/modules/module_03/data_template/template_landcover_polygon.json",
        "src/modules/module_03/data_template/template_tree_detection.json",
    ];

    for rel_path in &template_paths {
        let full_path = find_path(rel_path);
        let content = fs::read_to_string(&full_path)
            .unwrap_or_else(|e| panic!("Gagal membaca template {}: {}", rel_path, e));
        let raw_val: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Deserialisasi bertipe ke LayerPayload
        let payload: LayerPayload = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Gagal deserialize template {}: {}", rel_path, e));

        // Serialisasi ulang ke JSON string
        let roundtrip_str = serde_json::to_string_pretty(&payload).unwrap();
        let roundtrip_val: serde_json::Value = serde_json::from_str(&roundtrip_str).unwrap();

        // Verifikasi bahwa SEMUA key properties selamat dari pembacaan bertipe
        let raw_features = raw_val["spatial_results"].as_array().unwrap();
        let rt_features = roundtrip_val["spatial_results"].as_array().unwrap();
        assert_eq!(
            raw_features.len(),
            rt_features.len(),
            "Jumlah fitur harus identik pada {}",
            rel_path
        );

        for (i, raw_feat) in raw_features.iter().enumerate() {
            let raw_props = raw_feat["properties"].as_object().unwrap();
            let rt_props = rt_features[i]["properties"].as_object().unwrap();

            for (k, v) in raw_props {
                assert!(
                    rt_props.contains_key(k),
                    "Template {}: Key properties '{}' pada fitur [{}] HILANG setelah pembacaan bertipe!",
                    rel_path,
                    k,
                    i
                );
                assert_eq!(
                    rt_props.get(k).unwrap(),
                    v,
                    "Template {}: Nilai key '{}' berubah setelah roundtrip!",
                    rel_path,
                    k
                );
            }
        }
    }

    // 2. Uji round-trip 5 berkas data spasial riil
    let real_paths = [
        "map_storage/spatial_results/result_landcover_001.json",
        "map_storage/spatial_results/result_line_001.json",
        "map_storage/spatial_results/result_tree_001.json",
        "map_storage/spatial_results/result_tree_002.json",
        "map_storage/spatial_results/result_vegetation_001.json",
    ];

    for rel_path in &real_paths {
        let full_path = find_path(rel_path);
        let content = fs::read_to_string(&full_path)
            .unwrap_or_else(|e| panic!("Gagal membaca berkas riil {}: {}", rel_path, e));
        let raw_val: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Deserialisasi bertipe ke LayerPayload
        let payload: LayerPayload = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Gagal deserialize berkas riil {}: {}", rel_path, e));

        // Serialisasi ulang ke JSON string
        let roundtrip_str = serde_json::to_string_pretty(&payload).unwrap();
        let roundtrip_val: serde_json::Value = serde_json::from_str(&roundtrip_str).unwrap();

        let raw_features = raw_val["spatial_results"].as_array().unwrap();
        let rt_features = roundtrip_val["spatial_results"].as_array().unwrap();
        assert_eq!(
            raw_features.len(),
            rt_features.len(),
            "Jumlah fitur harus identik pada {}",
            rel_path
        );

        for (i, raw_feat) in raw_features.iter().enumerate() {
            let raw_props = raw_feat["properties"].as_object().unwrap();
            let rt_props = rt_features[i]["properties"].as_object().unwrap();

            for (k, v) in raw_props {
                assert!(
                    rt_props.contains_key(k),
                    "Berkas {}: Key properties '{}' pada fitur [{}] HILANG setelah pembacaan bertipe!",
                    rel_path,
                    k,
                    i
                );
                assert_eq!(
                    rt_props.get(k).unwrap(),
                    v,
                    "Berkas {}: Nilai key '{}' berubah setelah roundtrip!",
                    rel_path,
                    k
                );
            }
        }
    }
}

#[test]
fn test_fielddef_and_payload_unknown_fields() {
    // 1. FieldDef tersimpan: TANPA deny_unknown_fields, visible default true, custom default false
    let field_json = r#"{
        "key": "catatan_lapangan",
        "label": "Catatan Lapangan",
        "unknown_future_meta": "extra_value",
        "another_field": 123
    }"#;
    let field: FieldDef =
        serde_json::from_str(field_json).expect("FieldDef harus menerima unknown fields");
    assert_eq!(field.key, "catatan_lapangan");
    assert_eq!(field.label, "Catatan Lapangan");
    assert!(field.visible, "visible harus default true");
    assert!(!field.custom, "custom harus default false");
    assert_eq!(field.kind, None);

    // 2. UpdateSpatialResultPayload: DENGAN deny_unknown_fields (payload menolak field asing)
    let payload_json = r#"{
        "id": "result_01",
        "unexpected_key": "malicious_payload"
    }"#;
    let payload_res: Result<UpdateSpatialResultPayload, _> = serde_json::from_str(payload_json);
    assert!(
        payload_res.is_err(),
        "Payload input harus menolak unknown fields"
    );
}

#[test]
fn test_startup_trash_recovery_with_registration_and_without() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    let layers_dir = temp.path().join("layers");
    let photos_dir = temp.path().join("images").join("result_image");
    fs::create_dir_all(&sr_dir).unwrap();
    fs::create_dir_all(&layers_dir).unwrap();
    fs::create_dir_all(&photos_dir).unwrap();

    // Skenario A: Registrasi layer masih ada di layers/ (misal crash di fase 2 delete)
    // -> Startup harus MEMULIHKAN layer dan foto!
    fs::write(
        layers_dir.join("result_layer_recover.json"),
        r#"{"id":"result_layer_recover"}"#,
    )
    .unwrap();
    let trash_a = sr_dir.join(".trash_sr_result_layer_recover_1792825000000");
    let trash_a_photos = trash_a.join("photos");
    fs::create_dir_all(&trash_a_photos).unwrap();
    fs::write(trash_a.join("flat.json"), r#"{"recovered": true}"#).unwrap();
    fs::write(
        trash_a_photos.join("result_layer_recover_1792825000000.png"),
        "image-bytes",
    )
    .unwrap();

    // Skenario B: Registrasi layer TIDAK ada di layers/ (registrasi sudah terhapus bersih)
    // -> Startup harus MENGHAPUS PERMANEN trash!
    let trash_b = sr_dir.join(".trash_sr_result_layer_purge_1792825000000");
    let trash_b_photos = trash_b.join("photos");
    fs::create_dir_all(&trash_b_photos).unwrap();
    fs::write(trash_b.join("flat.json"), r#"{"purged": true}"#).unwrap();
    fs::write(
        trash_b_photos.join("result_layer_purge_1792825000000.png"),
        "image-bytes",
    )
    .unwrap();

    // Jalankan recovery saat startup
    cleanup_orphaned_trash(temp.path());

    // Verifikasi Skenario A:
    assert!(!trash_a.exists(), "Folder sampah A harus sudah dibersihkan");
    assert!(
        sr_dir.join("result_layer_recover.json").exists(),
        "Berkas layer_recover harus dipulihkan"
    );
    assert!(
        photos_dir
            .join("result_layer_recover_1792825000000.png")
            .exists(),
        "Foto layer_recover harus dipulihkan"
    );

    // Verifikasi Skenario B:
    assert!(
        !trash_b.exists(),
        "Folder sampah B harus sudah dihapus permanen"
    );
    assert!(
        !sr_dir.join("result_layer_purge.json").exists(),
        "Berkas layer_purge TIDAK boleh dipulihkan"
    );
    assert!(
        !photos_dir
            .join("result_layer_purge_1792825000000.png")
            .exists(),
        "Foto layer_purge TIDAK boleh dipulihkan"
    );
}

#[test]
fn test_custom_field_regex_and_30_limit() {
    // 1. Regex key hanya untuk field custom baru
    assert!(validate_custom_field_key("custom_field_1").is_ok());
    assert!(validate_custom_field_key("catatan").is_ok());
    assert!(validate_custom_field_key("a").is_ok());

    // Ditolak: huruf kapital, dimulai angka, ada strip, spasi, reserved
    assert!(validate_custom_field_key("CustomField").is_err());
    assert!(validate_custom_field_key("1custom").is_err());
    assert!(validate_custom_field_key("custom-field").is_err());
    assert!(validate_custom_field_key("custom field").is_err());
    assert!(validate_custom_field_key("image_url").is_err());
    assert!(validate_custom_field_key("edited_fields").is_err());
    assert!(validate_custom_field_key("properties").is_err());
    assert!(validate_custom_field_key(&"a".repeat(41)).is_err());

    // 2. Batas 30 hanya untuk field custom
    let mut fields = Vec::new();
    // Tambah 20 field bawaan (custom = false)
    for i in 0..20 {
        fields.push(FieldDef {
            key: format!("plugin_field_{}", i),
            label: format!("Plugin Field {}", i),
            unit: None,
            visible: true,
            custom: false,
            kind: None,
        });
    }
    // Tambah 30 field custom (custom = true) -> total 50, custom 30 -> Sah!
    for i in 0..30 {
        fields.push(FieldDef {
            key: format!("custom_{}", i),
            label: format!("Custom {}", i),
            unit: None,
            visible: true,
            custom: true,
            kind: Some("text".to_string()),
        });
    }
    assert!(validate_custom_fields_count(&fields).is_ok());

    // Tambah field custom ke-31 -> Harus Err!
    fields.push(FieldDef {
        key: "custom_31".to_string(),
        label: "Custom 31".to_string(),
        unit: None,
        visible: true,
        custom: true,
        kind: Some("text".to_string()),
    });
    assert!(validate_custom_fields_count(&fields).is_err());

    // 3. Validasi label dan unit
    assert!(validate_field_label("Nama Label Valid").is_ok());
    assert!(validate_field_label("").is_err());
    assert!(validate_field_label(&"a".repeat(41)).is_err());
    assert!(validate_field_unit(Some("cm")).is_ok());
    assert!(validate_field_unit(None).is_ok());
    assert!(validate_field_unit(Some(&"a".repeat(13))).is_err());
}

#[test]
fn test_property_value_type_preservation() {
    // 1. Validasi tipe nilai mempertahankan tipe JSON asli
    let num_val = serde_json::json!(42.5);
    let str_val = serde_json::json!("teks valid");
    let bool_val = serde_json::json!(true);
    let obj_val = serde_json::json!({"a": 1});
    let arr_val = serde_json::json!([1, 2, 3]);

    // Number -> Number: Ok, Number -> String: Err, Number -> Bool: Err
    assert!(validate_property_value_type(Some(&num_val), &serde_json::json!(100.0), None).is_ok());
    assert!(validate_property_value_type(Some(&num_val), &str_val, None).is_err());
    assert!(validate_property_value_type(Some(&num_val), &bool_val, None).is_err());

    // String -> String: Ok, String -> Number: Err
    assert!(
        validate_property_value_type(Some(&str_val), &serde_json::json!("teks baru"), None).is_ok()
    );
    assert!(validate_property_value_type(Some(&str_val), &num_val, None).is_err());

    // Bool -> Bool: Ok, Bool -> Number: Err
    assert!(validate_property_value_type(Some(&bool_val), &serde_json::json!(false), None).is_ok());
    assert!(validate_property_value_type(Some(&bool_val), &num_val, None).is_err());

    // Object/Array kompleks: selalu Err
    assert!(validate_property_value_type(None, &obj_val, None).is_err());
    assert!(validate_property_value_type(None, &arr_val, None).is_err());

    // Custom field baru (tanpa existing value) divalidasi terhadap kind
    assert!(validate_property_value_type(None, &num_val, Some("number")).is_ok());
    assert!(validate_property_value_type(None, &str_val, Some("number")).is_err());
    assert!(validate_property_value_type(None, &str_val, Some("text")).is_ok());
    assert!(validate_property_value_type(None, &bool_val, Some("boolean")).is_ok());
}

#[test]
fn test_concurrency_token_and_manual_edits_command() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    fs::create_dir_all(&sr_dir).unwrap();

    let initial_json = r##"{
        "layer_id": "result_concurrency_test",
        "layer_name": "Test Konkurensi",
        "display_preference": {
            "type": "point",
            "color": "#2ECC71",
            "icon": "tree"
        },
        "spatial_results": [
            {
                "source": "tree_plugin",
                "geometry": { "type": "Point", "coordinates": [116.85, -1.25] },
                "properties": {
                    "tree_id": "T-01",
                    "confidence": 0.95
                }
            }
        ]
    }"##;
    fs::write(sr_dir.join("result_concurrency_test.json"), initial_json).unwrap();

    // 1. Baca detail awal & periksa concurrency_token
    let initial_layer =
        get_saved_spatial_result_in_dir(temp.path(), "result_concurrency_test").unwrap();
    assert!(initial_layer.concurrency_token.is_some());
    let token = initial_layer.concurrency_token.unwrap();

    // Periksa check_spatial_result_manual_edits awal: harus false
    let check_before =
        check_spatial_result_manual_edits_in_dir(temp.path(), "result_concurrency_test").unwrap();
    assert!(!check_before.has_manual_edits);
    assert_eq!(check_before.edited_features_count, 0);

    // 2. Update dengan expected_updated_at SALAH -> harus Err(StorageError)
    let bad_payload = UpdateSpatialResultPayload {
        id: "result_concurrency_test".to_string(),
        name: Some("Nama Baru".to_string()),
        expected_updated_at: Some("token_salah_9999".to_string()),
        ..Default::default()
    };
    let bad_res = update_spatial_result_in_dir(temp.path(), bad_payload);
    assert!(
        bad_res.is_err(),
        "Update dengan concurrency token salah harus gagal!"
    );

    // 3. Update dengan expected_updated_at BENAR dan lakukan feature edit
    let mut edit_map = std::collections::HashMap::new();
    edit_map.insert("confidence".to_string(), serde_json::json!(0.99));
    let good_payload = UpdateSpatialResultPayload {
        id: "result_concurrency_test".to_string(),
        name: Some("Nama Berhasil Diubah".to_string()),
        expected_updated_at: Some(token),
        feature_edits: Some(vec![FeatureEdit {
            index: 0,
            set: edit_map,
            remove: vec![],
        }]),
        ..Default::default()
    };
    let updated = update_spatial_result_in_dir(temp.path(), good_payload).unwrap();
    assert_eq!(updated.layer_name, "Nama Berhasil Diubah");
    assert!(updated.concurrency_token.is_some());

    // 4. Verifikasi edited_fields pada hasil update
    let feat0 = &updated.spatial_results[0];
    assert_eq!(feat0.edited_fields, Some(vec!["confidence".to_string()]));

    // 5. Periksa check_spatial_result_manual_edits setelah edit: harus true
    let check_after =
        check_spatial_result_manual_edits_in_dir(temp.path(), "result_concurrency_test").unwrap();
    assert!(check_after.has_manual_edits);
    assert_eq!(check_after.edited_features_count, 1);

    // 6. Ringkasan summary juga harus memiliki has_manual_edits = true
    let summaries = get_saved_spatial_results_summary_in_dir(temp.path()).unwrap();
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].has_manual_edits);
    assert!(!summaries[0].concurrency_token.is_empty());
}

#[test]
fn test_delete_owned_photos_vs_public_assets() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let sr_dir = temp.path().join("spatial_results");
    let photos_dir = temp.path().join("images").join("result_image");
    fs::create_dir_all(&sr_dir).unwrap();
    fs::create_dir_all(&photos_dir).unwrap();

    let owned_photo_name = "test_del_photo_1792825123456.png";
    fs::write(photos_dir.join(owned_photo_name), "owned-photo-data").unwrap();

    // Verifikasi fungsi murni is_owned_result_photo
    assert!(is_owned_result_photo("test_del_photo", owned_photo_name));
    assert!(!is_owned_result_photo("other_layer", owned_photo_name));
    assert!(!is_owned_result_photo("test_del_photo", "sample_drone.jpg"));

    // Buat layer dengan 1 foto owned dan 1 foto publik sample
    let layer_val = serde_json::json!({
        "layer_id": "test_del_photo",
        "layer_name": "Layer Uji Hapus Foto",
        "display_preference": {
            "type": "point",
            "color": "#E74C3C",
            "image_url": format!("/map_storage/images/result_image/{}", owned_photo_name)
        },
        "spatial_results": [
            {
                "source": "drone",
                "geometry": { "type": "Point", "coordinates": [116.85, -1.25] },
                "properties": { "tree_id": "T-1" },
                "image_url": "/sample_drone.jpg"
            }
        ]
    });
    fs::write(
        sr_dir.join("test_del_photo.json"),
        serde_json::to_string_pretty(&layer_val).unwrap(),
    )
    .unwrap();

    // Verifikasi count_spatial_result_dependents_in_dir
    let deps = count_spatial_result_dependents_in_dir(temp.path(), "test_del_photo").unwrap();
    assert_eq!(deps.photos_count, 1);

    // Lakukan delete
    let del_summary = delete_saved_spatial_result_in_dir(temp.path(), "test_del_photo").unwrap();
    assert!(del_summary);

    // Foto owned di images/result_image harus terhapus permanen
    assert!(
        !photos_dir.join(owned_photo_name).exists(),
        "Foto owned harus terhapus bersih"
    );

    // File layer harus terhapus
    assert!(!sr_dir.join("test_del_photo.json").exists());
}
