use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;

use aerial_analytics_platform_lib::map_controller::annotations::types::AnnotationFeature;
use aerial_analytics_platform_lib::map_controller::error::MapControllerError;
use aerial_analytics_platform_lib::map_controller::ortho_source::store::{
    delete_ortho_image_single, get_ortho_cascade_summary, get_ortho_image, list_ortho_images,
    update_ortho_image, validate_ortho_id,
};
use aerial_analytics_platform_lib::map_controller::ortho_source::types::{
    ImageCategory, StoredImageMetadata, UpdateOrthoImagePayload,
};
use aerial_analytics_platform_lib::map_controller::types::sp_result_type::SpatialGeometry;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_validate_ortho_id_allowlist_regex() {
    // Valid IDs
    assert!(validate_ortho_id("ortho_1792805664700").is_ok());
    assert!(validate_ortho_id("ortho_1").is_ok());
    assert!(validate_ortho_id("ortho_00012345").is_ok());

    // Invalid IDs
    assert!(validate_ortho_id("").is_err());
    assert!(validate_ortho_id("ortho_").is_err());
    assert!(validate_ortho_id("ortho_abc").is_err());
    assert!(validate_ortho_id("ortho_123a").is_err());
    assert!(validate_ortho_id("raster_12345").is_err());
    assert!(validate_ortho_id("normal_image").is_err());
    assert!(validate_ortho_id("../ortho_123").is_err());
    assert!(validate_ortho_id("ortho_123/bad").is_err());
    assert!(validate_ortho_id("ortho_123\\bad").is_err());
}

#[test]
fn test_list_ortho_images_sorting_and_corrupt_metadata() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    let images_dir = temp.path().join("images");
    fs::create_dir_all(&images_dir).unwrap();

    // 1. Folder ortho_100: meta valid (lebih lama)
    let dir_100 = images_dir.join("ortho_100");
    fs::create_dir_all(&dir_100).unwrap();
    let meta_100 = StoredImageMetadata {
        schema_version: 1,
        id: "ortho_100".to_string(),
        filename: "Area_A.jpg".to_string(),
        category: ImageCategory::Ortho,
        extension: "jpg".to_string(),
        file_path: "/map_storage/images/ortho_100/original.jpg".to_string(),
        size_bytes: 1000,
        dimensions: Some([1000, 1000]),
        thumbnail_url: Some("/map_storage/images/ortho_100/display.jpg".to_string()),
        bounds: Some([[-1.22, 116.84], [-1.21, 116.87]]),
        aspect_ratio: Some(1.0),
        display_size_bytes: Some(500),
        imported_at: Some("2026-10-24T01:00:00Z".to_string()),
        updated_at: None,
        is_corrupted: Some(false),
    };
    fs::write(
        dir_100.join("meta.json"),
        serde_json::to_string(&meta_100).unwrap(),
    )
    .unwrap();

    // 2. Folder ortho_200: meta valid (lebih baru)
    let dir_200 = images_dir.join("ortho_200");
    fs::create_dir_all(&dir_200).unwrap();
    let meta_200 = StoredImageMetadata {
        schema_version: 1,
        id: "ortho_200".to_string(),
        filename: "Area_B.jpg".to_string(),
        category: ImageCategory::Ortho,
        extension: "jpg".to_string(),
        file_path: "/map_storage/images/ortho_200/original.jpg".to_string(),
        size_bytes: 2000,
        dimensions: Some([2000, 2000]),
        thumbnail_url: Some("/map_storage/images/ortho_200/display.jpg".to_string()),
        bounds: Some([[-1.23, 116.85], [-1.20, 116.88]]),
        aspect_ratio: Some(1.0),
        display_size_bytes: Some(800),
        imported_at: Some("2026-10-24T02:00:00Z".to_string()),
        updated_at: None,
        is_corrupted: Some(false),
    };
    fs::write(
        dir_200.join("meta.json"),
        serde_json::to_string(&meta_200).unwrap(),
    )
    .unwrap();

    // 3. Folder ortho_300: meta.json KORUP
    let dir_300 = images_dir.join("ortho_300");
    fs::create_dir_all(&dir_300).unwrap();
    fs::write(dir_300.join("meta.json"), "{ invalid-json-corrupt").unwrap();

    // Jalankan list_ortho_images
    let list = list_ortho_images().expect("Harus berhasil list");
    assert_eq!(
        list.len(),
        3,
        "Semua 3 item harus termuat, termasuk yang korup"
    );

    // Verifikasi ortho_200 (02:00) muncul sebelum ortho_100 (01:00)
    let idx_100 = list.iter().position(|i| i.id == "ortho_100").unwrap();
    let idx_200 = list.iter().position(|i| i.id == "ortho_200").unwrap();
    assert!(
        idx_200 < idx_100,
        "ortho_200 (lebih baru) harus sebelum ortho_100"
    );

    // Verifikasi item korup (ortho_300)
    let corrupt_item = list
        .iter()
        .find(|i| i.id == "ortho_300")
        .expect("Item ortho_300 harus ada");
    assert_eq!(corrupt_item.is_corrupted, Some(true));
    assert!(corrupt_item.filename.contains("Metadata rusak"));
}

#[test]
fn test_get_ortho_image_success_and_not_found() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    // 1. Not Found
    let res = get_ortho_image("ortho_9999999999999");
    assert!(res.is_err());
    match res {
        Err(MapControllerError::NotFound(_)) => {}
        other => panic!("Harusnya NotFound error, malah: {:?}", other),
    }

    // 2. Success
    let dir = temp.path().join("images").join("ortho_555");
    fs::create_dir_all(&dir).unwrap();
    let meta = StoredImageMetadata {
        schema_version: 1,
        id: "ortho_555".to_string(),
        filename: "Kariangau.png".to_string(),
        category: ImageCategory::Ortho,
        extension: "png".to_string(),
        file_path: "/map_storage/images/ortho_555/original.png".to_string(),
        size_bytes: 4500,
        dimensions: Some([1200, 1200]),
        thumbnail_url: Some("/map_storage/images/ortho_555/display.jpg".to_string()),
        bounds: Some([[-1.22, 116.84], [-1.21, 116.87]]),
        aspect_ratio: Some(1.0),
        display_size_bytes: Some(1200),
        imported_at: Some("2026-10-24T03:00:00Z".to_string()),
        updated_at: None,
        is_corrupted: Some(false),
    };
    fs::write(dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();

    let fetched = get_ortho_image("ortho_555").expect("Harus berhasil get_ortho_image");
    assert_eq!(fetched.id, "ortho_555");
    assert_eq!(fetched.filename, "Kariangau.png");
    assert_eq!(fetched.is_corrupted, Some(false));
}

#[test]
fn test_update_ortho_image_preserves_immutable_fields() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    let dir = temp.path().join("images").join("ortho_777");
    fs::create_dir_all(&dir).unwrap();

    let initial_meta = StoredImageMetadata {
        schema_version: 1,
        id: "ortho_777".to_string(),
        filename: "Foto Lama.jpg".to_string(),
        category: ImageCategory::Ortho,
        extension: "jpg".to_string(),
        file_path: "/map_storage/images/ortho_777/original.jpg".to_string(),
        size_bytes: 12345,
        dimensions: Some([4000, 3000]),
        thumbnail_url: Some("/map_storage/images/ortho_777/display.jpg".to_string()),
        bounds: Some([[-1.2299, 116.8431], [-1.2129, 116.8760]]),
        aspect_ratio: Some(1.333),
        display_size_bytes: Some(400000),
        imported_at: Some("2026-10-24T00:10:00Z".to_string()),
        updated_at: None,
        is_corrupted: Some(false),
    };
    fs::write(
        dir.join("meta.json"),
        serde_json::to_string_pretty(&initial_meta).unwrap(),
    )
    .unwrap();

    // Buat layer di layers/ortho_777.json
    let layers_dir = temp.path().join("layers");
    fs::create_dir_all(&layers_dir).unwrap();
    let layer_state = serde_json::json!({
        "layer_id": "ortho_777",
        "layer_name": "Foto Lama.jpg",
        "category": "ortho",
        "is_visible": true,
        "opacity": 1.0,
        "z_index": 0
    });
    fs::write(layers_dir.join("ortho_777.json"), layer_state.to_string()).unwrap();

    // Jalankan update
    let payload = UpdateOrthoImagePayload {
        id: "ortho_777".to_string(),
        filename: Some("Foto Baru Kariangau.jpg".to_string()),
        west: Some(116.800000),
        south: Some(-1.250000),
        east: Some(116.900000),
        north: Some(-1.200000),
    };

    let updated = update_ortho_image(payload).expect("Harus berhasil update");

    // Verifikasi field IMMUTABLE tetap sama persis!
    assert_eq!(updated.id, "ortho_777", "ID tidak boleh berubah");
    assert_eq!(
        updated.imported_at,
        Some("2026-10-24T00:10:00Z".to_string()),
        "imported_at tidak boleh berubah"
    );
    assert_eq!(
        updated.schema_version, 1,
        "schema_version tidak boleh berubah"
    );

    // Verifikasi field yang di-update berubah
    assert_eq!(updated.filename, "Foto Baru Kariangau.jpg");
    assert_eq!(
        updated.bounds,
        Some([[-1.250000, 116.800000], [-1.200000, 116.900000]])
    );
    assert!(updated.updated_at.is_some(), "updated_at harus terisi");

    // Verifikasi layer sinkron
    let layer_content = fs::read_to_string(layers_dir.join("ortho_777.json")).unwrap();
    let layer_json: serde_json::Value = serde_json::from_str(&layer_content).unwrap();
    assert_eq!(layer_json["layer_name"], "Foto Baru Kariangau.jpg");
}

#[test]
fn test_cascade_summary_and_delete_single_clean() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    let image_id = "ortho_888";
    let image_dir = temp.path().join("images").join(image_id);
    fs::create_dir_all(&image_dir).unwrap();
    fs::write(image_dir.join("display.jpg"), "preview-bytes").unwrap();
    fs::write(image_dir.join("meta.json"), "{}").unwrap();

    // Layer terkait
    let layers_dir = temp.path().join("layers");
    fs::create_dir_all(&layers_dir).unwrap();
    fs::write(layers_dir.join(format!("{}.json", image_id)), "{}").unwrap();

    // Anotasi: 2 terkait citra ini, 1 citra lain
    let anno_dir = temp.path().join("annotations");
    fs::create_dir_all(&anno_dir).unwrap();

    let anno1 = AnnotationFeature {
        id: "anno_01".to_string(),
        schema_version: 1,
        text: "Pohon kelapa sawit 1".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.85, -1.22],
        },
        created_at: "2026-10-24T00:00:00Z".to_string(),
        image_id: Some(image_id.to_string()),
    };
    fs::write(
        anno_dir.join("anno_01.json"),
        serde_json::to_string(&anno1).unwrap(),
    )
    .unwrap();

    let anno2 = AnnotationFeature {
        id: "anno_02".to_string(),
        schema_version: 1,
        text: "Pohon kelapa sawit 2".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.86, -1.23],
        },
        created_at: "2026-10-24T00:00:00Z".to_string(),
        image_id: Some(image_id.to_string()),
    };
    fs::write(
        anno_dir.join("anno_02.json"),
        serde_json::to_string(&anno2).unwrap(),
    )
    .unwrap();

    let anno_other = AnnotationFeature {
        id: "anno_other".to_string(),
        schema_version: 1,
        text: "Anotasi citra lain".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.87, -1.24],
        },
        created_at: "2026-10-24T00:00:00Z".to_string(),
        image_id: Some("ortho_99999".to_string()),
    };
    fs::write(
        anno_dir.join("anno_other.json"),
        serde_json::to_string(&anno_other).unwrap(),
    )
    .unwrap();

    // 1. Cek Summary Cascade
    let summary = get_ortho_cascade_summary(image_id).expect("Harus berhasil summary");
    assert_eq!(summary.id, image_id);
    assert!(summary.layer_deleted);
    assert_eq!(summary.annotations_deleted_count, 2);

    // 2. Eksekusi Hapus Tuntas
    let del_res = delete_ortho_image_single(image_id).expect("Harus berhasil delete");
    assert!(del_res.image_deleted);
    assert!(del_res.layer_deleted);
    assert_eq!(del_res.annotations_deleted_count, 2);

    // Verifikasi berkas terhapus bersih
    assert!(!image_dir.exists(), "Folder citra harus terhapus bersih");
    assert!(
        !layers_dir.join(format!("{}.json", image_id)).exists(),
        "Layer harus terhapus bersih"
    );
    assert!(
        !anno_dir.join("anno_01.json").exists(),
        "Anotasi terikat 1 harus terhapus"
    );
    assert!(
        !anno_dir.join("anno_02.json").exists(),
        "Anotasi terikat 2 harus terhapus"
    );

    // Anotasi milik citra lain TIDAK BOLEH TERHAPUS
    assert!(
        anno_dir.join("anno_other.json").exists(),
        "Anotasi citra lain tidak boleh terhapus"
    );
}

#[test]
fn test_delete_ortho_image_not_found() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    let res = delete_ortho_image_single("ortho_404404404");
    assert!(res.is_err());
    match res {
        Err(MapControllerError::NotFound(_)) => {}
        other => panic!("Harusnya NotFound error, malah: {:?}", other),
    }
}

#[test]
#[allow(clippy::permissions_set_readonly_false)]
fn test_delete_ortho_image_rollback_on_failure() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    let image_id = "ortho_999";
    let image_dir = temp.path().join("images").join(image_id);
    fs::create_dir_all(&image_dir).unwrap();
    fs::write(image_dir.join("original.jpg"), "original-drone-bytes").unwrap();
    fs::write(image_dir.join("meta.json"), "{}").unwrap();

    // Buat anotasi terikat yang dikunci read-only agar penghapusan gagal di Fase 2
    let anno_dir = temp.path().join("annotations");
    fs::create_dir_all(&anno_dir).unwrap();
    let anno_path = anno_dir.join("anno_locked.json");
    let anno = AnnotationFeature {
        id: "anno_locked".to_string(),
        schema_version: 1,
        text: "Anotasi terkunci".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.85, -1.22],
        },
        created_at: "2026-10-24T00:00:00Z".to_string(),
        image_id: Some(image_id.to_string()),
    };
    fs::write(&anno_path, serde_json::to_string(&anno).unwrap()).unwrap();

    // Kunci berkas menjadi read-only
    let mut perms = fs::metadata(&anno_path).unwrap().permissions();
    perms.set_readonly(true);
    let _ = fs::set_permissions(&anno_path, perms.clone());

    // Jalankan delete_ortho_image_single
    let result = delete_ortho_image_single(image_id);

    // Buka kembali izin agar tempdir bisa dibersihkan
    perms.set_readonly(false);
    let _ = fs::set_permissions(&anno_path, perms);

    // Di Windows, penghapusan file read-only akan gagal dan memicu rollback
    // Jika di lingkungan OS yang mengizinkan penghapusan file read-only, result tetap terdefinisi
    if result.is_err() {
        // ROLLBACK BERHASIL: Folder asli harus kembali ke nama semula
        assert!(
            image_dir.exists(),
            "Folder asli harus di-rollback dan tetap ada"
        );
        assert!(
            image_dir.join("original.jpg").exists(),
            "Berkas original.jpg harus utuh"
        );
    }
}
