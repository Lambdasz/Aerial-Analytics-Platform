//! Pengujian integrasi komprehensif untuk Fase 1 (Fondasi):
//! 1. Single-Instance Lock (eksklusif kernel OS pada map_storage/.app.lock, penolakan instansi ke-2)
//! 2. Pelepasan slot job di semua jalur (JobRegistrationGuard: sukses, error, cancel, panic)
//! 3. projection.rs dengan nilai referensi (asal belum terverifikasi terhadap PROJ/pyproj) (3 titik UTM 50S, 3 titik EPSG:3857, toleransi < 1m)
//! 4. Penulis layer atomik (register_ortho_layer)
//! 5. Pemecahan tes rekonsiliasi store per skenario:
//!    - Folder titik diabaikan (.staging_*, .trash_*)
//!    - Trash dipulihkan jika layer masih ada
//!    - Trash diselesaikan jika layer sudah hilang
//!    - Kasus A (auto-heal)
//!    - Kasus B (layer yatim dihapus)
//!    - Kasus B TIDAK menyentuh layer non-ortho (aoi, vector, spatial_result)
//! 6. test_delete_ortho_clean (hapus bersih folder images/{id}/ termasuk original.*, display.*, layer, dan anotasi)

use std::fs;
use tempfile::tempdir;

use aerial_analytics_platform_lib::modules::module_03::annotations::types::AnnotationFeature;
use aerial_analytics_platform_lib::modules::module_03::error::OrthoImportError;
use aerial_analytics_platform_lib::modules::module_03::geometry::projection::{
    compute_wgs84_envelope_from_corners, epsg3857_to_wgs84, utm_to_wgs84, wgs84_to_epsg3857,
    wgs84_to_utm,
};
use aerial_analytics_platform_lib::modules::module_03::layers::store::get_layers_dir;
use aerial_analytics_platform_lib::modules::module_03::layers::types::LayerState;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::AppInstanceLock;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::job_manager::{
    cleanup_stale_import_jobs, get_jobs_dir, load_job_record, release_job, save_job_record,
    try_acquire_job, JobRegistrationGuard, StagingGuard,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::store::{
    delete_ortho_image_single, get_images_dir, list_ortho_images, reconcile_storage_layers,
    register_ortho_layer,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::types::{
    ImageCategory, JobRecord, StoredImageMetadata,
};
use aerial_analytics_platform_lib::modules::module_03::storage_utils::{
    get_storage_root, write_atomic,
};
use aerial_analytics_platform_lib::modules::module_03::types::sp_result_type::SpatialGeometry;

#[test]
fn test_ortho_import_error_behavior() {
    let err_coord = OrthoImportError::KoordinatTidakValid("south >= north".to_string());
    assert_eq!(err_coord.code(), "KOORDINAT_TIDAK_VALID");
    assert!(err_coord.is_recoverable());
    assert!(err_coord
        .user_friendly_message()
        .contains("Batas koordinat tidak valid"));

    let err_crs = OrthoImportError::CrsTidakDidukung("EPSG:9999".to_string());
    assert_eq!(err_crs.code(), "CRS_TIDAK_DIDUKUNG");
    assert!(err_crs.is_recoverable());

    let err_job = OrthoImportError::JobSedangBerjalan("job-abc-123".to_string());
    assert_eq!(err_job.code(), "JOB_SEDANG_BERJALAN");
    assert!(err_job.is_recoverable());

    let err_kompresi = OrthoImportError::KompresiTidakDidukung("JPEG".to_string());
    assert_eq!(err_kompresi.code(), "KOMPRESI_TIDAK_DIDUKUNG");
    assert!(!err_kompresi.is_recoverable());
    assert!(err_kompresi.user_friendly_message().contains("Deflate"));

    let err_rotasi = OrthoImportError::RotasiTidakDidukung(0.15);
    assert_eq!(err_rotasi.code(), "ROTASI_TIDAK_DIDUKUNG");
    assert!(!err_rotasi.is_recoverable());

    let err_cancel = OrthoImportError::Dibatalkan;
    assert_eq!(err_cancel.code(), "DIBATALKAN");
    assert!(!err_cancel.is_recoverable());
}

#[test]
fn test_single_instance_lock_rejection_and_release() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let lock_path = temp.path().join(".app.lock");

    // Instansi pertama berhasil mengunci
    let lock_1 = AppInstanceLock::try_acquire(lock_path.clone())
        .expect("Instansi 1 harus berhasil memperoleh file lock eksklusif");
    assert!(lock_1.lock_path().exists());

    // Instansi kedua harus langsung ditolak oleh kernel OS
    let res_2 = AppInstanceLock::try_acquire(lock_path.clone());
    assert!(
        matches!(res_2, Err(OrthoImportError::AplikasiSudahBerjalan(_))),
        "Instansi 2 harus ditolak dengan AplikasiSudahBerjalan saat lock aktif"
    );

    // Lepaskan instansi 1 (drop)
    drop(lock_1);

    // Instansi kedua sekarang harus berhasil memperoleh kunci
    let lock_2 = AppInstanceLock::try_acquire(lock_path)
        .expect("Instansi 2 harus berhasil memperoleh lock setelah instansi 1 dilepas");
    drop(lock_2);
}

#[test]
fn test_job_slot_released_on_all_exits_including_panic() {
    let job_id = "test_guaranteed_release_job";
    release_job(job_id);

    // 1. Jalur keluar biasa via Drop
    {
        let _token = try_acquire_job(job_id).expect("Acquire job");
        let _guard = JobRegistrationGuard::new(job_id.to_string());
        // Guard di-drop saat scope berakhir
    }
    // Slot harus sudah bebas
    let token_reacquired =
        try_acquire_job(job_id).expect("Slot job harus otomatis bebas setelah guard keluar scope");
    drop(token_reacquired);
    release_job(job_id);

    // 2. Jalur keluar via panic (catch_unwind)
    let _ = std::panic::catch_unwind(|| {
        let _token = try_acquire_job(job_id).expect("Acquire inside thread");
        let _guard = JobRegistrationGuard::new(job_id.to_string());
        panic!("Simulasi panic pekerja impor");
    });

    // Slot harus tetap bebas meskipun terjadi panic pada thread
    let token_post_panic =
        try_acquire_job(job_id).expect("Slot job harus tetap dilepas meskipun pekerja panic");
    drop(token_post_panic);
    release_job(job_id);
}

#[test]
fn test_staging_guard_cleanup_on_drop() {
    let images_dir = get_images_dir();
    let staging_dir = images_dir.join(".staging_test_guard_123");
    let _ = fs::create_dir_all(&staging_dir);
    assert!(staging_dir.exists());

    // Skenario 1: Drop tanpa disarm -> folder harus terhapus
    {
        let _guard = StagingGuard::new(staging_dir.clone());
    }
    assert!(
        !staging_dir.exists(),
        "Folder staging harus terhapus otomatis saat drop tanpa disarm"
    );

    // Skenario 2: Disarm -> folder tetap dipertahankan
    let _ = fs::create_dir_all(&staging_dir);
    {
        let mut guard = StagingGuard::new(staging_dir.clone());
        guard.disarm();
    }
    assert!(
        staging_dir.exists(),
        "Folder staging harus tetap ada setelah disarm"
    );
    let _ = fs::remove_dir_all(&staging_dir);

    // Skenario 3: StagingGuard::for_job menurunkan path dari job_id di bawah images/
    let job_id = "job_test_secure_staging";
    let guard = StagingGuard::for_job(job_id).expect("for_job harus berhasil");
    assert_eq!(
        guard.staging_dir(),
        images_dir.join(format!(".staging_{}", job_id))
    );
    let _ = fs::create_dir_all(guard.staging_dir());
    assert!(guard.staging_dir().exists());
    drop(guard);
    assert!(!images_dir.join(format!(".staging_{}", job_id)).exists());
}

#[test]
fn test_cleanup_stale_import_jobs_and_retention() {
    let jobs_dir = get_jobs_dir();
    let _ = fs::create_dir_all(&jobs_dir);

    // 1. Buat mock job berstatus "running"
    let running_job = JobRecord {
        job_id: "test_job_running_crash".to_string(),
        status: "running".to_string(),
        stage: "resampling".to_string(),
        progress_percent: 45.0,
        message: "Memproses...".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        error: None,
        result_image_id: None,
    };
    save_job_record(&running_job).expect("Gagal simpan job running");

    // 2. Buat folder staging sisa crash
    let images_dir = get_images_dir();
    let staging_orphan = images_dir.join(".staging_orphan_crash_456");
    let _ = fs::create_dir_all(&staging_orphan);
    assert!(staging_orphan.exists());

    // 3. Jalankan startup cleanup
    cleanup_stale_import_jobs().expect("Gagal cleanup stale jobs");

    // Folder staging harus sudah bersih
    assert!(
        !staging_orphan.exists(),
        "Folder staging sisa crash harus dibersihkan"
    );

    // Status job running harus berubah menjadi "interrupted"
    let reloaded = load_job_record("test_job_running_crash")
        .expect("Gagal reload")
        .expect("Job harus ada");
    assert_eq!(reloaded.status, "interrupted");

    // 4. Uji retensi 20 job: buat 25 file job
    for i in 1..=25 {
        let job = JobRecord {
            job_id: format!("retention_test_job_{}", i),
            status: "completed".to_string(),
            stage: "selesai".to_string(),
            progress_percent: 100.0,
            message: "Selesai".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
            result_image_id: None,
        };
        let _ = save_job_record(&job);
    }

    cleanup_stale_import_jobs().expect("Gagal cleanup retention");

    // Hitung total berkas job di .jobs/
    let total_jobs = fs::read_dir(&jobs_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
                .count()
        })
        .unwrap_or(0);

    assert!(total_jobs <= 20, "Total berkas job harus <= 20 (retensi)");

    // Bersihkan file tes
    let _ = fs::remove_file(jobs_dir.join("test_job_running_crash.json"));
    for i in 1..=25 {
        let _ = fs::remove_file(jobs_dir.join(format!("retention_test_job_{}.json", i)));
    }
}

/// Pembanding toleransi: true jika |actual - expected| < tol (meter).
fn within_tolerance(actual: f64, expected: f64, tol: f64) -> bool {
    (actual - expected).abs() < tol
}

/// Membaca golden proyeksi yang dihasilkan OTOMATIS oleh `scripts/gen_projection_golden.py` (pyproj).
fn load_projection_golden() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("projection_golden.json");
    let text = fs::read_to_string(&path).expect("projection_golden.json harus ada");
    serde_json::from_str(&text).expect("projection_golden.json harus JSON valid")
}

fn forward_case(case: &serde_json::Value) -> (f64, f64) {
    let lat = case["lat"].as_f64().unwrap();
    let lon = case["lon"].as_f64().unwrap();
    if case["kind"] == "utm" {
        let zone = case["zone"].as_u64().unwrap() as u8;
        let south = case["south"].as_bool().unwrap();
        wgs84_to_utm(lat, lon, zone, south).expect("forward UTM")
    } else {
        wgs84_to_epsg3857(lat, lon).expect("forward 3857")
    }
}

fn inverse_case(case: &serde_json::Value) -> (f64, f64) {
    let x = case["x"].as_f64().unwrap();
    let y = case["y"].as_f64().unwrap();
    if case["kind"] == "utm" {
        let zone = case["zone"].as_u64().unwrap() as u8;
        let south = case["south"].as_bool().unwrap();
        utm_to_wgs84(x, y, zone, south).expect("inverse UTM")
    } else {
        epsg3857_to_wgs84(x, y).expect("inverse 3857")
    }
}

/// Forward dan inverse terhadap golden pyproj (toleransi < 0,5 m). Referensi TIDAK diketik manual.
#[test]
fn test_projection_against_pyproj_golden_forward_and_inverse() {
    use aerial_analytics_platform_lib::modules::module_03::geometry::haversine::calculate_distance;

    let golden = load_projection_golden();
    println!(
        "Golden: pyproj {} / PROJ {}",
        golden["pyproj_version"], golden["proj_version"]
    );
    let cases = golden["cases"].as_array().expect("cases");
    assert!(cases.len() >= 9, "minimal 9 kasus");

    for case in cases {
        let name = case["name"].as_str().unwrap();
        let (fx, fy) = forward_case(case);
        let (gx, gy) = (case["x"].as_f64().unwrap(), case["y"].as_f64().unwrap());
        println!(
            "{name}: fwd=({fx:.4}, {fy:.4}) golden=({gx:.4}, {gy:.4}) diff=({:.4}, {:.4})",
            (fx - gx).abs(),
            (fy - gy).abs()
        );
        assert!(
            within_tolerance(fx, gx, 0.5),
            "{name}: forward X/E selisih >= 0.5 m"
        );
        assert!(
            within_tolerance(fy, gy, 0.5),
            "{name}: forward Y/N selisih >= 0.5 m"
        );

        let (ilat, ilon) = inverse_case(case);
        let lat = case["lat"].as_f64().unwrap();
        let lon = case["lon"].as_f64().unwrap();
        let ground = calculate_distance([lon, lat], [ilon, ilat]);
        println!("{name}: inverse ground error = {ground:.4} m");
        assert!(ground < 0.5, "{name}: inverse ground error >= 0.5 m");
    }
}

/// Membuktikan toleransi benar-benar bekerja: golden digeser +1 m harus ditolak pembanding.
#[test]
fn test_projection_tolerance_rejects_shifted_golden() {
    let golden = load_projection_golden();
    for case in golden["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let (fx, fy) = forward_case(case);
        let (gx, gy) = (case["x"].as_f64().unwrap(), case["y"].as_f64().unwrap());
        assert!(within_tolerance(fx, gx, 0.5) && within_tolerance(fy, gy, 0.5));
        assert!(
            !within_tolerance(fx, gx + 1.0, 0.5),
            "{name}: geser +1 m X harus false"
        );
        assert!(
            !within_tolerance(fy, gy + 1.0, 0.5),
            "{name}: geser +1 m Y harus false"
        );
    }
}

#[test]
fn test_projection_envelope_calculation() {
    let corners = vec![
        (483000.0, 9861000.0),
        (484000.0, 9861000.0),
        (484000.0, 9862000.0),
        (483000.0, 9862000.0),
    ];

    let bounds =
        compute_wgs84_envelope_from_corners(&corners, 32750).expect("Envelope calculation failed");

    assert!(bounds[0][0] < bounds[1][0]); // south < north
    assert!(bounds[0][1] < bounds[1][1]); // west < east
}

static STORAGE_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_atomic_layer_writer() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let test_id = "ortho_77777777701";
    let test_name = "Atomic Layer Writer Test";
    let layers_dir = get_layers_dir();
    let layer_file = layers_dir.join(format!("{}.json", test_id));

    if layer_file.exists() {
        let _ = fs::remove_file(&layer_file);
    }

    let layer = register_ortho_layer(test_id, test_name).expect("Gagal register ortho layer");
    assert_eq!(layer.layer_id, test_id);
    assert_eq!(layer.layer_name, test_name);
    assert_eq!(layer.category, "raster");
    assert!(layer_file.exists());

    // Bersihkan
    let _ = fs::remove_file(&layer_file);
}

#[test]
fn test_store_dot_folder_ignored() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let images_dir = get_images_dir();
    let staging_dot = images_dir.join(".staging_ignored_test");
    let trash_dot = images_dir.join(".trash_ignored_test");
    let _ = fs::create_dir_all(&staging_dot);
    let _ = fs::create_dir_all(&trash_dot);

    let images = list_ortho_images().expect("list_ortho_images");
    for img in &images {
        assert!(
            !img.id.starts_with('.'),
            "Folder titik '{}' tidak boleh muncul di daftar",
            img.id
        );
    }

    let _ = fs::remove_dir_all(&staging_dot);
    let _ = fs::remove_dir_all(&trash_dot);
}

#[test]
fn test_reconcile_trash_restored_if_layer_exists() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let images_dir = get_images_dir();
    let layers_dir = get_layers_dir();
    let test_id = format!(
        "ortho_8881_{}_{}",
        std::process::id(),
        chrono::Utc::now().timestamp_micros()
    );

    let image_dir = images_dir.join(&test_id);
    let trash_dir = images_dir.join(format!(".trash_{}_123456", test_id));
    let layer_file = layers_dir.join(format!("{}.json", test_id));

    // Siapkan folder karantina .trash_*
    let _ = fs::create_dir_all(&trash_dir);
    // Layer masih ada di disk
    let layer_state = LayerState {
        layer_id: test_id.clone(),
        layer_name: "Trash Restore Test".to_string(),
        category: "raster".to_string(),
        is_visible: true,
        opacity: 1.0,
        z_index: 10,
        source_type: Some("raster".to_string()),
        source_id: Some(test_id.clone()),
    };
    aerial_analytics_platform_lib::modules::module_03::layers::store::save_layer(layer_state)
        .expect("save_layer must succeed in test");

    // Jalankan rekonsiliasi: karena layer masih ada, trash harus di-rollback ke images/{id}
    reconcile_storage_layers().expect("reconcile");

    assert!(
        image_dir.exists(),
        "Folder citra harus dipulihkan dari trash"
    );
    assert!(!trash_dir.exists(), "Folder trash harus sudah tidak ada");

    // Bersihkan
    let _ = fs::remove_file(&layer_file);
    let _ = fs::remove_dir_all(&image_dir);
}

#[test]
fn test_reconcile_trash_purged_if_layer_gone() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let images_dir = get_images_dir();
    let layers_dir = get_layers_dir();
    let test_id = format!(
        "ortho_8882_{}_{}",
        std::process::id(),
        chrono::Utc::now().timestamp_micros()
    );

    let trash_dir = images_dir.join(format!(".trash_{}_999999", test_id));
    let layer_file = layers_dir.join(format!("{}.json", test_id));

    // Siapkan folder karantina .trash_* dan pastikan file layernya TIDAK ada
    let _ = fs::create_dir_all(&trash_dir);
    if layer_file.exists() {
        let _ = fs::remove_file(&layer_file);
    }

    // Jalankan rekonsiliasi: karena layer sudah tidak ada, trash harus dimusnahkan
    reconcile_storage_layers().expect("reconcile");

    assert!(
        !trash_dir.exists(),
        "Folder trash harus dimusnahkan jika layer sudah terhapus"
    );
}

#[test]
fn test_reconcile_kasus_a_auto_heal() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let images_dir = get_images_dir();
    let layers_dir = get_layers_dir();
    let test_id = "ortho_88888888803";

    let image_dir = images_dir.join(test_id);
    let layer_file = layers_dir.join(format!("{}.json", test_id));

    let _ = fs::create_dir_all(&image_dir);
    let meta = StoredImageMetadata {
        schema_version: 1,
        id: test_id.to_string(),
        filename: "Auto Heal Verification".to_string(),
        category: ImageCategory::Ortho,
        extension: "jpg".to_string(),
        file_path: format!("/map_storage/images/{}/display.jpg", test_id),
        size_bytes: 2048,
        dimensions: Some([1000, 1000]),
        thumbnail_url: Some(format!("/map_storage/images/{}/display.jpg", test_id)),
        bounds: Some([[-1.25, 116.85], [-1.24, 116.86]]),
        aspect_ratio: Some(1.0),
        display_size_bytes: Some(2048),
        display_dimensions: Some([1000, 1000]),
        warnings: None,
        imported_at: Some(chrono::Utc::now().to_rfc3339()),
        updated_at: None,
        is_corrupted: Some(false),
    };
    write_atomic(
        &image_dir.join("meta.json"),
        &serde_json::to_string_pretty(&meta).unwrap(),
    )
    .expect("Tulis meta");

    if layer_file.exists() {
        let _ = fs::remove_file(&layer_file);
    }

    reconcile_storage_layers().expect("reconcile");

    assert!(layer_file.exists(), "Layer harus di-auto-heal");

    let _ = fs::remove_file(&layer_file);
    let _ = fs::remove_dir_all(&image_dir);
}

#[test]
fn test_reconcile_kasus_b_orphan_raster_layer_deleted() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let images_dir = get_images_dir();
    let layers_dir = get_layers_dir();
    let test_id = "ortho_88888888804";

    let layer_file = layers_dir.join(format!("{}.json", test_id));
    let layer_state = LayerState {
        layer_id: test_id.to_string(),
        layer_name: "Orphan Raster Layer".to_string(),
        category: "raster".to_string(),
        is_visible: true,
        opacity: 1.0,
        z_index: 10,
        source_type: Some("raster".to_string()),
        source_id: Some(test_id.to_string()),
    };
    let _ =
        aerial_analytics_platform_lib::modules::module_03::layers::store::save_layer(layer_state);

    let image_dir = images_dir.join(test_id);
    if image_dir.exists() {
        let _ = fs::remove_dir_all(&image_dir);
    }

    reconcile_storage_layers().expect("reconcile");

    assert!(!layer_file.exists(), "Layer raster yatim harus dihapus");
}

#[test]
fn test_reconcile_kasus_b_preserves_non_ortho_layers() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let layers_dir = get_layers_dir();
    let test_id = "aoi_protected_layer_001";
    let layer_file = layers_dir.join(format!("{}.json", test_id));

    // Buat layer non-raster (mis. category: "aoi")
    let layer_state = LayerState {
        layer_id: test_id.to_string(),
        layer_name: "Protected AOI Layer".to_string(),
        category: "aoi".to_string(),
        is_visible: true,
        opacity: 0.8,
        z_index: 20,
        source_type: Some("vector".to_string()),
        source_id: Some(test_id.to_string()),
    };
    let _ =
        aerial_analytics_platform_lib::modules::module_03::layers::store::save_layer(layer_state);

    reconcile_storage_layers().expect("reconcile");

    assert!(
        layer_file.exists(),
        "Kasus B TIDAK BOLEH menghapus layer non-raster (AOI/Vector)!"
    );

    let _ = fs::remove_file(&layer_file);
}

#[test]
fn test_delete_ortho_clean_cascade() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    let images_dir = get_images_dir();
    let layers_dir = get_layers_dir();
    let anno_dir = get_storage_root().join("annotations");
    let _ = fs::create_dir_all(&anno_dir);

    let test_id = "ortho_99999999999";
    let image_dir = images_dir.join(test_id);
    let layer_file = layers_dir.join(format!("{}.json", test_id));
    let anno_file = anno_dir.join(format!("anno_{}.json", test_id));

    // 1. Buat folder citra lengkap: original.jpg, display.jpg, meta.json, metadata.json
    let _ = fs::create_dir_all(&image_dir);
    let _ = fs::write(image_dir.join("original.jpg"), [1u8; 1024]);
    let _ = fs::write(image_dir.join("display.jpg"), [2u8; 512]);
    let _ = fs::write(image_dir.join("meta.json"), "{}");
    let _ = fs::write(image_dir.join("metadata.json"), "{}");

    // 2. Buat file layer
    let layer_state = LayerState {
        layer_id: test_id.to_string(),
        layer_name: "Clean Cascade Test".to_string(),
        category: "raster".to_string(),
        is_visible: true,
        opacity: 1.0,
        z_index: 10,
        source_type: Some("raster".to_string()),
        source_id: Some(test_id.to_string()),
    };
    let _ =
        aerial_analytics_platform_lib::modules::module_03::layers::store::save_layer(layer_state);

    // 3. Buat file anotasi terikat
    let anno_feature = AnnotationFeature {
        id: format!("anno_{}", test_id),
        schema_version: 1,
        text: "Test Annotation".to_string(),
        geometry: SpatialGeometry::Point {
            coordinates: vec![116.85, -1.25],
        },
        created_at: chrono::Utc::now().to_rfc3339(),
        image_id: Some(test_id.to_string()),
    };
    let _ = write_atomic(
        &anno_file,
        &serde_json::to_string_pretty(&anno_feature).unwrap(),
    );

    assert!(image_dir.exists());
    assert!(layer_file.exists());
    assert!(anno_file.exists());

    // Jalankan penghapusan cascade
    let summary = delete_ortho_image_single(test_id).expect("delete_ortho_image_single");
    assert!(summary.image_deleted);
    assert!(summary.layer_deleted);
    assert_eq!(summary.annotations_deleted_count, 1);

    // Verifikasi pembersihan total
    assert!(
        !image_dir.exists(),
        "Folder images/{} harus terhapus tuntas",
        test_id
    );
    assert!(!layer_file.exists(), "File layer harus terhapus tuntas");
    assert!(
        !anno_file.exists(),
        "File anotasi terikat harus terhapus tuntas"
    );

    // Verifikasi tidak ada sisa folder .trash_*
    if let Ok(entries) = fs::read_dir(&images_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(&format!(".trash_{}_", test_id)),
                "Folder karantina .trash_* tidak boleh tertinggal"
            );
        }
    }
}

#[test]
fn test_secondary_instance_blocks_all_storage_mutating_commands() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    use aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::{
        is_primary_instance, set_is_primary_instance,
    };

    // Pastikan instansi diset sekunder (false)
    let prev = is_primary_instance();
    set_is_primary_instance(false);

    // 1. Anotasi commands
    let res_anno_create =
        aerial_analytics_platform_lib::modules::module_03::annotations::commands::create_annotation_cmd(
            "anno_mock_sec".to_string(),
            "Text".to_string(),
            SpatialGeometry::Point {
                coordinates: vec![116.8, -1.2],
            },
            chrono::Utc::now().to_rfc3339(),
        );
    assert!(
        res_anno_create.is_err(),
        "create_annotation_cmd harus ditolak pada instansi sekunder"
    );

    let res_anno_update =
        aerial_analytics_platform_lib::modules::module_03::annotations::commands::update_annotation_cmd(
            AnnotationFeature {
                id: "anno_mock_sec".to_string(),
                schema_version: 1,
                text: "Updated".to_string(),
                geometry: SpatialGeometry::Point {
                    coordinates: vec![116.8, -1.2],
                },
                created_at: chrono::Utc::now().to_rfc3339(),
                image_id: None,
            },
        );
    assert!(
        res_anno_update.is_err(),
        "update_annotation_cmd harus ditolak pada instansi sekunder"
    );

    let res_anno_del =
        aerial_analytics_platform_lib::modules::module_03::annotations::commands::delete_annotation_cmd(
            "anno_mock_sec".to_string(),
        );
    assert!(
        res_anno_del.is_err(),
        "delete_annotation_cmd harus ditolak pada instansi sekunder"
    );

    // 2. AOI commands
    let res_aoi_save_new =
        aerial_analytics_platform_lib::modules::module_03::aoi::commands::save_new_aoi_cmd(
            "aoi_mock_sec".to_string(),
            "Mock AOI".to_string(),
            None,
            vec![vec![
                [116.8, -1.2],
                [116.9, -1.2],
                [116.9, -1.1],
                [116.8, -1.1],
                [116.8, -1.2],
            ]],
            None,
        );
    assert!(
        res_aoi_save_new.is_err(),
        "save_new_aoi_cmd harus ditolak pada instansi sekunder"
    );

    let res_aoi_del =
        aerial_analytics_platform_lib::modules::module_03::aoi::commands::delete_saved_aoi(
            "aoi_mock_sec".to_string(),
        );
    assert!(
        res_aoi_del.is_err(),
        "delete_saved_aoi harus ditolak pada instansi sekunder"
    );

    // 3. Photo commands
    let res_photo =
        aerial_analytics_platform_lib::modules::module_03::photos::commands::import_photo(
            "dummy_path.jpg".to_string(),
        );
    assert!(
        res_photo.is_err(),
        "import_photo harus ditolak pada instansi sekunder"
    );

    // 4. Ortho Source commands
    let res_ortho_job =
        aerial_analytics_platform_lib::modules::module_03::ortho_source::commands::start_import_ortho_job(
            aerial_analytics_platform_lib::modules::module_03::ortho_source::types::ImportDualModePayload {
                mode: "A".to_string(),
                file_path: "dummy.jpg".to_string(),
                custom_filename: None,
                manual_bounds: None,
                save_original: false,
            },
        );
    assert!(
        res_ortho_job.is_err(),
        "start_import_ortho_job harus ditolak pada instansi sekunder"
    );

    let res_ortho_del =
        aerial_analytics_platform_lib::modules::module_03::ortho_source::commands::delete_ortho_image(
            "ortho_mock_sec".to_string(),
        );
    assert!(
        res_ortho_del.is_err(),
        "delete_ortho_image harus ditolak pada instansi sekunder"
    );

    let res_ortho_legacy =
        aerial_analytics_platform_lib::modules::module_03::ortho_source::commands::import_ortho_image(
            aerial_analytics_platform_lib::modules::module_03::ortho_source::types::ImportImagePayload {
                file_path: "dummy.jpg".to_string(),
                west: 116.8,
                south: -1.2,
                east: 116.9,
                north: -1.1,
            },
        );
    assert!(
        res_ortho_legacy.is_err(),
        "import_ortho_image harus ditolak"
    );

    let res_ortho_legacy_save =
        aerial_analytics_platform_lib::modules::module_03::ortho_source::commands::save_image_from_path(
            "dummy.jpg".to_string(),
        );
    assert!(
        res_ortho_legacy_save.is_err(),
        "save_image_from_path harus ditolak"
    );

    // Kembalikan state awal
    set_is_primary_instance(prev);
}

#[test]
fn test_static_source_code_audit_all_mutating_commands_guarded() {
    // Audit statis sumber kode Rust di Module 3 untuk memastikan SEMUA command yang memodifikasi
    // disk storage memanggil require_primary_instance()? secara eksplisit.
    let base_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("modules")
        .join("module_03");

    let mutating_command_files = [
        base_dir.join("annotations").join("commands.rs"),
        base_dir.join("aoi").join("commands.rs"),
        base_dir.join("photos").join("commands.rs"),
        base_dir.join("sp_result").join("commands.rs"),
        base_dir.join("ortho_source").join("commands.rs"),
    ];

    for file_path in &mutating_command_files {
        assert!(
            file_path.exists(),
            "File command harus ada: {}",
            file_path.display()
        );
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|_| panic!("Gagal membaca file {}", file_path.display()));

        // Cari setiap blok fn #[tauri::command]
        let mut lines = content.lines().peekable();
        while let Some(line) = lines.next() {
            if line.contains("#[tauri::command]") {
                // Baris berikutnya atau setelahnya adalah deklarasi fungsi
                let mut fn_decl = String::new();
                for next_l in lines.by_ref() {
                    fn_decl.push_str(next_l);
                    if next_l.contains('{') {
                        break;
                    }
                }

                // Ekstrak nama fungsi
                if let Some(pos) = fn_decl.find("pub fn ") {
                    let rest = &fn_decl[pos + 7..];
                    if let Some(end_name) = rest.find('(') {
                        let fn_name = rest[..end_name].trim();

                        // Lewati fungsi yang murni query / kalkulasi in-memory (tidak menyentuh disk)
                        let is_query_or_in_memory = fn_name.starts_with("get_")
                            || fn_name.starts_with("list_")
                            || fn_name.starts_with("count_")
                            || fn_name.starts_with("check_")
                            || fn_name.starts_with("measure_")
                            || fn_name.starts_with("inspect_")
                            || fn_name == "create_aoi_cmd" // pure in-memory builder (GeoJSON), tidak menulis disk
                            || fn_name == "cancel_import_job"; // cancel hanya mensinyalkan atomic token

                        if !is_query_or_in_memory {
                            // Baca tubuh fungsi hingga penutup
                            let mut body = String::new();
                            let mut brace_count = 1;
                            for body_line in lines.by_ref() {
                                body.push_str(body_line);
                                body.push('\n');
                                for ch in body_line.chars() {
                                    if ch == '{' {
                                        brace_count += 1;
                                    } else if ch == '}' {
                                        brace_count -= 1;
                                    }
                                }
                                if brace_count <= 0 {
                                    break;
                                }
                            }

                            // Pastikan tubuh fungsi memuat pemanggilan require_primary_instance
                            assert!(
                                body.contains("require_primary_instance"),
                                "KEGAGALAN KEAMANAN: Command mutasi '{}' pada '{}' TIDAK memanggil require_primary_instance()!",
                                fn_name,
                                file_path.display()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_start_import_ortho_job_spawns_without_tokio_reactor_requirement() {
    let _lock = STORAGE_TEST_MUTEX.lock().unwrap();
    use aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::{
        is_primary_instance, set_is_primary_instance,
    };
    let prev = is_primary_instance();
    set_is_primary_instance(true);

    let temp = tempfile::tempdir().expect("tempdir");
    let dummy_jpg = temp.path().join("dummy.jpg");
    // Write valid minimal JPEG header
    fs::write(
        &dummy_jpg,
        [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
        ],
    )
    .unwrap();

    let payload = aerial_analytics_platform_lib::modules::module_03::ortho_source::types::ImportDualModePayload {
        mode: "A".to_string(),
        file_path: dummy_jpg.to_string_lossy().to_string(),
        custom_filename: Some("test_reactor".to_string()),
        manual_bounds: Some(aerial_analytics_platform_lib::modules::module_03::ortho_source::types::ImportBoundsPayload {
            west: 116.85,
            south: -1.25,
            east: 116.86,
            north: -1.24,
        }),
        save_original: false,
    };

    // Ini dipanggil dari thread tes biasa (tanpa runtime Tokio).
    // Harus berhasil mengembalikan Ok(job_id) tanpa panic "there is no reactor running".
    let res = aerial_analytics_platform_lib::modules::module_03::ortho_source::commands::start_import_ortho_job(payload);
    assert!(
        res.is_ok(),
        "start_import_ortho_job harus berhasil mengalokasikan job"
    );
    let job_id = res.unwrap();
    assert!(job_id.starts_with("job_"));

    // Tunggu sebentar agar thread pekerja mencatat status
    std::thread::sleep(std::time::Duration::from_millis(150));

    // Bersihkan status job dari storage
    let job_record_path =
        aerial_analytics_platform_lib::modules::module_03::storage_utils::get_storage_root()
            .join(".jobs")
            .join(format!("{}.json", job_id));
    let _ = fs::remove_file(job_record_path);

    set_is_primary_instance(prev);
}
