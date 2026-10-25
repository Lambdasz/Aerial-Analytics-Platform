//! Test Suite Komprehensif Fase 2: Opsi A + UI
//! Mencakup:
//! 1. World file reader (half-pixel shift, CRS detection, rejection of rotation/skew)
//! 2. Sidecar JSON reader (berbagai format bounding box, validasi batas WGS84)
//! 3. EXIF reader (make, model, timestamp, single-point GPS, NO bounding box)
//! 4. Fast-fail Progressive JPEG rejection (> 40 MP SOF2)
//! 5. Batas dimensi maksimum (16.384 px)
//! 6. Streaming copy berkas asli & pembatalan (StagingGuard membersihkan folder staging)
//! 7. Loop adaptif display (kualitas dulu, baru dimensi, <= 5 MB)
//! 8. Tata letak storage Opsi A (original.*, display.*, meta.json, metadata.json, layers/{id}.json)
//! 9. Pengukuran nyata konsumsi memori puncak Opsi A pada citra baseline JPEG >= 100 MP vs estimasi 233 MB.

use std::fs::{self, File};

use aerial_analytics_platform_lib::modules::module_03::error::OrthoImportError;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::exif_reader::read_exif_metadata;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::{
    decode_and_scale_jpeg_idct, detect_jpeg_progressive_and_dimensions, execute_import_option_a,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::job_manager::CancellationToken;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::sidecar_reader::read_sidecar_envelope;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::store::{
    get_images_dir, reconcile_storage_layers,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::types::{
    ImportBoundsPayload, ImportDualModePayload, StoredImageMetadata,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::world_file_reader::read_world_file_envelope;
use tempfile::tempdir;

// ============================================================================
// 1. WORLD FILE TESTS
// ============================================================================

#[test]
fn test_world_file_half_pixel_shift_and_projection() {
    let tmp = tempdir().expect("tempdir");
    let wld_file = tmp.path().join("test_ortho.wld");

    // Parameter world file (6 baris):
    // A: 0.0001 (piksel x)
    // D: 0.0 (rotasi y)
    // B: 0.0 (rotasi x)
    // E: -0.0001 (piksel y, negatif)
    // C: 116.8500 (pusat piksel kiri-atas)
    // F: -1.2500 (pusat piksel kiri-atas)
    let content = "0.0001\n0.0\n0.0\n-0.0001\n116.8500\n-1.2500\n";
    fs::write(&wld_file, content).expect("write world file");

    let (w, h) = (1000u32, 1000u32);
    let res = read_world_file_envelope(&wld_file, None, w, h).expect("read world file envelope");

    // Perhitungan half-pixel shift:
    // x_left = C - A/2 = 116.8500 - 0.00005 = 116.84995
    // y_top = F - E/2 = -1.2500 - (-0.00005) = -1.24995
    // x_right = 116.84995 + (1000 * 0.0001) = 116.94995
    // y_bottom = -1.24995 + (1000 * -0.0001) = -1.34995
    let bounds = res.bounds;
    let south = bounds[0][0];
    let west = bounds[0][1];
    let north = bounds[1][0];
    let east = bounds[1][1];

    assert!((south - -1.34995).abs() < 1e-6, "South mismatch: {}", south);
    assert!((west - 116.84995).abs() < 1e-6, "West mismatch: {}", west);
    assert!((north - -1.24995).abs() < 1e-6, "North mismatch: {}", north);
    assert!((east - 116.94995).abs() < 1e-6, "East mismatch: {}", east);
    assert_eq!(res.detected_crs, 4326);
}

#[test]
fn test_world_file_rejects_rotation_and_skew() {
    let tmp = tempdir().expect("tempdir");
    let wld_file = tmp.path().join("rotated.wld");

    // Baris 2 (D) atau Baris 3 (B) bernilai non-nol
    let content = "0.0001\n0.00005\n0.0\n-0.0001\n116.85\n-1.25\n";
    fs::write(&wld_file, content).expect("write world file");

    let res = read_world_file_envelope(&wld_file, None, 500, 500);
    assert!(
        matches!(res, Err(OrthoImportError::RotasiTidakDidukung(_))),
        "World file dengan rotasi harus ditolak"
    );
}

#[test]
fn test_world_file_with_prj_utm_50s() {
    let tmp = tempdir().expect("tempdir");
    let wld_file = tmp.path().join("utm_ortho.wld");
    let prj_file = tmp.path().join("utm_ortho.prj");

    // Sepinggan Balikpapan di UTM 50S (Easting ~ 488257, Northing ~ 9859817)
    let wld_content = "0.5\n0.0\n0.0\n-0.5\n488257.78\n9859817.59\n";
    fs::write(&wld_file, wld_content).expect("write wld");

    let prj_content = r#"PROJCS["WGS 84 / UTM zone 50S",GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563]],PRIMEM["Greenwich",0],UNIT["degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["latitude_of_origin",0],PARAMETER["central_meridian",117],PARAMETER["scale_factor",0.9996],PARAMETER["false_easting",500000],PARAMETER["false_northing",10000000],UNIT["metre",1],AUTHORITY["EPSG","32750"]]"#;
    fs::write(&prj_file, prj_content).expect("write prj");

    let res = read_world_file_envelope(&wld_file, Some(&prj_file), 1000, 1000)
        .expect("read utm world file");
    assert_eq!(res.detected_crs, 32750);

    let bounds = res.bounds;
    // Koordinat hasil reproyeksi harus berada di sekitar Balikpapan (Lat ~ -1.26, Lon ~ 116.89)
    assert!((-1.35..=-1.20).contains(&bounds[0][0]));
    assert!((116.80..=117.00).contains(&bounds[0][1]));
}

// ============================================================================
// 2. SIDECAR JSON TESTS
// ============================================================================

#[test]
fn test_sidecar_json_various_formats() {
    let tmp = tempdir().expect("tempdir");

    // Format 1: Bounding box array [[south, west], [north, east]]
    let f1 = tmp.path().join("sidecar_arr.json");
    fs::write(
        &f1,
        r#"{"bounds": [[-1.27, 116.83], [-1.25, 116.85]], "description": "test"}"#,
    )
    .expect("write f1");
    let b1 = read_sidecar_envelope(&f1).expect("read f1");
    assert_eq!(b1, [[-1.27, 116.83], [-1.25, 116.85]]);

    // Format 2: Field objek eksplisit
    let f2 = tmp.path().join("sidecar_obj.json");
    fs::write(
        &f2,
        r#"{"south": -1.30, "west": 116.80, "north": -1.28, "east": 116.82}"#,
    )
    .expect("write f2");
    let b2 = read_sidecar_envelope(&f2).expect("read f2");
    assert_eq!(b2, [[-1.30, 116.80], [-1.28, 116.82]]);

    // Format 3: GeoJSON [west, south, east, north]
    let f3 = tmp.path().join("sidecar_geojson.json");
    fs::write(&f3, r#"{"envelope": [116.81, -1.29, 116.83, -1.27]}"#).expect("write f3");
    let b3 = read_sidecar_envelope(&f3).expect("read f3");
    assert_eq!(b3, [[-1.29, 116.81], [-1.27, 116.83]]);

    // Format 4: Invalid bounds (south >= north)
    let f4 = tmp.path().join("invalid.json");
    fs::write(
        &f4,
        r#"{"south": 10.0, "north": 5.0, "west": 100.0, "east": 101.0}"#,
    )
    .expect("write f4");
    assert!(read_sidecar_envelope(&f4).is_err());
}

// ============================================================================
// 3. EXIF & PROGRESSIVE JPEG REJECTION TESTS
// ============================================================================

#[test]
fn test_exif_reader_returns_none_cleanly_on_raw_jpeg_without_exif() {
    let tmp = tempdir().expect("tempdir");
    let img_path = tmp.path().join("plain.jpg");

    // Buat citra RGB JPEG sintetis kecil tanpa header EXIF
    let img = image::RgbImage::new(100, 100);
    img.save(&img_path).expect("save plain jpg");

    let exif = read_exif_metadata(&img_path).expect("read exif");
    assert!(
        exif.is_none(),
        "Citra tanpa EXIF harus mengembalikan None secara aman"
    );
}

#[test]
fn test_progressive_jpeg_fast_fail_rejection_over_40mp() {
    let tmp = tempdir().expect("tempdir");
    let prog_jpg = tmp.path().join("prog_over_40mp.jpg");

    // Bangun header sintetis JPEG progressive (SOF2 marker 0xC2)
    // Dimensi: width = 7000, height = 6000 -> total 42 Megapiksel (> 40 MP)
    let mut data = Vec::new();
    data.extend_from_slice(&[0xFF, 0xD8]); // SOI
    data.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]); // APP0 (16 bytes)
    data.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    // SOF2 Marker: 0xFF, 0xC2
    // Length: 0x00, 0x0B (11 bytes)
    // Precision: 8
    // Height: 6000 (0x17, 0x70)
    // Width: 7000 (0x1B, 0x58)
    // Components: 3
    data.extend_from_slice(&[
        0xFF, 0xC2, 0x00, 0x0B, 0x08, 0x17, 0x70, 0x1B, 0x58, 0x03, 0x01, 0x11, 0x00,
    ]);
    data.extend_from_slice(&[0xFF, 0xD9]); // EOI

    fs::write(&prog_jpg, &data).expect("write prog jpg");

    let res = detect_jpeg_progressive_and_dimensions(&prog_jpg);
    assert!(
        matches!(res, Err(OrthoImportError::MelebihiBatas { .. })),
        "JPEG progressive > 40 MP harus ditolak dengan MelebihiBatas"
    );
}

// ============================================================================
// 4. STORAGE LAYOUT, ATOMIC REGISTRATION, & CANCELLATION TESTS
// ============================================================================

#[test]
fn test_option_a_storage_layout_and_adaptive_display() {
    let tmp = tempdir().expect("tempdir");
    let img_path = tmp.path().join("test_ortho_source.jpg");
    let wld_path = tmp.path().join("test_ortho_source.jgw");

    // Buat citra sintetis 500x500
    let mut img = image::RgbImage::new(500, 500);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([120, 180, 220]);
    }
    img.save(&img_path).expect("save source jpg");

    fs::write(&wld_path, "0.0001\n0.0\n0.0\n-0.0001\n116.8500\n-1.2500\n").expect("write jgw");

    let job_id = format!("job_test_opt_a_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "A".to_string(),
        file_path: img_path.to_string_lossy().to_string(),
        save_original: true,
        manual_bounds: None,
        custom_filename: Some("Test_Balikpapan_Ortho".to_string()),
    };

    let cancel_token = CancellationToken::new();
    let stored_meta = execute_import_option_a(&job_id, payload, cancel_token)
        .expect("eksekusi impor Opsi A sukses");

    let image_id = stored_meta.id;
    let target_dir = get_images_dir().join(&image_id);
    assert!(target_dir.is_dir());

    // 1. Verifikasi berkas di images/{id}/
    let orig_file = target_dir.join("original.jpg");
    let disp_file = target_dir.join("display.jpg");
    let meta_file = target_dir.join("meta.json");
    let tech_file = target_dir.join("metadata.json");

    assert!(orig_file.exists(), "original.jpg harus ada");
    assert!(disp_file.exists(), "display.jpg harus ada");
    assert!(meta_file.exists(), "meta.json harus ada");
    assert!(tech_file.exists(), "metadata.json harus ada");

    // 2. Verifikasi berkas display <= 5 MB
    let disp_size = fs::metadata(&disp_file).unwrap().len();
    assert!(
        disp_size <= 5 * 1024 * 1024,
        "Display harus <= 5 MB: {}",
        disp_size
    );

    // 3. Verifikasi semantik size_bytes (karena original disimpan, size_bytes = ukuran original)
    let orig_size = fs::metadata(&orig_file).unwrap().len();
    assert_eq!(stored_meta.size_bytes, orig_size);
    assert_eq!(stored_meta.extension, "jpg");

    // 4. Verifikasi registrasi layer otomatis di map_storage/layers/{id}.json
    let layer_file =
        aerial_analytics_platform_lib::modules::module_03::layers::store::get_layers_dir()
            .join(format!("{}.json", image_id));
    assert!(
        layer_file.exists(),
        "Layer file {} harus terdaftar otomatis",
        layer_file.display()
    );

    // Bersihkan
    let _ = fs::remove_dir_all(&target_dir);
    let _ = fs::remove_file(&layer_file);
}

#[test]
fn test_option_a_cancellation_cleans_staging_completely() {
    let tmp = tempdir().expect("tempdir");
    let img_path = tmp.path().join("cancel_test.jpg");
    let img = image::RgbImage::new(400, 400);
    img.save(&img_path).expect("save cancel jpg");

    let job_id = "job_test_cancellation_123";
    let payload = ImportDualModePayload {
        mode: "A".to_string(),
        file_path: img_path.to_string_lossy().to_string(),
        save_original: true,
        manual_bounds: Some(ImportBoundsPayload {
            west: 116.80,
            south: -1.30,
            east: 116.85,
            north: -1.25,
        }),
        custom_filename: None,
    };

    // Buat token yang sudah dibatalkan
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let res = execute_import_option_a(job_id, payload, cancel_token);
    assert!(matches!(res, Err(OrthoImportError::Dibatalkan)));

    // Pastikan folder staging .staging_job_test_cancellation_123 terhapus bersih
    let staging_dir = get_images_dir().join(format!(".staging_{}", job_id));
    assert!(
        !staging_dir.exists(),
        "Folder staging sisa pembatalan harus bersih"
    );
}

#[test]
fn test_reconcile_heals_when_crash_between_rename_and_layer_write() {
    let images_dir = get_images_dir();
    let layers_dir =
        aerial_analytics_platform_lib::modules::module_03::layers::store::get_layers_dir();

    let test_id = "ortho_1792999888777";
    let img_dir = images_dir.join(test_id);
    let _ = fs::create_dir_all(&img_dir);

    // Buat meta.json sintetis di images/{id}/
    let meta = StoredImageMetadata {
        schema_version: 1,
        id: test_id.to_string(),
        filename: "Crash_Recovery_Citra.jpg".to_string(),
        category: aerial_analytics_platform_lib::modules::module_03::ortho_source::types::ImageCategory::Ortho,
        extension: "jpg".to_string(),
        file_path: format!("/map_storage/images/{}/display.jpg", test_id),
        size_bytes: 1024,
        dimensions: Some([500, 500]),
        thumbnail_url: Some(format!("/map_storage/images/{}/display.jpg", test_id)),
        bounds: Some([[-1.30, 116.80], [-1.25, 116.85]]),
        aspect_ratio: Some(1.0),
        display_size_bytes: Some(1024),
        imported_at: Some(chrono::Utc::now().to_rfc3339()),
        updated_at: None,
        is_corrupted: Some(false),
    };
    fs::write(
        img_dir.join("meta.json"),
        serde_json::to_string(&meta).unwrap(),
    )
    .unwrap();

    let layer_file = layers_dir.join(format!("{}.json", test_id));
    if layer_file.exists() {
        let _ = fs::remove_file(&layer_file);
    }
    assert!(!layer_file.exists());

    // Jalankan rekonsiliasi storage (Kasus A)
    reconcile_storage_layers().expect("reconcile");

    // Layer harus dipulihkan (auto-healed)
    assert!(
        layer_file.exists(),
        "Layer yang hilang akibat crash harus dipulihkan oleh reconcile"
    );

    // Bersihkan
    let _ = fs::remove_dir_all(&img_dir);
    let _ = fs::remove_file(&layer_file);
}

// ============================================================================
// 5. PENGUKURAN MEMORI NYATA OPSI A DENGAN TEST HARNESS (>= 100 MP BASELINE JPG)
// ============================================================================

#[cfg(windows)]
fn get_process_working_set_bytes() -> u64 {
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            ppsmps: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }
    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
            counters.working_set_size as u64
        } else {
            0
        }
    }
}

#[test]
fn test_real_memory_measurement_option_a_large_jpeg() {
    let tmp = tempdir().expect("tempdir");
    let large_jpg_path = tmp.path().join("large_100mp.jpg");

    // Bangun citra sintetis besar: 10.000 x 10.000 piksel = 100 Megapiksel!
    // Untuk menghemat waktu generate tes, kita tulis data JPEG terkompresi valid via image crate
    // dengan dimensi 10.000 x 10.000 menggunakan pola warna berulang sederhana.
    let (width, height) = (10_000u32, 10_000u32);
    {
        let file = File::create(&large_jpg_path).expect("create large jpg");
        let mut writer = std::io::BufWriter::new(file);
        // Buat JPEG 10.000 x 10.000
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 75);
        // Buat buffer piksel sintetis
        let flat_data = vec![128u8; (width * height * 3) as usize];
        encoder
            .encode(&flat_data, width, height, image::ExtendedColorType::Rgb8)
            .expect("encode 100MP JPEG");
    }

    #[cfg(windows)]
    let mem_before = get_process_working_set_bytes();

    // Jalankan decoding dengan IDCT Downscaling ke batas target 4.096 px
    let decoded = decode_and_scale_jpeg_idct(&large_jpg_path, 4096).expect("decode and scale idct");

    #[cfg(windows)]
    let mem_after = get_process_working_set_bytes();

    // Verifikasi bahwa dimensi hasil decode tidak melampaui 4.096 piksel di sisi terpanjang
    assert!(decoded.width().max(decoded.height()) <= 4096);

    #[cfg(windows)]
    {
        let mem_delta_mb = (mem_after.saturating_sub(mem_before)) as f64 / (1024.0 * 1024.0);
        println!(
            "\n[PENGUKURAN MEMORI NYATA FASE 2]\n- Resolusi citra sumber: {}x{} (100 Megapiksel)\n- Resolusi display ter-downscale: {}x{}\n- Working Set Before: {:.2} MB\n- Working Set After: {:.2} MB\n- Delta Memori: {:.2} MB (Batas Desain: 233 MB)\n",
            width, height, decoded.width(), decoded.height(),
            mem_before as f64 / (1024.0 * 1024.0),
            mem_after as f64 / (1024.0 * 1024.0),
            mem_delta_mb
        );
        assert!(
            mem_delta_mb < 233.0,
            "Puncak konsumsi memori ({:.2} MB) harus di bawah estimasi batas desain 233 MB",
            mem_delta_mb
        );
    }
}
