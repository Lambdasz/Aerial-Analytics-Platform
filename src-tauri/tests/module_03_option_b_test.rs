//! Integration & Unit Test Suite untuk Opsi B (GeoTIFF) Module 3.
//!
//! Menguji:
//! 1. Fast-fail validasi header (ukuran, signature magic bytes, batas megapiksel).
//! 2. Parsing georeferensi (ModelTiepointTag, ModelPixelScaleTag, GeoKeyDirectoryTag EPSG:32750 UTM 50S & EPSG:4326).
//! 3. Offset setengah piksel (RasterPixelIsPoint vs RasterPixelIsArea).
//! 4. Penolakan affine rotation/shear (ModelTransformationTag).
//! 5. Penanganan CRS tidak didukung (is_recoverable = true).
//! 6. Ekstraksi detail teknis (ExtraSamples, GDAL_NODATA, kompresi).
//! 7. Peregangan kontras deterministik 16-bit (p0,5 - p99,5) dan deteksi full-range.
//! 8. Alur penuh execute_import_option_b (staging, loop adaptif <= 5 MB, registrasi layer).
//! 9. Latensi pembatalan via CancellationToken (< 100 ms).

use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use aerial_analytics_platform_lib::modules::module_03::error::OrthoImportError;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::geotiff_reader::{
    compute_16bit_percentiles, extract_geotiff_georeference, extract_tiff_technical_details,
    validate_geotiff_header_fast,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::execute_import_option_b;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::job_manager::CancellationToken;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::types::ImportDualModePayload;
use aerial_analytics_platform_lib::modules::module_03::storage_utils::get_storage_root;
use tiff::decoder::Decoder;
use tiff::encoder::{colortype, TiffEncoder};
use tiff::tags::Tag;

/// Helper untuk membuat direktori pengujian terisolasi.
fn setup_test_env(test_name: &str) -> PathBuf {
    let base_dir = std::env::temp_dir().join(format!("aap_test_opt_b_{}", test_name));
    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&base_dir).unwrap();
    base_dir
}

/// Helper untuk membuat berkas GeoTIFF sintetis valid dengan georeferensi.
fn create_synthetic_geotiff(
    path: &Path,
    width: u32,
    height: u32,
    crs_code: u16,
    is_point_raster: bool,
    extra_samples_alpha: bool,
    nodata_val: Option<&str>,
) {
    let file = File::create(path).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();

    let mut img_encoder = encoder.new_image::<colortype::RGB8>(width, height).unwrap();

    // 1. ModelTiepointTag (33922): [I, J, K, X, Y, Z]
    // Misal: origin Balikpapan UTM 50S (E = 500.000 m, N = 9.866.000 m)
    let tiepoints = [0.0f64, 0.0, 0.0, 500000.0, 9866000.0, 0.0];
    img_encoder
        .encoder()
        .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
        .unwrap();

    // 2. ModelPixelScaleTag (33550): [ScaleX, ScaleY, ScaleZ]
    let pixel_scales = [1.0f64, 1.0, 0.0];
    img_encoder
        .encoder()
        .write_tag(Tag::ModelPixelScaleTag, &pixel_scales[..])
        .unwrap();

    // 3. GeoKeyDirectoryTag (34735)
    // Header: [1, 1, 0, 3] -> 3 keys
    // Key 1: GTModelTypeGeoKey (1024) = 1 (Projected)
    // Key 2: GTRasterTypeGeoKey (1025) = 1 (Area) atau 2 (Point)
    // Key 3: ProjectedCSTypeGeoKey (3072) = crs_code (e.g. 32750)
    let raster_type_val = if is_point_raster { 2u16 } else { 1u16 };
    let geokeys = [
        1u16,
        1,
        0,
        3, // Header
        1024,
        0,
        1,
        1, // ModelType = Projected
        1025,
        0,
        1,
        raster_type_val, // RasterType
        3072,
        0,
        1,
        crs_code, // Projected CRS
    ];
    img_encoder
        .encoder()
        .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
        .unwrap();

    // 4. Tag Tambahan Opsional
    if extra_samples_alpha {
        img_encoder
            .encoder()
            .write_tag(Tag::ExtraSamples, &[2u16][..]) // Unassociated Alpha
            .unwrap();
    }

    if let Some(nd) = nodata_val {
        img_encoder
            .encoder()
            .write_tag(Tag::GdalNodata, nd)
            .unwrap();
    }

    // Tulis piksel dummy (gradien sintetis)
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = 128u8;
            pixels.extend_from_slice(&[r, g, b]);
        }
    }

    img_encoder.write_data(&pixels).unwrap();
}

#[test]
fn test_geotiff_header_fast_fail_size_and_magic() {
    let test_dir = setup_test_env("fast_fail");
    let corrupt_file = test_dir.join("corrupt.tif");
    fs::write(&corrupt_file, b"NOT_A_TIFF_FILE").unwrap();

    let res = validate_geotiff_header_fast(&corrupt_file);
    assert!(res.is_err());
    match res {
        Err(OrthoImportError::FormatTidakDidukung(_)) => {}
        other => panic!("Harusnya FormatTidakDidukung, didapat: {:?}", other),
    }

    // Uji berkas TIFF valid
    let valid_file = test_dir.join("valid.tif");
    create_synthetic_geotiff(&valid_file, 100, 100, 32750, false, false, None);

    let summary = validate_geotiff_header_fast(&valid_file).unwrap();
    assert_eq!(summary.width, 100);
    assert_eq!(summary.height, 100);
    assert_eq!(summary.bit_depth, 8);
    assert_eq!(summary.samples_per_pixel, 3);
    assert!(!summary.has_alpha);
    assert_eq!(summary.nodata_value, None);
}

#[test]
fn test_geotiff_georeference_and_bounds_computation() {
    let test_dir = setup_test_env("georef");
    let tiff_file = test_dir.join("utm50s.tif");
    // Buat citra 100x100 di Balikpapan UTM 50S (EPSG:32750)
    create_synthetic_geotiff(&tiff_file, 100, 100, 32750, false, false, None);

    let file = File::open(&tiff_file).unwrap();
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap();

    let georef = extract_geotiff_georeference(&mut decoder, 100, 100).unwrap();
    assert_eq!(georef.crs_code, 32750);
    assert!(georef.crs_string.contains("EPSG:32750 (UTM Zone 50S)"));
    assert!(!georef.is_point_raster);

    let [[south, west], [north, east]] = georef.bounds;
    // Balikpapan terletak di ~ -1.2° Lintang Selatan dan ~ 117.0° Bujur Timur
    assert!(south < north);
    assert!(west < east);
    assert!(south >= -2.0 && north <= 0.0);
    assert!(west >= 116.0 && east <= 118.0);
}

#[test]
fn test_geotiff_half_pixel_point_raster_shift() {
    let test_dir = setup_test_env("point_raster");
    let file_area = test_dir.join("area.tif");
    let file_point = test_dir.join("point.tif");

    create_synthetic_geotiff(&file_area, 100, 100, 32750, false, false, None);
    create_synthetic_geotiff(&file_point, 100, 100, 32750, true, false, None);

    let mut dec_area = Decoder::new(BufReader::new(File::open(&file_area).unwrap())).unwrap();
    let mut dec_point = Decoder::new(BufReader::new(File::open(&file_point).unwrap())).unwrap();

    let georef_area = extract_geotiff_georeference(&mut dec_area, 100, 100).unwrap();
    let georef_point = extract_geotiff_georeference(&mut dec_point, 100, 100).unwrap();

    assert!(!georef_area.is_point_raster);
    assert!(georef_point.is_point_raster);

    // Titik Point raster harus memiliki batas koordinat yang tergeser setengah piksel ke luar
    let area_b = georef_area.bounds;
    let point_b = georef_point.bounds;

    assert!(point_b[0][1] < area_b[0][1]); // west point < west area
    assert!(point_b[1][0] > area_b[1][0]); // north point > north area
}

#[test]
fn test_geotiff_affine_rotation_rejected() {
    let test_dir = setup_test_env("rotation");
    let tiff_file = test_dir.join("rotated.tif");

    let file = File::create(&tiff_file).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();
    let mut img = encoder.new_image::<colortype::RGB8>(50, 50).unwrap();

    // Matriks rotasi 4x4 non-aksial (M01 = 0.5)
    let transform = [
        1.0, 0.5, 0.0, 500000.0, -0.5, -1.0, 0.0, 9866000.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    img.encoder()
        .write_tag(Tag::ModelTransformationTag, &transform[..])
        .unwrap();
    img.write_data(&vec![0u8; 50 * 50 * 3]).unwrap();

    let mut dec = Decoder::new(BufReader::new(File::open(&tiff_file).unwrap())).unwrap();
    let res = extract_geotiff_georeference(&mut dec, 50, 50);

    assert!(res.is_err());
    match res {
        Err(OrthoImportError::RotasiTidakDidukung(_)) => {}
        other => panic!("Harusnya RotasiTidakDidukung, didapat: {:?}", other),
    }
}

#[test]
fn test_geotiff_unsupported_crs_is_recoverable() {
    let test_dir = setup_test_env("unsupported_crs");
    let tiff_file = test_dir.join("unknown_crs.tif");

    // Gunakan kode EPSG yang tidak terdaftar: 9999
    create_synthetic_geotiff(&tiff_file, 50, 50, 9999, false, false, None);

    let mut dec = Decoder::new(BufReader::new(File::open(&tiff_file).unwrap())).unwrap();
    let res = extract_geotiff_georeference(&mut dec, 50, 50);

    assert!(res.is_err());
    match res {
        Err(err @ OrthoImportError::CrsTidakDidukung(_)) => {
            assert!(
                err.is_recoverable(),
                "Error CRS tidak didukung harus recoverable"
            );
        }
        other => panic!("Harusnya CrsTidakDidukung, didapat: {:?}", other),
    }
}

#[test]
fn test_geotiff_alpha_and_nodata_detection() {
    let test_dir = setup_test_env("tags");
    let tiff_file = test_dir.join("alpha_nodata.tif");

    create_synthetic_geotiff(&tiff_file, 50, 50, 32750, false, true, Some("-9999.0"));

    let mut dec = Decoder::new(BufReader::new(File::open(&tiff_file).unwrap())).unwrap();
    let details = extract_tiff_technical_details(&mut dec, 50, 50).unwrap();

    assert!(details.has_alpha);
    assert_eq!(details.nodata_value, Some(-9999.0));
}

#[test]
fn test_geotiff_16bit_percentile_stretch_deterministic() {
    let test_dir = setup_test_env("u16_stretch");
    let tiff_file = test_dir.join("gray16.tif");

    let file = File::create(&tiff_file).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();
    let img = encoder.new_image::<colortype::Gray16>(100, 100).unwrap();

    // Isi piksel dengan rentang 12-bit sensor drone: 2000 sampai 10000
    let mut pixels = Vec::with_capacity(100 * 100);
    for y in 0..100 {
        for x in 0..100 {
            let val = 2000u16 + (((x + y) * 8000) / 200) as u16;
            pixels.push(val);
        }
    }
    img.write_data(&pixels).unwrap();

    let mut dec = Decoder::new(BufReader::new(File::open(&tiff_file).unwrap())).unwrap();
    let (lo, hi, is_full_range) =
        compute_16bit_percentiles(&mut dec, 100, 100, false, None).unwrap();

    assert!(!is_full_range);
    assert!((1900.0..=2500.0).contains(&lo));
    assert!((9500.0..=10100.0).contains(&hi));
}

#[test]
fn test_execute_import_option_b_end_to_end() {
    let test_dir = setup_test_env("e2e");
    let tiff_file = test_dir.join("drone_balpapan.tif");

    create_synthetic_geotiff(&tiff_file, 200, 200, 32750, false, false, None);

    let job_id = format!("job_opt_b_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: tiff_file.to_string_lossy().to_string(),
        save_original: true,
        manual_bounds: None,
        custom_filename: Some("Balikpapan Port Ortho".to_string()),
    };

    let token = CancellationToken::new();
    let start_time = Instant::now();

    let result = execute_import_option_b(&job_id, payload, token);
    let duration = start_time.elapsed();

    assert!(result.is_ok(), "Impor Opsi B harus berhasil: {:?}", result);
    let meta = result.unwrap();

    println!(
        "Impor Opsi B Berhasil! ID: {}, Waktu Eksekusi: {:.2?}",
        meta.id, duration
    );

    assert_eq!(meta.filename, "Balikpapan Port Ortho");
    assert_eq!(meta.dimensions, Some([200, 200]));
    assert!(meta.display_size_bytes.unwrap() <= 5 * 1024 * 1024);

    // Pastikan folder final ada dan berisi metadata.json
    let image_dir = get_storage_root().join("images").join(&meta.id);
    assert!(image_dir.exists());
    assert!(image_dir.join("metadata.json").exists());
    assert!(image_dir.join("meta.json").exists());
    assert!(image_dir.join("original.tif").exists());

    // Bersihkan artifak uji
    let _ = fs::remove_dir_all(&image_dir);
    let _ = fs::remove_file(
        get_storage_root()
            .join("layers")
            .join(format!("{}.json", meta.id)),
    );
    let _ = fs::remove_file(
        get_storage_root()
            .join(".jobs")
            .join(format!("{}.json", job_id)),
    );
}

#[test]
fn test_geotiff_cancellation_latency() {
    let test_dir = setup_test_env("cancel");
    let tiff_file = test_dir.join("cancel_test.tif");

    create_synthetic_geotiff(&tiff_file, 100, 100, 32750, false, false, None);

    let job_id = format!("job_cancel_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: tiff_file.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: None,
    };

    let token = CancellationToken::new();
    token.cancel(); // Langsung batalkan sebelum dieksekusi

    let start = Instant::now();
    let result = execute_import_option_b(&job_id, payload, token);
    let latency = start.elapsed();

    println!("Latensi Pembatalan Opsi B: {:.2?}", latency);
    assert!(
        latency.as_millis() < 100,
        "Latensi pembatalan harus di bawah 100 ms (aktual: {:?})",
        latency
    );

    assert!(result.is_err());
    match result {
        Err(OrthoImportError::Dibatalkan) => {}
        other => panic!("Harusnya Dibatalkan, didapat: {:?}", other),
    }

    // Pastikan staging folder langsung bersih
    let staging_dir = get_storage_root()
        .join("images")
        .join(format!(".staging_{}", job_id));
    assert!(
        !staging_dir.exists(),
        "Folder staging harus bersih setelah pembatalan"
    );
}

#[cfg(windows)]
fn get_process_memory_counters() -> (u64, u64) {
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
            hProcess: isize,
            ppsmemCounters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }
    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        let h = GetCurrentProcess();
        if K32GetProcessMemoryInfo(h, &mut counters, counters.cb) != 0 {
            (
                counters.peak_working_set_size as u64,
                counters.peak_pagefile_usage as u64,
            )
        } else {
            (0, 0)
        }
    }
}

#[test]
fn test_real_oam_balikpapan_geotiff_inspection_and_import() {
    let tiff_candidates = [
        PathBuf::from(r"C:\Users\x0776\Downloads\600fcc9055481e0005e3129e (1).tif"),
        PathBuf::from(r"C:\Users\x0776\Downloads\600fcc9055481e0005e3129e.tif"),
    ];

    let tiff_path = match tiff_candidates.iter().find(|p| p.exists()) {
        Some(p) => p.clone(),
        None => {
            println!(
                "Berkas GeoTIFF Balikpapan nyata tidak ditemukan di Downloads. Melewati tes nyata."
            );
            return;
        }
    };

    println!("\n=== PENGUJIAN GEOTIFF NYATA BALIKPAPAN ===");
    println!("Lokasi Berkas: {:?}", tiff_path);

    // 1. Validasi fast-fail header
    let header = validate_geotiff_header_fast(&tiff_path).unwrap();
    let header_w = header.width;
    let header_h = header.height;
    let file_size = fs::metadata(&tiff_path).unwrap().len();

    // 2. Ekstraksi detail teknis & georeferensi
    let file = File::open(&tiff_path).unwrap();
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap();

    let details = extract_tiff_technical_details(&mut decoder, header_w, header_h).unwrap();
    let georef = extract_geotiff_georeference(&mut decoder, header_w, header_h).unwrap();

    println!("Detail Berkas GeoTIFF:");
    println!(
        "- Ukuran File: {} bytes ({:.2} MiB)",
        file_size,
        file_size as f64 / (1024.0 * 1024.0)
    );
    println!("- Dimensi Asli: {} x {} px", header_w, header_h);
    println!("- Kompresi: {}", details.compression);
    println!(
        "- Layout: {} (Dimensi Chunk: {:?}, Total Chunks: {})",
        details.layout, details.chunk_dimensions, details.total_chunks
    );
    println!("- CRS: {} (Kode: {})", georef.crs_string, georef.crs_code);
    println!("- Bounding Box WGS84: {:?}", georef.bounds);
    println!("- Jumlah Band: {}", details.bands);
    println!("- Bit Depth: {}", details.bit_depth);
    println!("- Has Alpha: {}", details.has_alpha);
    println!("- Nodata Value: {:?}", details.nodata_value);

    // 3. Ukur Memori Baseline Sebelum Eksekusi
    #[cfg(windows)]
    let (ws_before, pf_before) = get_process_memory_counters();

    // 4. Jalankan Pipeline Impor Opsi B
    let job_id = format!(
        "job_real_balikpapan_{}",
        chrono::Utc::now().timestamp_millis()
    );
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: tiff_path.to_string_lossy().to_string(),
        save_original: false, // uji pipeline dengan display tanpa duplikasi berkas fisik
        manual_bounds: None,
        custom_filename: Some("Balikpapan Real Ortho".to_string()),
    };

    let token = CancellationToken::new();
    let start_time = Instant::now();
    let result = execute_import_option_b(&job_id, payload, token);
    let duration = start_time.elapsed();

    assert!(
        result.is_ok(),
        "Impor citra nyata Balikpapan harus berhasil: {:?}",
        result
    );
    let meta = result.unwrap();

    // 5. Ukur Memori Puncak Setelah Eksekusi
    #[cfg(windows)]
    let (ws_peak, pf_peak) = get_process_memory_counters();

    #[cfg(windows)]
    {
        println!("\nPengukuran Memori (Windows K32GetProcessMemoryInfo):");
        println!(
            "- Baseline Peak WorkingSet: {:.2} MiB",
            ws_before as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Baseline Peak Pagefile: {:.2} MiB",
            pf_before as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak WorkingSet (RAM Fisik): {:.2} MiB",
            ws_peak as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak Pagefile (Private Commit): {:.2} MiB",
            pf_peak as f64 / (1024.0 * 1024.0)
        );
        println!("- Waktu Eksekusi: {:.2?}", duration);
        assert!(
            ws_peak <= 400 * 1024 * 1024,
            "Puncak Working Set harus <= 400 MiB (aktual: {:.2} MiB)",
            ws_peak as f64 / (1024.0 * 1024.0)
        );
    }

    let image_dir = get_storage_root().join("images").join(&meta.id);
    let display_file = image_dir.join(format!("display.{}", meta.extension));
    let display_dims = image::image_dimensions(&display_file).unwrap_or((0, 0));

    println!("\nHasil Display Akhir:");
    println!("- ID: {}", meta.id);
    println!("- Ekstensi Display: {}", meta.extension);
    println!("- Resolusi Citra Asli: {:?}", meta.dimensions);
    println!(
        "- Resolusi Display Akhir: {} x {} px",
        display_dims.0, display_dims.1
    );
    println!(
        "- Ukuran Berkas Display: {} bytes ({:.2} MiB)",
        meta.display_size_bytes.unwrap_or(0),
        meta.display_size_bytes.unwrap_or(0) as f64 / (1024.0 * 1024.0)
    );

    if meta.extension == "png" && details.has_alpha {
        let max_dim = display_dims.0.max(display_dims.1);
        println!("- Resolusi Display PNG Alpha: {} px", max_dim);
        if max_dim < 2048 {
            println!(
                "PERINGATAN KRUSIAL: Resolusi display PNG alpha < 2.048 px ({} px).",
                max_dim
            );
        }
    }

    // Bersihkan data uji dari map_storage
    let image_dir = get_storage_root().join("images").join(&meta.id);
    let _ = fs::remove_dir_all(&image_dir);
    let _ = fs::remove_file(
        get_storage_root()
            .join("layers")
            .join(format!("{}.json", meta.id)),
    );
    let _ = fs::remove_file(
        get_storage_root()
            .join(".jobs")
            .join(format!("{}.json", job_id)),
    );
}
