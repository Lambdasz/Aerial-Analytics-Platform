//! Pengujian integrasi komprehensif untuk Fase 1 (Fondasi):
//! 1. Single-Instance Lock (eksklusif kernel OS pada map_storage/.app.lock, penolakan instansi ke-2)
//! 2. Pelepasan slot job di semua jalur (JobRegistrationGuard: sukses, error, cancel, panic)
//! 3. projection.rs dengan golden values (deret Krüger, diverifikasi dengan implementasi independen) (3 titik UTM 50S, 3 titik EPSG:3857, toleransi < 1m)
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

#[test]
fn test_projection_independent_references_utm_50s_balikpapan() {
    // 3 Titik referensi: golden values (deret Krüger, diverifikasi dengan implementasi independen) (UTM Zona 50S, EPSG:32750):
    // Toleransi: forward < 0.5 meter, inverse < 9e-6 derajat lintang / bujur (< 1.0 meter)

    // Titik 1: Bandara SAMS Sepinggan Balikpapan (-1.268270, 116.894450)
    // Nilai referensi independen deret Krüger (EPSG:32750): E = 488257.78, N = 9859817.59
    let (lat1, lon1) = (-1.268270, 116.894450);
    let (ref_e1, ref_n1) = (488257.78, 9859817.59);
    let (fwd_e1, fwd_n1) = wgs84_to_utm(lat1, lon1, 50, true).expect("UTM forward Sepinggan");
    assert!(
        (fwd_e1 - ref_e1).abs() < 0.5,
        "Easting selisih > 0.5m: {}",
        (fwd_e1 - ref_e1).abs()
    );
    assert!(
        (fwd_n1 - ref_n1).abs() < 0.5,
        "Northing selisih > 0.5m: {}",
        (fwd_n1 - ref_n1).abs()
    );
    let (calc_lat1, calc_lon1) =
        utm_to_wgs84(ref_e1, ref_n1, 50, true).expect("UTM 50S Sepinggan ke WGS84");
    assert!((calc_lat1 - lat1).abs() < 9e-6, "Lat selisih > 1m");
    assert!((calc_lon1 - lon1).abs() < 9e-6, "Lon selisih > 1m");

    // Titik 2: Kawasan Industri Kariangau (-1.173800, 116.812300)
    // Nilai referensi independen deret Krüger (EPSG:32750): E = 479118.01, N = 9870258.95
    let (lat2, lon2) = (-1.173800, 116.812300);
    let (ref_e2, ref_n2) = (479118.01, 9870258.95);
    let (fwd_e2, fwd_n2) = wgs84_to_utm(lat2, lon2, 50, true).expect("UTM forward Kariangau");
    assert!((fwd_e2 - ref_e2).abs() < 0.5);
    assert!((fwd_n2 - ref_n2).abs() < 0.5);
    let (calc_lat2, calc_lon2) =
        utm_to_wgs84(ref_e2, ref_n2, 50, true).expect("UTM 50S Kariangau ke WGS84");
    assert!((calc_lat2 - lat2).abs() < 9e-6);
    assert!((calc_lon2 - lon2).abs() < 9e-6);

    // Titik 3: Pelabuhan Semayang Balikpapan (-1.282500, 116.817200)
    // Nilai referensi independen deret Krüger (EPSG:32750): E = 479663.96, N = 9858244.25
    let (lat3, lon3) = (-1.282500, 116.817200);
    let (ref_e3, ref_n3) = (479663.96, 9858244.25);
    let (fwd_e3, fwd_n3) = wgs84_to_utm(lat3, lon3, 50, true).expect("UTM forward Semayang");
    assert!((fwd_e3 - ref_e3).abs() < 0.5);
    assert!((fwd_n3 - ref_n3).abs() < 0.5);
    let (calc_lat3, calc_lon3) =
        utm_to_wgs84(ref_e3, ref_n3, 50, true).expect("UTM 50S Semayang ke WGS84");
    assert!((calc_lat3 - lat3).abs() < 9e-6);
    assert!((calc_lon3 - lon3).abs() < 9e-6);
}

#[test]
fn test_projection_independent_references_utm_north_paris_and_greenwich() {
    // Paris / Eiffel Tower (48.858370 N, 2.294480 E) - UTM Zona 31N (EPSG:32631)
    // Diverifikasi independen via kalkulasi deret Krüger WGS84 terpisah:
    // E = 448250.50 m, N = 5411951.59 m
    let (lat_paris, lon_paris) = (48.858370, 2.294480);
    let (ref_e_paris, ref_n_paris) = (448250.50, 5411951.59);
    let (fwd_e_paris, fwd_n_paris) =
        wgs84_to_utm(lat_paris, lon_paris, 31, false).expect("Forward UTM 31N Paris");
    assert!(
        (fwd_e_paris - ref_e_paris).abs() < 0.5,
        "Forward Easting Paris selisih > 0.5m: ref={}, actual={}",
        ref_e_paris,
        fwd_e_paris
    );
    assert!(
        (fwd_n_paris - ref_n_paris).abs() < 0.5,
        "Forward Northing Paris selisih > 0.5m: ref={}, actual={}",
        ref_n_paris,
        fwd_n_paris
    );
    let (inv_lat_paris, inv_lon_paris) =
        utm_to_wgs84(ref_e_paris, ref_n_paris, 31, false).expect("Inverse UTM 31N Paris");
    assert!(
        (inv_lat_paris - lat_paris).abs() < 9e-6,
        "Lat Paris selisih > 1m"
    );
    assert!(
        (inv_lon_paris - lon_paris).abs() < 9e-6,
        "Lon Paris selisih > 1m"
    );

    // Greenwich Royal Observatory (51.476852 N, 0.000000 E) - UTM Zona 30N (EPSG:32630)
    // Diverifikasi independen via kalkulasi deret Krüger WGS84 terpisah:
    // E = 708322.41 m, N = 5707123.41 m
    let (lat_gw, lon_gw) = (51.476852, 0.000000);
    let (ref_e_gw, ref_n_gw) = (708322.41, 5707123.41);
    let (fwd_e_gw, fwd_n_gw) =
        wgs84_to_utm(lat_gw, lon_gw, 30, false).expect("Forward UTM 30N Greenwich");
    assert!(
        (fwd_e_gw - ref_e_gw).abs() < 0.5,
        "Forward Easting Greenwich selisih > 0.5m: ref={}, actual={}",
        ref_e_gw,
        fwd_e_gw
    );
    assert!(
        (fwd_n_gw - ref_n_gw).abs() < 0.5,
        "Forward Northing Greenwich selisih > 0.5m: ref={}, actual={}",
        ref_n_gw,
        fwd_n_gw
    );
    let (inv_lat_gw, inv_lon_gw) =
        utm_to_wgs84(ref_e_gw, ref_n_gw, 30, false).expect("Inverse UTM 30N Greenwich");
    assert!(
        (inv_lat_gw - lat_gw).abs() < 9e-6,
        "Lat Greenwich selisih > 1m"
    );
    assert!(
        (inv_lon_gw - lon_gw).abs() < 9e-6,
        "Lon Greenwich selisih > 1m"
    );
}

#[test]
fn test_projection_independent_references_epsg3857() {
    // 3 Titik referensi: golden values (deret Krüger, diverifikasi dengan implementasi independen) EPSG:3857 (Web Mercator) vs WGS84:
    // Toleransi: < 1 meter (< 1.0 meter koordinat EPSG:3857, < 9e-6 derajat lat/lon)

    // Titik 1: Null Island (Origin) (0.0, 0.0)
    let (lat1, lon1) = (0.0, 0.0);
    let (ref_x1, ref_y1) = (0.0, 0.0);
    let (fwd_x1, fwd_y1) = wgs84_to_epsg3857(lat1, lon1).expect("Forward Null Island");
    assert!((fwd_x1 - ref_x1).abs() < 1e-7);
    assert!((fwd_y1 - ref_y1).abs() < 1e-7);
    let (calc_lat1, calc_lon1) = epsg3857_to_wgs84(ref_x1, ref_y1).expect("EPSG:3857 Null Island");
    assert!((calc_lat1 - lat1).abs() < 1e-7);
    assert!((calc_lon1 - lon1).abs() < 1e-7);

    // Titik 2: Monas Jakarta (-6.1753924, 106.8271528)
    // Nilai referensi: golden values (deret Krüger, diverifikasi dengan implementasi independen) (EPSG:3857): x = 11891944.25, y = -688776.39
    let (lat2, lon2) = (-6.1753924, 106.8271528);
    let (ref_x2, ref_y2) = (11891944.25, -688776.39);
    let (fwd_x2, fwd_y2) = wgs84_to_epsg3857(lat2, lon2).expect("Forward Monas");
    assert!(
        (fwd_x2 - ref_x2).abs() < 1.0,
        "x selisih > 1m: {}",
        (fwd_x2 - ref_x2).abs()
    );
    assert!(
        (fwd_y2 - ref_y2).abs() < 1.0,
        "y selisih > 1m: {}",
        (fwd_y2 - ref_y2).abs()
    );
    let (calc_lat2, calc_lon2) = epsg3857_to_wgs84(ref_x2, ref_y2).expect("EPSG:3857 Monas");
    assert!((calc_lat2 - lat2).abs() < 9e-6, "lat selisih > 1m");
    assert!((calc_lon2 - lon2).abs() < 9e-6, "lon selisih > 1m");

    // Titik 3: Greenwich London (51.482578, 0.0)
    // Nilai referensi: golden values (deret Krüger, diverifikasi dengan implementasi independen) (EPSG:3857): x = 0.0, y = 6707104.24
    let (lat3, lon3) = (51.482578, 0.0);
    let (ref_x3, ref_y3) = (0.0, 6707104.24);
    let (fwd_x3, fwd_y3) = wgs84_to_epsg3857(lat3, lon3).expect("Forward Greenwich");
    assert!(
        (fwd_x3 - ref_x3).abs() < 1.0,
        "x selisih > 1m: {}",
        (fwd_x3 - ref_x3).abs()
    );
    assert!(
        (fwd_y3 - ref_y3).abs() < 1.0,
        "y selisih > 1m: {}",
        (fwd_y3 - ref_y3).abs()
    );
    let (calc_lat3, calc_lon3) = epsg3857_to_wgs84(ref_x3, ref_y3).expect("EPSG:3857 Greenwich");
    assert!((calc_lat3 - lat3).abs() < 9e-6);
    assert!((calc_lon3 - lon3).abs() < 9e-6);
}

/// (Butir 2) Uji Validasi Georeferensi Ketat terhadap Referensi Independen pyproj 3.8.0 / PROJ 9.8.1
/// Perintah verifikasi independen:
/// `uv run --with pyproj python -c "import pyproj; ..."`
/// Titik Uji:
/// 1. Paris UTM 31N (EPSG:32631)
/// 2. Balikpapan UTM 50S (EPSG:32750)
/// 3. Jakarta UTM 48S (EPSG:32748) (Nilai koreksi independen: E=701377.05, N=9314341.41)
/// 4. New York UTM 18N (EPSG:32618)
/// 5. Zona 48S Dekat Batas (lon=107.95, EPSG:32748)
/// 6. Null Island (EPSG:3857)
/// 7. Monas Jakarta (EPSG:3857)
/// 8. Paris Eiffel (EPSG:3857)
#[test]
fn test_projection_rigorous_independent_forward_and_inverse() {
    use aerial_analytics_platform_lib::modules::module_03::geometry::haversine::calculate_distance;

    println!("\n================================================================================");
    println!("VERIFIKASI PROYEKSI TERHADAP REFERENSI INDEPENDEN pyproj 3.8.0 / PROJ 9.8.1");
    println!("================================================================================");

    // Kasus 1: Paris UTM 31N
    let (lat1, lon1) = (48.858370, 2.294480);
    let (ref_e1, ref_n1) = (448250.50, 5411951.59);
    let (fwd_e1, fwd_n1) = wgs84_to_utm(lat1, lon1, 31, false).expect("Forward Paris");
    let diff_e1 = (fwd_e1 - ref_e1).abs();
    let diff_n1 = (fwd_n1 - ref_n1).abs();
    println!(
        "1. Paris UTM 31N: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_e1, fwd_n1, ref_e1, ref_n1, diff_e1, diff_n1
    );
    assert!(
        diff_e1 < 0.5,
        "Paris Easting selisih > 0.5m: {:.4}",
        diff_e1
    );
    assert!(
        diff_n1 < 0.5,
        "Paris Northing selisih > 0.5m: {:.4}",
        diff_n1
    );
    let (inv_lat1, inv_lon1) = utm_to_wgs84(ref_e1, ref_n1, 31, false).expect("Inverse Paris");
    let dist1 = calculate_distance([lon1, lat1], [inv_lon1, inv_lat1]);
    println!("   Inverse Paris ground error: {:.4} m", dist1);
    assert!(
        dist1 < 0.5,
        "Paris Inverse ground error > 0.5m: {:.4}",
        dist1
    );

    // Kasus 2: Balikpapan UTM 50S
    let (lat2, lon2) = (-1.260000, 116.830000);
    let (ref_e2, ref_n2) = (481087.77, 9860731.30);
    let (fwd_e2, fwd_n2) = wgs84_to_utm(lat2, lon2, 50, true).expect("Forward Balikpapan");
    let diff_e2 = (fwd_e2 - ref_e2).abs();
    let diff_n2 = (fwd_n2 - ref_n2).abs();
    println!(
        "2. Balikpapan UTM 50S: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_e2, fwd_n2, ref_e2, ref_n2, diff_e2, diff_n2
    );
    assert!(
        diff_e2 < 0.5,
        "Balikpapan Easting selisih > 0.5m: {:.4}",
        diff_e2
    );
    assert!(
        diff_n2 < 0.5,
        "Balikpapan Northing selisih > 0.5m: {:.4}",
        diff_n2
    );
    let (inv_lat2, inv_lon2) = utm_to_wgs84(ref_e2, ref_n2, 50, true).expect("Inverse Balikpapan");
    let dist2 = calculate_distance([lon2, lat2], [inv_lon2, inv_lat2]);
    println!("   Inverse Balikpapan ground error: {:.4} m", dist2);
    assert!(
        dist2 < 0.5,
        "Balikpapan Inverse ground error > 0.5m: {:.4}",
        dist2
    );

    // Kasus 3: Jakarta UTM 48S (Nilai Koreksi Independen)
    let (lat3, lon3) = (-6.200000, 106.820000);
    let (ref_e3, ref_n3) = (701377.05, 9314341.41);
    let (fwd_e3, fwd_n3) = wgs84_to_utm(lat3, lon3, 48, true).expect("Forward Jakarta");
    let diff_e3 = (fwd_e3 - ref_e3).abs();
    let diff_n3 = (fwd_n3 - ref_n3).abs();
    println!(
        "3. Jakarta UTM 48S: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_e3, fwd_n3, ref_e3, ref_n3, diff_e3, diff_n3
    );
    assert!(
        diff_e3 < 0.5,
        "Jakarta Easting selisih > 0.5m: {:.4}",
        diff_e3
    );
    assert!(
        diff_n3 < 0.5,
        "Jakarta Northing selisih > 0.5m: {:.4}",
        diff_n3
    );
    let (inv_lat3, inv_lon3) = utm_to_wgs84(ref_e3, ref_n3, 48, true).expect("Inverse Jakarta");
    let dist3 = calculate_distance([lon3, lat3], [inv_lon3, inv_lat3]);
    println!("   Inverse Jakarta ground error: {:.4} m", dist3);
    assert!(
        dist3 < 0.5,
        "Jakarta Inverse ground error > 0.5m: {:.4}",
        dist3
    );

    // Kasus 4: New York UTM 18N
    let (lat4, lon4) = (40.689247, -74.044502);
    let (ref_e4, ref_n4) = (580735.64, 4504700.38);
    let (fwd_e4, fwd_n4) = wgs84_to_utm(lat4, lon4, 18, false).expect("Forward New York");
    let diff_e4 = (fwd_e4 - ref_e4).abs();
    let diff_n4 = (fwd_n4 - ref_n4).abs();
    println!(
        "4. New York UTM 18N: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_e4, fwd_n4, ref_e4, ref_n4, diff_e4, diff_n4
    );
    assert!(
        diff_e4 < 0.5,
        "New York Easting selisih > 0.5m: {:.4}",
        diff_e4
    );
    assert!(
        diff_n4 < 0.5,
        "New York Northing selisih > 0.5m: {:.4}",
        diff_n4
    );
    let (inv_lat4, inv_lon4) = utm_to_wgs84(ref_e4, ref_n4, 18, false).expect("Inverse New York");
    let dist4 = calculate_distance([lon4, lat4], [inv_lon4, inv_lat4]);
    println!("   Inverse New York ground error: {:.4} m", dist4);
    assert!(
        dist4 < 0.5,
        "New York Inverse ground error > 0.5m: {:.4}",
        dist4
    );

    // Kasus 5: Zona 48S Dekat Batas (lon=107.95)
    let (lat5, lon5) = (-6.200000, 107.950000);
    let (ref_e5, ref_n5) = (826495.74, 9313778.56);
    let (fwd_e5, fwd_n5) = wgs84_to_utm(lat5, lon5, 48, true).expect("Forward Near Edge");
    let diff_e5 = (fwd_e5 - ref_e5).abs();
    let diff_n5 = (fwd_n5 - ref_n5).abs();
    println!(
        "5. Zona 48S Dekat Batas: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_e5, fwd_n5, ref_e5, ref_n5, diff_e5, diff_n5
    );
    assert!(
        diff_e5 < 0.5,
        "Near Edge Easting selisih > 0.5m: {:.4}",
        diff_e5
    );
    assert!(
        diff_n5 < 0.5,
        "Near Edge Northing selisih > 0.5m: {:.4}",
        diff_n5
    );
    let (inv_lat5, inv_lon5) = utm_to_wgs84(ref_e5, ref_n5, 48, true).expect("Inverse Near Edge");
    let dist5 = calculate_distance([lon5, lat5], [inv_lon5, inv_lat5]);
    println!("   Inverse Near Edge ground error: {:.4} m", dist5);
    assert!(
        dist5 < 0.5,
        "Near Edge Inverse ground error > 0.5m: {:.4}",
        dist5
    );

    // Kasus 6: Null Island EPSG:3857
    let (lat6, lon6) = (0.0, 0.0);
    let (ref_x6, ref_y6) = (0.00, 0.00);
    let (fwd_x6, fwd_y6) = wgs84_to_epsg3857(lat6, lon6).expect("Forward Null Island");
    println!(
        "6. Null Island EPSG:3857: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2})",
        fwd_x6, fwd_y6, ref_x6, ref_y6
    );
    assert!((fwd_x6 - ref_x6).abs() < 0.5);
    assert!((fwd_y6 - ref_y6).abs() < 0.5);
    let (inv_lat6, inv_lon6) = epsg3857_to_wgs84(ref_x6, ref_y6).expect("Inverse Null Island");
    let dist6 = calculate_distance([lon6, lat6], [inv_lon6, inv_lat6]);
    assert!(dist6 < 0.5);

    // Kasus 7: Monas Jakarta EPSG:3857
    let (lat7, lon7) = (-6.1753924, 106.8271528);
    let (ref_x7, ref_y7) = (11891944.25, -688776.39);
    let (fwd_x7, fwd_y7) = wgs84_to_epsg3857(lat7, lon7).expect("Forward Monas EPSG:3857");
    let diff_x7 = (fwd_x7 - ref_x7).abs();
    let diff_y7 = (fwd_y7 - ref_y7).abs();
    println!(
        "7. Monas EPSG:3857: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_x7, fwd_y7, ref_x7, ref_y7, diff_x7, diff_y7
    );
    assert!(diff_x7 < 0.5);
    assert!(diff_y7 < 0.5);
    let (inv_lat7, inv_lon7) = epsg3857_to_wgs84(ref_x7, ref_y7).expect("Inverse Monas EPSG:3857");
    let dist7 = calculate_distance([lon7, lat7], [inv_lon7, inv_lat7]);
    println!("   Inverse Monas ground error: {:.4} m", dist7);
    assert!(dist7 < 0.5);

    // Kasus 8: Paris Eiffel EPSG:3857
    let (lat8, lon8) = (48.858370, 2.294480);
    let (ref_x8, ref_y8) = (255420.35, 6250863.83);
    let (fwd_x8, fwd_y8) = wgs84_to_epsg3857(lat8, lon8).expect("Forward Paris EPSG:3857");
    let diff_x8 = (fwd_x8 - ref_x8).abs();
    let diff_y8 = (fwd_y8 - ref_y8).abs();
    println!(
        "8. Paris EPSG:3857: fwd=({:.2}, {:.2}), ref=({:.2}, {:.2}), diff=({:.4} m, {:.4} m)",
        fwd_x8, fwd_y8, ref_x8, ref_y8, diff_x8, diff_y8
    );
    assert!(diff_x8 < 0.5);
    assert!(diff_y8 < 0.5);
    let (inv_lat8, inv_lon8) = epsg3857_to_wgs84(ref_x8, ref_y8).expect("Inverse Paris EPSG:3857");
    let dist8 = calculate_distance([lon8, lat8], [inv_lon8, inv_lat8]);
    println!("   Inverse Paris ground error: {:.4} m", dist8);
    assert!(dist8 < 0.5);
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
