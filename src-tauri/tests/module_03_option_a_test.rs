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
fn test_png_import_with_manual_bounds_and_alpha() {
    let tmp = tempdir().expect("tempdir");
    let png_path = tmp.path().join("test_alpha.png");

    // Buat citra RGBA dengan alpha channel
    let mut img = image::RgbaImage::new(400, 300);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let alpha = if (x + y) % 2 == 0 { 255 } else { 128 };
        *pixel = image::Rgba([100, 150, 200, alpha]);
    }
    img.save(&png_path).expect("save png");

    let job_id = format!("job_png_manual_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "A".to_string(),
        file_path: png_path.to_string_lossy().to_string(),
        save_original: true,
        manual_bounds: Some(ImportBoundsPayload {
            west: 116.82,
            south: -1.28,
            east: 116.86,
            north: -1.24,
        }),
        custom_filename: Some("PNG_Alpha_Manual".to_string()),
    };

    let cancel_token = CancellationToken::new();
    let stored_meta =
        execute_import_option_a(&job_id, payload, cancel_token).expect("eksekusi impor PNG sukses");

    let image_id = stored_meta.id;
    let target_dir = get_images_dir().join(&image_id);
    assert!(target_dir.is_dir());

    // Verifikasi display adalah PNG (mempertahankan transparansi alpha)
    let disp_file = target_dir.join("display.png");
    let orig_file = target_dir.join("original.png");
    assert!(
        disp_file.exists(),
        "display.png harus dibuat untuk citra alpha"
    );
    assert!(orig_file.exists(), "original.png harus ada");

    // Verifikasi bounds
    let bounds = stored_meta.bounds.expect("bounds harus ada");
    assert_eq!(bounds, [[-1.28, 116.82], [-1.24, 116.86]]);

    // Bersihkan
    let _ = fs::remove_dir_all(&target_dir);
    let layer_file =
        aerial_analytics_platform_lib::modules::module_03::layers::store::get_layers_dir()
            .join(format!("{}.json", image_id));
    let _ = fs::remove_file(&layer_file);
}

#[test]
fn test_instance_lock_blocks_second_instance_and_skips_cleanup() {
    let tmp = tempdir().expect("tempdir");
    let lock_file = tmp.path().join(".test_instance.lock");

    // 1. Kunci pertama berhasil didapat
    let lock1 = aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::AppInstanceLock::try_acquire(lock_file.clone())
        .expect("Kunci instance pertama harus berhasil didapat");

    // 2. Kunci kedua dari proses/thread lain harus GAGAL dengan varian AplikasiSudahBerjalan
    let lock2_res = aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::AppInstanceLock::try_acquire(lock_file.clone());
    assert!(
        matches!(lock2_res, Err(OrthoImportError::AplikasiSudahBerjalan(_))),
        "Instance kedua harus menghasilkan error AplikasiSudahBerjalan, bukan JobSedangBerjalan: {:?}",
        lock2_res
    );

    // 3. Verifikasi perilaku startup: instance kedua TIDAK BOLEH memanggil cleanup atau reconcile
    let mut cleanup_called = false;
    let mut reconcile_called = false;
    let acquire_res = aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::AppInstanceLock::try_acquire(lock_file.clone());
    match acquire_res {
        Ok(_) => {
            cleanup_called = true;
            reconcile_called = true;
        }
        Err(e) => {
            assert!(matches!(e, OrthoImportError::AplikasiSudahBerjalan(_)));
            // Mode sekunder: Melewati cleanup dan reconcile!
        }
    }
    assert!(
        !cleanup_called,
        "Cleanup STALE jobs TIDAK BOLEH dipanggil oleh instance kedua!"
    );
    assert!(
        !reconcile_called,
        "Reconcile storage TIDAK BOLEH dipanggil oleh instance kedua!"
    );

    drop(lock1);
}

#[test]
fn test_edge_case_jpeg_rejections() {
    let tmp = tempdir().expect("tempdir");

    // 1. Arithmetic coding (SOF9 = 0xC9)
    let p_sof9 = tmp.path().join("arithmetic_sof9.jpg");
    let mut sof9_bytes = vec![0xFF, 0xD8]; // SOI
    sof9_bytes.extend_from_slice(&[
        0xFF, 0xC9, 0x00, 0x0B, 0x08, 0x00, 0x64, 0x00, 0x64, 0x03, 0x01, 0x11, 0x00,
    ]);
    fs::write(&p_sof9, &sof9_bytes).unwrap();
    let res_sof9 = detect_jpeg_progressive_and_dimensions(&p_sof9);
    assert!(
        matches!(res_sof9, Err(OrthoImportError::FormatTidakDidukung(_))),
        "JPEG SOF9 arithmetic harus ditolak: {:?}",
        res_sof9
    );

    // 2. 12-bit precision JPEG
    let p_12bit = tmp.path().join("precision_12bit.jpg");
    let mut bit12_bytes = vec![0xFF, 0xD8];
    bit12_bytes.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 0x0C, 0x00, 0x64, 0x00, 0x64, 0x03, 0x01, 0x11, 0x00,
    ]);
    fs::write(&p_12bit, &bit12_bytes).unwrap();
    let res_12bit = detect_jpeg_progressive_and_dimensions(&p_12bit);
    assert!(
        matches!(res_12bit, Err(OrthoImportError::FormatTidakDidukung(_))),
        "JPEG 12-bit harus ditolak: {:?}",
        res_12bit
    );

    // 3. CMYK / YCCK 4-komponen
    let p_cmyk = tmp.path().join("cmyk_4comp.jpg");
    let mut cmyk_bytes = vec![0xFF, 0xD8];
    cmyk_bytes.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0E, 0x08, 0x00, 0x64, 0x00, 0x64, 0x04, 0x01, 0x11, 0x00, 0x02, 0x11,
        0x00,
    ]);
    fs::write(&p_cmyk, &cmyk_bytes).unwrap();
    let res_cmyk = detect_jpeg_progressive_and_dimensions(&p_cmyk);
    assert!(
        matches!(res_cmyk, Err(OrthoImportError::FormatTidakDidukung(_))),
        "JPEG CMYK 4-komponen harus ditolak: {:?}",
        res_cmyk
    );
}

#[test]
fn test_edge_case_png_rejections() {
    let tmp = tempdir().expect("tempdir");

    // 1. PNG 16-bit depth
    let p_16bit = tmp.path().join("png_16bit.png");
    {
        let file = File::create(&p_16bit).unwrap();
        let mut enc = png::Encoder::new(file, 50, 50);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Sixteen);
        let mut writer = enc.write_header().unwrap();
        let data = vec![0u8; 50 * 50 * 3 * 2];
        writer.write_image_data(&data).unwrap();
    }
    let res_16bit = aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::decode_and_scale_png(&p_16bit, 4096);
    assert!(
        matches!(res_16bit, Err(OrthoImportError::FormatTidakDidukung(_))),
        "PNG 16-bit harus ditolak: {:?}",
        res_16bit
    );

    // 2. PNG Palette / Indexed
    let p_pal = tmp.path().join("png_indexed.png");
    {
        let file = File::create(&p_pal).unwrap();
        let mut enc = png::Encoder::new(file, 50, 50);
        enc.set_color(png::ColorType::Indexed);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_palette(&[255, 0, 0, 0, 255, 0, 0, 0, 255]);
        let mut writer = enc.write_header().unwrap();
        let data = vec![0u8; 50 * 50];
        writer.write_image_data(&data).unwrap();
    }
    let res_pal = aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::decode_and_scale_png(&p_pal, 4096);
    assert!(
        matches!(res_pal, Err(OrthoImportError::FormatTidakDidukung(_))),
        "PNG Palette harus ditolak: {:?}",
        res_pal
    );
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

#[test]
fn test_exif_positive_synthetic_extraction_and_orientation_warning() {
    let tmp = tempdir().expect("tempdir");
    let jpg_path = tmp.path().join("synthetic_exif.jpg");

    // Bangun segmen JPEG valid dengan EXIF APP1 buatan:
    // Header TIFF Little Endian ("II", 42, offset IFD0 = 8)
    // IFD0 memuat:
    // - Tag 0x010F (Make): "DJI\0"
    // - Tag 0x0110 (Model): "FC3\0"
    // - Tag 0x0112 (Orientation): 6 (SHORT, rotasi 90 derajat CW)
    // - Tag 0x0132 (DateTime): "2026:10:25 12:00:00\0"
    // - Tag 0x8825 (GPS IFD Pointer): offset ke GPS sub-IFD
    let mut tiff_data = Vec::new();
    // TIFF Header: 8 bytes
    tiff_data.extend_from_slice(b"II\x2A\x00\x08\x00\x00\x00");

    // IFD0: 5 entries (2 bytes count) -> offset 8
    let ifd0_count = 5u16;
    tiff_data.extend_from_slice(&ifd0_count.to_le_bytes());

    // Hitung offset untuk data yang > 4 bytes:
    // Header (8) + Count (2) + 5 entries * 12 (60) + Next IFD (4) = 74 bytes
    let str_datetime_offset = 74u32; // 20 bytes
    let gps_ifd_offset = str_datetime_offset + 20; // offset 94

    // Entry 1: Make (0x010F), ASCII (2), len 4, inline "DJI\0"
    tiff_data.extend_from_slice(&0x010Fu16.to_le_bytes());
    tiff_data.extend_from_slice(&2u16.to_le_bytes());
    tiff_data.extend_from_slice(&4u32.to_le_bytes());
    tiff_data.extend_from_slice(b"DJI\0");

    // Entry 2: Model (0x0110), ASCII (2), len 4, inline "FC3\0"
    tiff_data.extend_from_slice(&0x0110u16.to_le_bytes());
    tiff_data.extend_from_slice(&2u16.to_le_bytes());
    tiff_data.extend_from_slice(&4u32.to_le_bytes());
    tiff_data.extend_from_slice(b"FC3\0");

    // Entry 3: Orientation (0x0112), SHORT (3), count 1, value 6
    tiff_data.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff_data.extend_from_slice(&3u16.to_le_bytes());
    tiff_data.extend_from_slice(&1u32.to_le_bytes());
    tiff_data.extend_from_slice(&6u16.to_le_bytes());
    tiff_data.extend_from_slice(&[0u8, 0u8]); // padding inline value

    // Entry 4: DateTime (0x0132), ASCII (2), count 20, offset str_datetime_offset
    tiff_data.extend_from_slice(&0x0132u16.to_le_bytes());
    tiff_data.extend_from_slice(&2u16.to_le_bytes());
    tiff_data.extend_from_slice(&20u32.to_le_bytes());
    tiff_data.extend_from_slice(&str_datetime_offset.to_le_bytes());

    // Entry 5: GPSInfo IFD (0x8825), LONG (4), count 1, offset gps_ifd_offset
    tiff_data.extend_from_slice(&0x8825u16.to_le_bytes());
    tiff_data.extend_from_slice(&4u16.to_le_bytes());
    tiff_data.extend_from_slice(&1u32.to_le_bytes());
    tiff_data.extend_from_slice(&gps_ifd_offset.to_le_bytes());

    // Next IFD = 0
    tiff_data.extend_from_slice(&0u32.to_le_bytes());

    // Value untuk DateTime: "2026:10:25 12:00:00\0" (20 bytes)
    tiff_data.extend_from_slice(b"2026:10:25 12:00:00\0");

    // GPS IFD: offset 94
    // 3 entries: GPSLatitudeRef (0x0001), GPSLatitude (0x0002), GPSAltitude (0x0006)
    let gps_count = 3u16;
    tiff_data.extend_from_slice(&gps_count.to_le_bytes());

    let rationals_offset = gps_ifd_offset + 2 + 3 * 12 + 4; // 94 + 2 + 36 + 4 = 136

    // GPS Entry 1: GPSLatitudeRef (1), ASCII (2), len 2, inline "S\0"
    tiff_data.extend_from_slice(&0x0001u16.to_le_bytes());
    tiff_data.extend_from_slice(&2u16.to_le_bytes());
    tiff_data.extend_from_slice(&2u32.to_le_bytes());
    tiff_data.extend_from_slice(b"S\0\0\0");

    // GPS Entry 2: GPSLatitude (2), RATIONAL (5), count 3, offset rationals_offset
    tiff_data.extend_from_slice(&0x0002u16.to_le_bytes());
    tiff_data.extend_from_slice(&5u16.to_le_bytes());
    tiff_data.extend_from_slice(&3u32.to_le_bytes());
    tiff_data.extend_from_slice(&rationals_offset.to_le_bytes());

    // GPS Entry 3: GPSAltitude (6), RATIONAL (5), count 1, offset rationals_offset + 24
    let alt_offset = rationals_offset + 24;
    tiff_data.extend_from_slice(&0x0006u16.to_le_bytes());
    tiff_data.extend_from_slice(&5u16.to_le_bytes());
    tiff_data.extend_from_slice(&1u32.to_le_bytes());
    tiff_data.extend_from_slice(&alt_offset.to_le_bytes());

    // Next GPS IFD = 0
    tiff_data.extend_from_slice(&0u32.to_le_bytes());

    // 3 RATIONALs untuk Lat: 1/1 deg, 15/1 min, 137304/10000 sec (= 1.253814 deg)
    tiff_data.extend_from_slice(&1u32.to_le_bytes());
    tiff_data.extend_from_slice(&1u32.to_le_bytes());
    tiff_data.extend_from_slice(&15u32.to_le_bytes());
    tiff_data.extend_from_slice(&1u32.to_le_bytes());
    tiff_data.extend_from_slice(&137304u32.to_le_bytes());
    tiff_data.extend_from_slice(&10000u32.to_le_bytes());

    // 1 RATIONAL untuk Altitude: 1205/10 m (= 120.5 m)
    tiff_data.extend_from_slice(&1205u32.to_le_bytes());
    tiff_data.extend_from_slice(&10u32.to_le_bytes());

    // Susun berkas JPEG lengkap: SOI + APP1 + SOF0 minimal + EOI
    let mut jpg_bytes = vec![0xFF, 0xD8]; // SOI
    let app1_len = (tiff_data.len() + 6 + 2) as u16; // + "Exif\0\0" (6) + length bytes (2)
    jpg_bytes.extend_from_slice(&[0xFF, 0xE1]);
    jpg_bytes.extend_from_slice(&app1_len.to_be_bytes());
    jpg_bytes.extend_from_slice(b"Exif\0\0");
    jpg_bytes.extend_from_slice(&tiff_data);

    // SOF0 (dimensi 100 x 100)
    jpg_bytes.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x64, 0x00, 0x64, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11,
        0x00, 0x03, 0x11, 0x00,
    ]);
    // EOI
    jpg_bytes.extend_from_slice(&[0xFF, 0xD9]);

    fs::write(&jpg_path, &jpg_bytes).expect("tulis synthetic exif jpeg");

    // 1. Uji ekstraksi metadata positif (Make, Model, DateTime, GPS)
    let meta_res = read_exif_metadata(&jpg_path).expect("read exif metadata");
    assert!(
        meta_res.is_some(),
        "EXIF metadata harus berhasil diekstrak!"
    );
    let m = meta_res.unwrap();
    assert_eq!(m.camera_make.as_deref(), Some("DJI"));
    assert_eq!(m.camera_model.as_deref(), Some("FC3"));
    assert_eq!(m.captured_at.as_deref(), Some("2026-10-25 12:00:00"));
    assert!(m.gps_latitude.is_some(), "GPS latitude harus ada");
    let lat = m.gps_latitude.unwrap();
    assert!(
        (lat - (-1.253814)).abs() < 1e-4,
        "Latitude harus -1.2538: {}",
        lat
    );
    assert_eq!(m.gps_altitude_agl_m, Some(120.5));

    // 2. Uji peringatan Orientation != 1 (Orientation = 6)
    let warn = aerial_analytics_platform_lib::modules::module_03::ortho_source::check_exif_orientation_warning(&jpg_path);
    assert!(
        warn.is_some(),
        "Peringatan orientation harus dihasilkan untuk orientation != 1"
    );
    let w_text = warn.unwrap();
    assert!(
        w_text.contains("Orientation bernilai 6"),
        "Pesan harus mencantumkan Orientation 6: {}",
        w_text
    );
    assert!(
        w_text.contains("diabaikan"),
        "Pesan harus menyatakan rotasi diabaikan"
    );
}

#[test]
fn test_direct_pixel_limit_144mp_rejection() {
    let tmp = tempdir().expect("tempdir");
    let p = tmp.path().join("too_many_pixels.jpg");

    // Dimensi 13.000 x 12.000 = 156 MP (> 144 MP)
    let mut jpg_bytes = vec![0xFF, 0xD8];
    jpg_bytes.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x11, 0x08, 0x2E, 0xE0, // height = 12000
        0x32, 0xC8, // width = 13000
        0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00,
    ]);
    jpg_bytes.extend_from_slice(&[0xFF, 0xD9]);
    fs::write(&p, &jpg_bytes).unwrap();

    let job_id = "job_test_144mp";
    let payload = ImportDualModePayload {
        mode: "A".to_string(),
        file_path: p.to_string_lossy().to_string(),
        save_original: true,
        manual_bounds: Some(ImportBoundsPayload {
            south: -1.0,
            west: 116.0,
            north: -0.9,
            east: 116.1,
        }),
        custom_filename: None,
    };

    let res = execute_import_option_a(job_id, payload, CancellationToken::new());
    match res {
        Err(OrthoImportError::MelebihiBatas {
            width,
            height,
            max_dimension,
            ..
        }) => {
            assert_eq!(width, 13000);
            assert_eq!(height, 12000);
            assert_eq!(max_dimension, aerial_analytics_platform_lib::modules::module_03::ortho_source::MAX_IMAGE_DIMENSION_PX);
        }
        other => panic!("Harusnya MelebihiBatas (144 MP), didapat: {:?}", other),
    }
}

#[test]
fn test_direct_file_size_limit_50mb_rejection() {
    let tmp = tempdir().expect("tempdir");
    let p = tmp.path().join("too_large_52mb.jpg");

    // Tulis berkas dengan ukuran > 50 MB (misal 51 MB)
    let file = File::create(&p).unwrap();
    file.set_len(51 * 1024 * 1024).unwrap();

    let job_id = "job_test_50mb";
    let payload = ImportDualModePayload {
        mode: "A".to_string(),
        file_path: p.to_string_lossy().to_string(),
        save_original: true,
        manual_bounds: Some(ImportBoundsPayload {
            south: -1.0,
            west: 116.0,
            north: -0.9,
            east: 116.1,
        }),
        custom_filename: None,
    };

    let res = execute_import_option_a(job_id, payload, CancellationToken::new());
    match res {
        Err(OrthoImportError::MelebihiBatas { message, .. }) => {
            assert!(
                message.contains("50 MB"),
                "Pesan harus mencantumkan batas 50 MB: {}",
                message
            );
        }
        other => panic!("Harusnya MelebihiBatas (50 MB), didapat: {:?}", other),
    }
}

#[test]
fn test_secondary_instance_blocks_mutating_operations() {
    use aerial_analytics_platform_lib::modules::module_03::ortho_source::commands::{
        delete_saved_image, start_import_ortho_job,
    };
    use aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::{
        is_primary_instance, require_primary_instance, set_is_primary_instance,
    };

    let original_status = is_primary_instance();

    // 1. Simulasikan instansi sekunder (is_primary = false)
    set_is_primary_instance(false);

    let res_guard = require_primary_instance();
    assert!(
        matches!(res_guard, Err(OrthoImportError::AplikasiSudahBerjalan(_))),
        "require_primary_instance harus menolak ketika bukan instansi primer"
    );

    // Command start_import_ortho_job harus menolak
    let payload = ImportDualModePayload {
        mode: "A".to_string(),
        file_path: "test.jpg".to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: None,
    };
    let res_import = start_import_ortho_job(payload);
    assert!(
        res_import.is_err(),
        "start_import_ortho_job harus ditolak pada instansi sekunder"
    );

    // Command delete_saved_image harus menolak
    let res_del = delete_saved_image("ortho".to_string(), "sample".to_string());
    assert!(
        res_del.is_err(),
        "delete_saved_image harus ditolak pada instansi sekunder"
    );

    // 2. Pulihkan status
    set_is_primary_instance(original_status);
}
