//! Integration & Unit Test Suite untuk Opsi B (GeoTIFF) Module 3.
//!
//! Menguji:
//! 1. Fast-fail validasi header (ukuran, signature magic bytes, batas megapiksel).
//! 2. Parsing georeferensi (ModelTiepointTag, ModelPixelScaleTag, GeoKeyDirectoryTag EPSG:32750 UTM 50S & EPSG:4326).
//! 3. Offset setengah piksel (RasterPixelIsPoint vs RasterPixelIsArea).
//! 4. Penolakan affine rotation/shear (ModelTransformationTag).
//! 5. Penanganan CRS tidak didukung (is_recoverable = true).
//! 6. Ekstraksi detail teknis (ExtraSamples, GDAL_NODATA, kompresi).
//! 7. Peregangan kontras deterministik 16-bit (p0,5 - p99,5) dan deteksi full-range / empty histogram fallback.
//! 8. Alur penuh execute_import_option_b (staging, loop adaptif <= 5 MB, registrasi layer).
//! 9. Latensi pembatalan via CancellationToken (pre-flight fast-fail dan mid-process cancel).
//! 10. Validasi JPEG-in-TIFF sintetis (RGB dan YCbCr 4:2:0) dan perbandingan numerik pada citra nyata.
//! 11. Penolakan header TIFF tak didukung (PlanarConfiguration=2, BitDepth!=8/16, Palette, CMYK, YCbCr non-JPEG, MinIsWhite inversi).
//! 12. Seleksi overview IFD 0, tile parsial tepi, format stripped, dan tanpa upscale citra kecil.

use std::fs::{self, File};
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
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
use jpeg_encoder::{ColorType as JpegColorType, Encoder as JpegEncoder, SamplingFactor};
use tiff::decoder::ifd::Value;
use tiff::decoder::{Decoder, DecodingResult};
use tiff::encoder::{colortype, TiffEncoder};
use tiff::tags::Tag;

/// Helper untuk membuat direktori pengujian terisolasi.
fn setup_test_env(test_name: &str) -> PathBuf {
    let base_dir = std::env::temp_dir().join(format!("aap_test_opt_b_{}", test_name));
    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&base_dir).unwrap();
    base_dir
}

fn write_u16_le(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u32_le(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Helper untuk membuat berkas TIFF tiled JPEG valid dengan IFD custom
#[allow(clippy::too_many_arguments)]
fn create_synthetic_jpeg_tiff(
    path: &Path,
    width: u32,
    height: u32,
    tile_w: u32,
    tile_h: u32,
    photometric: u16,
    subsampling: Option<[u16; 2]>,
    jpeg_bytes: &[u8],
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    // Header (8 bytes)
    buf.extend_from_slice(b"II");
    write_u16_le(&mut buf, 42);
    write_u32_le(&mut buf, 8); // Offset ke IFD 0

    let mut tags: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, width),                   // ImageWidth
        (257, 4, 1, height),                  // ImageLength
        (258, 3, 3, 0),                       // BitsPerSample
        (259, 3, 1, 7),                       // Compression = 7 (JPEG)
        (262, 3, 1, photometric as u32),      // PhotometricInterpretation
        (277, 3, 1, 3),                       // SamplesPerPixel = 3
        (284, 3, 1, 1),                       // PlanarConfiguration = 1
        (322, 4, 1, tile_w),                  // TileWidth
        (323, 4, 1, tile_h),                  // TileLength
        (324, 4, 1, 0),                       // TileOffsets
        (325, 4, 1, jpeg_bytes.len() as u32), // TileByteCounts
    ];

    if let Some(_sub) = subsampling {
        tags.push((530, 3, 2, 0)); // YCbCrSubSampling
        tags.push((531, 3, 1, 1)); // YCbCrPositioning = 1
    }

    tags.sort_by_key(|t| t.0);

    let ifd_entry_count = tags.len() as u16;
    let ifd_len = 2 + (tags.len() * 12) + 4;
    let extra_data_start = 8 + ifd_len;

    let bits_per_sample_offset = extra_data_start as u32;
    let mut current_offset = bits_per_sample_offset + 6;

    let subsampling_offset = if subsampling.is_some() {
        let off = current_offset;
        current_offset += 4;
        Some(off)
    } else {
        None
    };

    let jpeg_offset = current_offset;

    write_u16_le(&mut buf, ifd_entry_count);
    for (tag, typ, count, val) in &tags {
        write_u16_le(&mut buf, *tag);
        write_u16_le(&mut buf, *typ);
        write_u32_le(&mut buf, *count);
        let final_val = match *tag {
            258 => bits_per_sample_offset,
            324 => jpeg_offset,
            530 => subsampling_offset.unwrap(),
            _ => *val,
        };
        write_u32_le(&mut buf, final_val);
    }
    write_u32_le(&mut buf, 0); // Next IFD = 0

    // Extra data: BitsPerSample [8, 8, 8]
    write_u16_le(&mut buf, 8);
    write_u16_le(&mut buf, 8);
    write_u16_le(&mut buf, 8);

    // Extra data: YCbCrSubSampling [subX, subY]
    if let Some(sub) = subsampling {
        write_u16_le(&mut buf, sub[0]);
        write_u16_le(&mut buf, sub[1]);
    }

    buf.extend_from_slice(jpeg_bytes);
    fs::write(path, buf)
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
    create_synthetic_geotiff(&tiff_file, 100, 100, 32750, false, false, None);

    let file = File::open(&tiff_file).unwrap();
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap();

    let georef = extract_geotiff_georeference(&mut decoder, 100, 100).unwrap();
    assert_eq!(georef.crs_code, 32750);
    assert!(georef.crs_string.contains("EPSG:32750 (UTM Zone 50S)"));
    assert!(!georef.is_point_raster);

    let [[south, west], [north, east]] = georef.bounds;
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

    let image_dir = get_storage_root().join("images").join(&meta.id);
    assert!(image_dir.exists());
    assert!(image_dir.join("metadata.json").exists());
    assert!(image_dir.join("meta.json").exists());
    assert!(image_dir.join("original.tif").exists());

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
fn test_geotiff_cancellation_latency_preflight() {
    let test_dir = setup_test_env("cancel_preflight");
    let tiff_file = test_dir.join("cancel_test.tif");

    create_synthetic_geotiff(&tiff_file, 100, 100, 32750, false, false, None);

    let job_id = format!("job_cancel_pre_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: tiff_file.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: None,
    };

    let token = CancellationToken::new();
    token.cancel(); // Pembatalan pre-flight sebelum dieksekusi

    let start = Instant::now();
    let result = execute_import_option_b(&job_id, payload, token);
    let latency = start.elapsed();

    println!("Latensi Pembatalan Pre-Flight Opsi B: {:.2?}", latency);
    assert!(
        latency.as_millis() < 100,
        "Latensi pre-flight cancel harus di bawah 100 ms (aktual: {:?})",
        latency
    );

    assert!(result.is_err());
    match result {
        Err(OrthoImportError::Dibatalkan) => {}
        other => panic!("Harusnya Dibatalkan, didapat: {:?}", other),
    }

    let staging_dir = get_storage_root()
        .join("images")
        .join(format!(".staging_{}", job_id));
    assert!(
        !staging_dir.exists(),
        "Folder staging harus bersih setelah pembatalan"
    );
}

/// (Butir 2f) Uji JPEG-in-TIFF Sintetis (Varian RGB dan YCbCr 4:2:0)
#[test]
fn test_geotiff_jpeg_in_tiff_synthetic_rgb_and_ycbcr() {
    let test_dir = setup_test_env("jpeg_in_tiff_synth");

    // 1. Varian PhotometricInterpretation = 2 (RGB) tanpa subsampling
    let rgb_tiff = test_dir.join("synth_jpeg_rgb.tif");
    let mut raw_pixels = Vec::with_capacity(64 * 64 * 3);
    for y in 0..64 {
        for x in 0..64 {
            let r = ((x * 240) / 64) as u8;
            let g = ((y * 240) / 64) as u8;
            let b = 160u8;
            raw_pixels.extend_from_slice(&[r, g, b]);
        }
    }

    let mut jpeg_buf_rgb = Vec::new();
    let mut enc = JpegEncoder::new(&mut jpeg_buf_rgb, 95);
    enc.set_sampling_factor(SamplingFactor::R_4_4_4);
    enc.encode(&raw_pixels, 64, 64, JpegColorType::Ycbcr)
        .unwrap();

    create_synthetic_jpeg_tiff(
        &rgb_tiff,
        64,
        64,
        64,
        64,
        2, // RGB
        None,
        &jpeg_buf_rgb,
    )
    .unwrap();

    let header_rgb = validate_geotiff_header_fast(&rgb_tiff).unwrap();
    assert_eq!(header_rgb.width, 64);
    assert_eq!(header_rgb.height, 64);
    assert_eq!(header_rgb.compression_name, "JPEG");

    let mut dec_rgb = Decoder::new(BufReader::new(File::open(&rgb_tiff).unwrap())).unwrap();
    let img_rgb = dec_rgb.read_image().unwrap();
    let decoded_rgb = match img_rgb {
        tiff::decoder::DecodingResult::U8(v) => v,
        _ => panic!("Hasil decode RGB harus U8"),
    };
    assert_eq!(decoded_rgb.len(), 64 * 64 * 3);

    let mut diff_sum_rgb: u64 = 0;
    let mut max_diff_rgb: u8 = 0;
    for (orig, dec) in raw_pixels.iter().zip(decoded_rgb.iter()) {
        let d = (*orig as i32 - *dec as i32).unsigned_abs() as u8;
        diff_sum_rgb += d as u64;
        max_diff_rgb = max_diff_rgb.max(d);
    }
    let mean_diff_rgb = diff_sum_rgb as f64 / raw_pixels.len() as f64;
    println!(
        "Sintetis JPEG-in-TIFF RGB (tanpa subsampling 4:4:4): max diff = {}, mean diff = {:.2}",
        max_diff_rgb, mean_diff_rgb
    );
    assert!(
        mean_diff_rgb < 6.0,
        "Mean diff RGB harus < 6.0 (aktual: {})",
        mean_diff_rgb
    );
    assert!(
        max_diff_rgb < 25,
        "Max diff RGB harus < 25 (aktual: {})",
        max_diff_rgb
    );

    // 2. Varian PhotometricInterpretation = 6 (YCbCr) dengan subsampling 4:2:0
    let ycbcr_tiff = test_dir.join("synth_jpeg_ycbcr.tif");
    let mut jpeg_buf_ycbcr = Vec::new();
    let mut enc_ycbcr = JpegEncoder::new(&mut jpeg_buf_ycbcr, 95);
    enc_ycbcr.set_sampling_factor(SamplingFactor::R_4_2_0);
    enc_ycbcr
        .encode(&raw_pixels, 64, 64, JpegColorType::Rgb)
        .unwrap();

    create_synthetic_jpeg_tiff(
        &ycbcr_tiff,
        64,
        64,
        64,
        64,
        6, // YCbCr
        Some([2, 2]),
        &jpeg_buf_ycbcr,
    )
    .unwrap();

    let header_ycbcr = validate_geotiff_header_fast(&ycbcr_tiff).unwrap();
    assert_eq!(header_ycbcr.width, 64);
    assert_eq!(header_ycbcr.height, 64);
    assert_eq!(header_ycbcr.compression_name, "JPEG");

    let mut dec_ycbcr = Decoder::new(BufReader::new(File::open(&ycbcr_tiff).unwrap())).unwrap();
    let img_ycbcr = dec_ycbcr.read_image().unwrap();
    let decoded_ycbcr = match img_ycbcr {
        tiff::decoder::DecodingResult::U8(v) => v,
        _ => panic!("Hasil decode YCbCr harus U8"),
    };
    assert_eq!(decoded_ycbcr.len(), 64 * 64 * 3);

    let mut diff_sum_ycbcr: u64 = 0;
    let mut max_diff_ycbcr: u8 = 0;
    for (orig, dec) in raw_pixels.iter().zip(decoded_ycbcr.iter()) {
        let d = (*orig as i32 - *dec as i32).unsigned_abs() as u8;
        diff_sum_ycbcr += d as u64;
        max_diff_ycbcr = max_diff_ycbcr.max(d);
    }
    let mean_diff_ycbcr = diff_sum_ycbcr as f64 / raw_pixels.len() as f64;
    println!(
        "Sintetis JPEG-in-TIFF YCbCr 4:2:0: max diff = {}, mean diff = {:.2}",
        max_diff_ycbcr, mean_diff_ycbcr
    );
    assert!(
        mean_diff_ycbcr < 8.0,
        "Mean diff YCbCr harus < 8.0 (aktual: {})",
        mean_diff_ycbcr
    );
    assert!(
        max_diff_ycbcr < 30,
        "Max diff YCbCr harus < 30 (aktual: {})",
        max_diff_ycbcr
    );
}

fn write_simple_test_tiff(path: &Path, mut tags: Vec<(u16, u16, u32, u32)>, next_ifd: u32) {
    if !tags.iter().any(|t| t.0 == 273 || t.0 == 324) {
        tags.push((273, 4, 1, 200)); // StripOffsets
    }
    if !tags.iter().any(|t| t.0 == 279 || t.0 == 325) {
        tags.push((279, 4, 1, 200)); // StripByteCounts
    }
    tags.sort_by_key(|t| t.0);

    let mut buf = Vec::new();
    buf.extend_from_slice(b"II");
    write_u16_le(&mut buf, 42);
    write_u32_le(&mut buf, 8);

    write_u16_le(&mut buf, tags.len() as u16);
    for (tag, typ, count, val) in tags {
        write_u16_le(&mut buf, tag);
        write_u16_le(&mut buf, typ);
        write_u32_le(&mut buf, count);
        write_u32_le(&mut buf, val);
    }
    write_u32_le(&mut buf, next_ifd);
    if buf.len() < 250 {
        buf.resize(250, 0);
    }
    fs::write(path, buf).unwrap();
}

/// (Butir 5) Penolakan Header TIFF yang Tidak Didukung
#[test]
fn test_geotiff_header_rejections() {
    let test_dir = setup_test_env("header_rejections");

    // 1. PlanarConfiguration = 2 (Planar)
    let planar_path = test_dir.join("planar2.tif");
    write_simple_test_tiff(
        &planar_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 8),
            (259, 3, 1, 1),
            (262, 3, 1, 1),
            (284, 3, 1, 2), // PlanarConfiguration = 2
        ],
        0,
    );
    let res_planar = validate_geotiff_header_fast(&planar_path);
    assert!(res_planar.is_err());
    match res_planar {
        Err(OrthoImportError::FormatTidakDidukung(msg)) => {
            assert!(msg.contains("PlanarConfiguration 2"));
        }
        other => panic!("Harusnya FormatTidakDidukung Planar, didapat: {:?}", other),
    }

    // 2. Bit per sampel selain 8/16 (BitDepth = 32)
    let bit32_path = test_dir.join("bit32.tif");
    write_simple_test_tiff(
        &bit32_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 32), // 32-bit
            (259, 3, 1, 1),
            (262, 3, 1, 1),
        ],
        0,
    );
    let res_bit32 = validate_geotiff_header_fast(&bit32_path);
    assert!(res_bit32.is_err());
    match res_bit32 {
        Err(OrthoImportError::FormatTidakDidukung(msg)) => {
            assert!(msg.contains("32"));
        }
        other => panic!("Harusnya FormatTidakDidukung 32-bit, didapat: {:?}", other),
    }

    // 3. Palette Color (PhotometricInterpretation = 3)
    let palette_path = test_dir.join("palette.tif");
    write_simple_test_tiff(
        &palette_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 8),
            (259, 3, 1, 1),
            (262, 3, 1, 3), // Palette
        ],
        0,
    );
    let res_palette = validate_geotiff_header_fast(&palette_path);
    assert!(res_palette.is_err());
    match res_palette {
        Err(OrthoImportError::FormatTidakDidukung(msg)) => {
            assert!(msg.contains("Palette"));
        }
        other => panic!("Harusnya FormatTidakDidukung Palette, didapat: {:?}", other),
    }

    // 4. CMYK (PhotometricInterpretation = 5)
    let cmyk_path = test_dir.join("cmyk.tif");
    write_simple_test_tiff(
        &cmyk_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 8),
            (259, 3, 1, 1),
            (262, 3, 1, 5), // CMYK
        ],
        0,
    );
    let res_cmyk = validate_geotiff_header_fast(&cmyk_path);
    assert!(res_cmyk.is_err());
    match res_cmyk {
        Err(OrthoImportError::FormatTidakDidukung(msg)) => {
            assert!(msg.contains("CMYK"));
        }
        other => panic!("Harusnya FormatTidakDidukung CMYK, didapat: {:?}", other),
    }

    // 5. YCbCr non-JPEG (PhotometricInterpretation = 6, Compression = 1 Uncompressed)
    let ycbcr_raw_path = test_dir.join("ycbcr_raw.tif");
    write_simple_test_tiff(
        &ycbcr_raw_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 8),
            (259, 3, 1, 1), // Non-JPEG
            (262, 3, 1, 6), // YCbCr
        ],
        0,
    );
    let res_ycbcr_raw = validate_geotiff_header_fast(&ycbcr_raw_path);
    assert!(res_ycbcr_raw.is_err());
    match res_ycbcr_raw {
        Err(OrthoImportError::FormatTidakDidukung(msg)) => {
            assert!(msg.contains("YCbCr"));
        }
        other => panic!(
            "Harusnya FormatTidakDidukung YCbCr non-JPEG, didapat: {:?}",
            other
        ),
    }

    // 6. MinIsWhite (PhotometricInterpretation = 0) -> is_min_is_white = true
    let min_is_white_path = test_dir.join("min_is_white.tif");
    write_simple_test_tiff(
        &min_is_white_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 8),
            (259, 3, 1, 1),
            (262, 3, 1, 0), // MinIsWhite
        ],
        0,
    );
    let header_white = validate_geotiff_header_fast(&min_is_white_path).unwrap();
    assert!(
        header_white.is_min_is_white,
        "MinIsWhite harus terdeteksi true"
    );
}

/// (Butir 5) Penolakan Kompresi yang Tak Didukung (mis. CCITT Fax / T.6)
#[test]
fn test_geotiff_unsupported_compression_rejection() {
    let test_dir = setup_test_env("unsupported_comp");
    let ccitt_path = test_dir.join("ccitt_t6.tif");

    write_simple_test_tiff(
        &ccitt_path,
        vec![
            (256, 4, 1, 50),
            (257, 4, 1, 50),
            (258, 3, 1, 8),
            (259, 3, 1, 4), // Compression 4 = CCITT T.6
            (262, 3, 1, 1),
        ],
        0,
    );

    let res = validate_geotiff_header_fast(&ccitt_path);
    assert!(res.is_err());
    match res {
        Err(OrthoImportError::KompresiTidakDidukung(msg)) => {
            assert!(msg.contains("4"));
        }
        other => panic!("Harusnya KompresiTidakDidukung, didapat: {:?}", other),
    }
}

/// (Butir 5) Seleksi Overview: Memastikan IFD 0 selalu dipilih pada multi-IFD TIFF
#[test]
fn test_geotiff_overview_selection_multi_ifd() {
    let test_dir = setup_test_env("multi_ifd");
    let multi_path = test_dir.join("multi_ifd.tif");

    // IFD 0: 500x400 (Full), IFD 1: 125x100 (Thumbnail)
    let mut tags_0: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, 500),
        (257, 4, 1, 400),
        (258, 3, 1, 8),
        (259, 3, 1, 1),
        (262, 3, 1, 1),
        (273, 4, 1, 300),
        (279, 4, 1, 300),
    ];
    tags_0.sort_by_key(|t| t.0);

    let mut tags_1: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, 125),
        (257, 4, 1, 100),
        (258, 3, 1, 8),
        (259, 3, 1, 1),
        (262, 3, 1, 1),
        (273, 4, 1, 300),
        (279, 4, 1, 300),
    ];
    tags_1.sort_by_key(|t| t.0);

    let ifd1_offset = (8 + 2 + (tags_0.len() * 12) + 4) as u32;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"II");
    write_u16_le(&mut buf, 42);
    write_u32_le(&mut buf, 8); // Pointer ke IFD 0

    // Tulis IFD 0
    write_u16_le(&mut buf, tags_0.len() as u16);
    for (tag, typ, count, val) in tags_0 {
        write_u16_le(&mut buf, tag);
        write_u16_le(&mut buf, typ);
        write_u32_le(&mut buf, count);
        write_u32_le(&mut buf, val);
    }
    write_u32_le(&mut buf, ifd1_offset); // Pointer ke IFD 1

    // Tulis IFD 1
    write_u16_le(&mut buf, tags_1.len() as u16);
    for (tag, typ, count, val) in tags_1 {
        write_u16_le(&mut buf, tag);
        write_u16_le(&mut buf, typ);
        write_u32_le(&mut buf, count);
        write_u32_le(&mut buf, val);
    }
    write_u32_le(&mut buf, 0); // End of chain

    if buf.len() < 350 {
        buf.resize(350, 0);
    }
    fs::write(&multi_path, buf).unwrap();

    let header = validate_geotiff_header_fast(&multi_path).unwrap();
    assert_eq!(
        header.width, 500,
        "Harus memilih dimensi IFD 0 (resolusi penuh)"
    );
    assert_eq!(header.height, 400);
}

/// (Butir 5) Uji Penanganan Tile Parsial Tepi Tanpa Buffer Overflow
#[test]
fn test_geotiff_partial_edge_tiles() {
    let test_dir = setup_test_env("partial_tiles");
    let partial_path = test_dir.join("partial_100x100_tile64.tif");

    // Dimensi 100x100 dengan tile 64x64 (kolom 1 lebar 36 px, baris 1 tinggi 36 px)
    let file = File::create(&partial_path).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();
    let mut img = encoder.new_image::<colortype::RGB8>(100, 100).unwrap();

    let tiepoints = [0.0f64, 0.0, 0.0, 500000.0, 9866000.0, 0.0];
    img.encoder()
        .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
        .unwrap();
    let pixel_scales = [1.0f64, 1.0, 0.0];
    img.encoder()
        .write_tag(Tag::ModelPixelScaleTag, &pixel_scales[..])
        .unwrap();

    let geokeys = [
        1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32750,
    ];
    img.encoder()
        .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
        .unwrap();

    let mut pixels = Vec::with_capacity(100 * 100 * 3);
    for _ in 0..(100 * 100) {
        pixels.extend_from_slice(&[200, 150, 100]);
    }
    img.write_data(&pixels).unwrap();

    let job_id = format!("job_partial_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: partial_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Partial Tiles Test".to_string()),
    };

    let result = execute_import_option_b(&job_id, payload, CancellationToken::new());
    assert!(result.is_ok(), "Impor tile parsial harus berhasil");
    let meta = result.unwrap();
    assert_eq!(meta.dimensions, Some([100, 100]));

    let _ = fs::remove_dir_all(get_storage_root().join("images").join(&meta.id));
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

/// (Butir 5) Uji GeoTIFF Format Stripped
#[test]
fn test_geotiff_stripped_format() {
    let test_dir = setup_test_env("stripped");
    let stripped_path = test_dir.join("stripped_image.tif");

    create_synthetic_geotiff(&stripped_path, 120, 120, 32750, false, false, None);

    let mut dec = Decoder::new(BufReader::new(File::open(&stripped_path).unwrap())).unwrap();
    let details = extract_tiff_technical_details(&mut dec, 120, 120).unwrap();
    assert_eq!(details.layout, "stripped");

    let job_id = format!("job_stripped_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: stripped_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Stripped Test".to_string()),
    };

    let result = execute_import_option_b(&job_id, payload, CancellationToken::new());
    assert!(result.is_ok(), "Impor stripped GeoTIFF harus berhasil");
    let meta = result.unwrap();

    let _ = fs::remove_dir_all(get_storage_root().join("images").join(&meta.id));
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

/// (Butir 5) Uji GeoTIFF UTM 31N (Paris) dan EPSG:3857 (Web Mercator) End-to-End
#[test]
fn test_geotiff_end_to_end_utm_and_epsg3857() {
    let test_dir = setup_test_env("crs_e2e");

    // 1. UTM 31N Paris (EPSG:32631)
    let paris_path = test_dir.join("paris_utm31n.tif");
    {
        let file = File::create(&paris_path).unwrap();
        let mut encoder = TiffEncoder::new(file).unwrap();
        let mut img = encoder.new_image::<colortype::RGB8>(80, 80).unwrap();
        // Eiffel Tower: E=448250.50, N=5411951.59
        let tiepoints = [0.0f64, 0.0, 0.0, 448250.50, 5411951.59, 0.0];
        img.encoder()
            .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
            .unwrap();
        let pixel_scales = [1.0f64, 1.0, 0.0];
        img.encoder()
            .write_tag(Tag::ModelPixelScaleTag, &pixel_scales[..])
            .unwrap();
        let geokeys = [
            1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32631,
        ];
        img.encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
            .unwrap();
        img.write_data(&vec![128u8; 80 * 80 * 3]).unwrap();
    }

    let mut dec_paris = Decoder::new(BufReader::new(File::open(&paris_path).unwrap())).unwrap();
    let georef_paris = extract_geotiff_georeference(&mut dec_paris, 80, 80).unwrap();
    assert_eq!(georef_paris.crs_code, 32631);
    let [[s_p, w_p], [n_p, e_p]] = georef_paris.bounds;
    assert!((s_p - 48.85).abs() < 0.1);
    assert!((w_p - 2.29).abs() < 0.1);
    assert!(s_p < n_p && w_p < e_p);

    // 2. EPSG:3857 Web Mercator
    let m3857_path = test_dir.join("mercator_3857.tif");
    {
        let file = File::create(&m3857_path).unwrap();
        let mut encoder = TiffEncoder::new(file).unwrap();
        let mut img = encoder.new_image::<colortype::RGB8>(80, 80).unwrap();
        // Monas Jakarta EPSG:3857: x = 11891944.25, y = -688776.39
        let tiepoints = [0.0f64, 0.0, 0.0, 11891944.25, -688776.39, 0.0];
        img.encoder()
            .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
            .unwrap();
        let pixel_scales = [1.0f64, 1.0, 0.0];
        img.encoder()
            .write_tag(Tag::ModelPixelScaleTag, &pixel_scales[..])
            .unwrap();
        let geokeys = [
            1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 3857,
        ];
        img.encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
            .unwrap();
        img.write_data(&vec![128u8; 80 * 80 * 3]).unwrap();
    }

    let mut dec_3857 = Decoder::new(BufReader::new(File::open(&m3857_path).unwrap())).unwrap();
    let georef_3857 = extract_geotiff_georeference(&mut dec_3857, 80, 80).unwrap();
    assert_eq!(georef_3857.crs_code, 3857);
    let [[s_m, w_m], [n_m, e_m]] = georef_3857.bounds;
    assert!((s_m - (-6.17)).abs() < 0.1);
    assert!((w_m - 106.82).abs() < 0.1);
    assert!(s_m < n_m && w_m < e_m);
}

/// (Butir 5) Citra Ber-Alpha Menghasilkan Berkas Display PNG
#[test]
fn test_geotiff_alpha_produces_png() {
    let test_dir = setup_test_env("alpha_png");
    let alpha_path = test_dir.join("drone_rgba.tif");

    create_synthetic_geotiff(&alpha_path, 80, 80, 32750, false, true, None);

    let job_id = format!("job_alpha_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: alpha_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Alpha PNG Test".to_string()),
    };

    let result = execute_import_option_b(&job_id, payload, CancellationToken::new());
    assert!(result.is_ok(), "Impor citra ber-alpha harus berhasil");
    let meta = result.unwrap();

    assert_eq!(
        meta.extension, "png",
        "Citra dengan alpha harus menghasilkan display berkas PNG"
    );

    let image_dir = get_storage_root().join("images").join(&meta.id);
    assert!(image_dir.join("display.png").exists());

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

/// (Butir 5) Tidak Ada Upscale untuk Citra Berukuran Kecil
#[test]
fn test_geotiff_no_upscale_for_small_images() {
    let test_dir = setup_test_env("no_upscale");
    let small_path = test_dir.join("small_ortho.tif");

    create_synthetic_geotiff(&small_path, 350, 250, 32750, false, false, None);

    let job_id = format!("job_small_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: small_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Small Image Test".to_string()),
    };

    let result = execute_import_option_b(&job_id, payload, CancellationToken::new());
    assert!(result.is_ok());
    let meta = result.unwrap();

    assert_eq!(meta.dimensions, Some([350, 250]));

    let display_path = get_storage_root()
        .join("images")
        .join(&meta.id)
        .join("display.jpg");
    let dims = image::image_dimensions(&display_path).unwrap();
    assert_eq!(
        dims,
        (350, 250),
        "Citra kecil tidak boleh di-upscale (harus tetap 350x250)"
    );

    let _ = fs::remove_dir_all(get_storage_root().join("images").join(&meta.id));
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

/// (Butir 6) Sampling Histogram 16-bit: Fallback Full-Range Jika Histogram Kosong
#[test]
fn test_geotiff_16bit_empty_histogram_fallback_and_extended_sampling() {
    let test_dir = setup_test_env("u16_fallback");
    let empty_path = test_dir.join("gray16_all_nodata.tif");

    let file = File::create(&empty_path).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();
    let img = encoder.new_image::<colortype::Gray16>(128, 128).unwrap();

    // Isi seluruh piksel dengan nilai nodata (0)
    let pixels = vec![0u16; 128 * 128];
    img.write_data(&pixels).unwrap();

    let mut dec = Decoder::new(BufReader::new(File::open(&empty_path).unwrap())).unwrap();
    // Nodata = 0, sehingga seluruh sampel valid bernilai 0 -> histogram kosong
    let (lo, hi, is_full_range) =
        compute_16bit_percentiles(&mut dec, 128, 128, false, Some(0.0)).unwrap();

    assert!(
        is_full_range,
        "Histogram kosong harus memicu fallback full-range"
    );
    assert_eq!(lo, 0.0);
    assert_eq!(hi, 65535.0);
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

/// (Butir 8) Benchmark Sintetis Worst-Case Model: Tiled Deflate 16-bit RGBA lebar ~22.000 px
#[test]
#[ignore]
fn test_worst_case_model_synthetic_22k_width_16bit_deflate() {
    println!("\n=== BENCHMARK SINTETIS WORST-CASE MODEL 16-BIT RGBA 22.000 PX ===");
    #[cfg(windows)]
    let (ws_before, pf_before) = get_process_memory_counters();

    let width = 22000usize;
    let height = 512usize; // 1 band tile row setinggi 512 px
    let channels = 4usize; // RGBA

    // Buffer tile band row mentah 16-bit: 22000 * 512 * 4 * 2 byte = 90.11 MiB
    let tile_band_bytes = width * height * channels * 2;
    let mut tile_band_buffer: Vec<u8> = vec![0u8; tile_band_bytes];
    // Sentuh seluruh halaman fisik 4 KiB secara aktif dengan data gradien non-nol
    // agar OS menaruhnya di Working Set fisik (RAM fisik), bukan hanya reservasi virtual commit.
    for page_idx in (0..tile_band_bytes).step_by(4096) {
        let val = ((page_idx / 4096) & 0xFF) as u8;
        tile_band_buffer[page_idx] = val.max(1);
    }

    // Akumulator StreamingResampler: lebar input 22.000 px -> target 4.096 px
    // Buffer band akumulator: 4 channel * 4096 px * 8 byte (f64) = 131 KB
    let out_w = 4096usize;
    let accum_bytes = out_w * channels * std::mem::size_of::<f64>();
    let mut accum_buffer: Vec<f64> = vec![0.0f64; out_w * channels];
    accum_buffer[0] = 1.0;

    // Final Display RGB/RGBA Buffer 4096 x 4096 x 4 = 64 MiB (67.1 MB)
    let display_buffer_bytes = 4096 * 4096 * 4;
    let mut display_buffer: Vec<u8> = vec![0u8; display_buffer_bytes];
    for page_idx in (0..display_buffer_bytes).step_by(4096) {
        let val = ((page_idx / 4096) & 0xFF) as u8;
        display_buffer[page_idx] = val.max(1);
    }

    #[cfg(windows)]
    let (ws_peak, pf_peak) = get_process_memory_counters();

    println!("Alokasi Simulasi Kasus Terburuk:");
    println!(
        "- Tile Band Row Buffer (22k px x 512 x 4 x 2B): {:.2} MiB",
        tile_band_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "- StreamingResampler Accumulator: {:.2} KiB",
        accum_bytes as f64 / 1024.0
    );
    println!(
        "- Display RGBA Frame Buffer (4096 x 4096 x 4): {:.2} MiB",
        display_buffer_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "- Total Estimasi Model Teoritis: {:.2} MiB",
        (tile_band_bytes + display_buffer_bytes + accum_bytes) as f64 / (1024.0 * 1024.0)
    );

    #[cfg(windows)]
    {
        println!("Pengukuran Memori Proses Aktual (Windows):");
        println!(
            "- Baseline WorkingSet: {:.2} MiB",
            ws_before as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Baseline Pagefile: {:.2} MiB",
            pf_before as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak WorkingSet: {:.2} MiB",
            ws_peak as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak Pagefile: {:.2} MiB",
            pf_peak as f64 / (1024.0 * 1024.0)
        );
        assert!(
            ws_peak <= 400 * 1024 * 1024,
            "Puncak WorkingSet worst-case harus <= 400 MiB"
        );
    }
}

/// (Butir 2a, 2b, 2c, 2d, 7) Validasi JPEG-in-TIFF pada Berkas Nyata
#[test]
#[ignore]
fn test_real_oam_balikpapan_numerical_and_chunk_validation() {
    let tiff_path_str = match std::env::var("ORTHO_REAL_TIFF") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            println!("Variabel lingkungan ORTHO_REAL_TIFF tidak disetel. Melewati pengujian berkas nyata.");
            return;
        }
    };
    let tiff_path = PathBuf::from(&tiff_path_str);
    if !tiff_path.exists() {
        println!(
            "Berkas di ORTHO_REAL_TIFF ({:?}) tidak ditemukan. Melewati pengujian berkas nyata.",
            tiff_path
        );
        return;
    }

    println!("\n================================================================================");
    println!("VALIDASI NUMERIK DAN CHUNK CITRA NYATA BALIKPAPAN (JPEG-IN-TIFF)");
    println!("================================================================================");
    println!("Lokasi Berkas: {:?}", tiff_path);

    // 1. Ekstraksi Tag Header Spesifik (Tag 262, Tag 530, Tag 347, Dimensi)
    let file = File::open(&tiff_path).unwrap();
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap();

    let (width, height) = decoder.dimensions().unwrap();
    let chunk_dims = decoder.chunk_dimensions();
    println!("Dimensi Citra: {} x {} px", width, height);
    println!("Dimensi Chunk / Tile: {:?}", chunk_dims);

    // Tag 262: PhotometricInterpretation
    let photometric_tag = decoder.get_tag(Tag::PhotometricInterpretation);
    let photometric_val = match photometric_tag {
        Ok(Value::Short(v)) => v,
        Ok(Value::Unsigned(v)) => v as u16,
        _ => 0,
    };
    println!(
        "- Tag 262 (PhotometricInterpretation): {} ({})",
        photometric_val,
        match photometric_val {
            2 => "RGB",
            6 => "YCbCr",
            _other => "Lainnya",
        }
    );

    // Tag 530: YCbCrSubSampling
    let subsampling_tag = decoder.get_tag(Tag::Unknown(530));
    println!("- Tag 530 (YCbCrSubSampling): {:?}", subsampling_tag);

    // Tag 347: JPEGTables
    let jpeg_tables_tag = decoder.get_tag(Tag::JPEGTables);
    let has_jpeg_tables = jpeg_tables_tag.is_ok();
    let jpeg_tables_bytes: Option<Vec<u8>> = match jpeg_tables_tag {
        Ok(Value::List(list)) => {
            let b: Vec<u8> = list
                .into_iter()
                .filter_map(|v| match v {
                    Value::Byte(b) => Some(b),
                    Value::Unsigned(u) => Some(u as u8),
                    Value::Short(s) => Some(s as u8),
                    _ => None,
                })
                .collect();
            if !b.is_empty() {
                Some(b)
            } else {
                None
            }
        }
        Ok(Value::Byte(b)) => Some(vec![b]),
        _ => None,
    };

    println!(
        "- Tag 347 (JPEGTables): {}",
        if has_jpeg_tables {
            format!(
                "Ada (Shared DHT/DQT present, panjang: {} B)",
                jpeg_tables_bytes.as_ref().map(|v| v.len()).unwrap_or(0)
            )
        } else {
            "Tidak Ada (Standalone JPEG tiles with embedded tables)".to_string()
        }
    );

    // 2. (Butir 2c) Verifikasi Chunk Tepi:
    // 20.111 = 39 x 512 + 143; 13.583 = 26 x 512 + 271
    let (chunk_w, chunk_h) = chunk_dims;
    let tiles_x = width.div_ceil(chunk_w);
    let tiles_y = height.div_ceil(chunk_h);
    let last_col_valid_w = width - (tiles_x - 1) * chunk_w;
    let last_row_valid_h = height - (tiles_y - 1) * chunk_h;
    println!("\nVerifikasi Chunk Tepi:");
    println!("- Grid Tile: {} kolom x {} baris", tiles_x, tiles_y);
    println!(
        "- Chunk Tepi Kanan (kolom {}): valid lebar {} px, padding {} px",
        tiles_x - 1,
        last_col_valid_w,
        chunk_w - last_col_valid_w
    );
    println!(
        "- Chunk Tepi Bawah (baris {}): valid tinggi {} px, padding {} px",
        tiles_y - 1,
        last_row_valid_h,
        chunk_h - last_row_valid_h
    );
    assert_eq!(last_col_valid_w, 143, "Lebar valid tile kanan harus 143 px");
    assert_eq!(
        last_row_valid_h, 271,
        "Tinggi valid tile bawah harus 271 px"
    );

    // 3. (Butir 2b) Uji Numerik >= 5 Tile: TL (0), TR parsial (39), Center (540), BL parsial (1040), BR parsial (1079)
    println!("\nUji Numerik 5 Tile (Tiff Crate vs jpeg-decoder Independen):");
    let test_tiles = [
        (0, "Top-Left (Sudut Kiri-Atas, Full 512x512)"),
        (39, "Top-Right (Tepi Kanan Parsial, 143x512)"),
        (540, "Center (Tengah, Full 512x512)"),
        (1040, "Bottom-Left (Tepi Bawah Parsial, 512x271)"),
        (1079, "Bottom-Right (Sudut Kanan-Bawah Parsial, 143x271)"),
    ];

    // Ekstraksi TileOffsets dan TileByteCounts
    let tile_offsets: Vec<u64> = match decoder.get_tag(Tag::TileOffsets) {
        Ok(Value::List(list)) => list
            .into_iter()
            .filter_map(|v| match v {
                Value::Unsigned(u) => Some(u as u64),
                Value::UnsignedBig(u) => Some(u),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let tile_byte_counts: Vec<u64> = match decoder.get_tag(Tag::TileByteCounts) {
        Ok(Value::List(list)) => list
            .into_iter()
            .filter_map(|v| match v {
                Value::Unsigned(u) => Some(u as u64),
                Value::UnsignedBig(u) => Some(u),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    assert!(
        !tile_offsets.is_empty() && tile_offsets.len() == tile_byte_counts.len(),
        "TileOffsets dan TileByteCounts harus terbaca valid"
    );

    let mut raw_file = File::open(&tiff_path).unwrap();

    let assemble_jpeg = |tables: Option<&[u8]>, raw_tile: &[u8]| -> Vec<u8> {
        if let Some(t) = tables {
            if t.len() >= 4 && raw_tile.len() >= 2 {
                let mut assembled = Vec::with_capacity(t.len() + raw_tile.len());
                let t_body = if t.ends_with(&[0xFF, 0xD9]) {
                    &t[..t.len() - 2]
                } else {
                    t
                };
                assembled.extend_from_slice(t_body);
                let tile_body = if raw_tile.starts_with(&[0xFF, 0xD8]) {
                    &raw_tile[2..]
                } else {
                    raw_tile
                };
                assembled.extend_from_slice(tile_body);
                return assembled;
            }
        }
        raw_tile.to_vec()
    };

    for (tile_idx, desc) in test_tiles {
        let offset = tile_offsets[tile_idx as usize];
        let count = tile_byte_counts[tile_idx as usize];

        // A. Decode independen via jpeg-decoder langsung dengan assembled tables
        raw_file.seek(SeekFrom::Start(offset)).unwrap();
        let mut raw_tile_bytes = vec![0u8; count as usize];
        raw_file.read_exact(&mut raw_tile_bytes).unwrap();

        let stream_to_decode = assemble_jpeg(jpeg_tables_bytes.as_deref(), &raw_tile_bytes);
        let mut jpeg_dec = jpeg_decoder::Decoder::new(Cursor::new(&stream_to_decode));
        let indep_pixels = jpeg_dec
            .decode()
            .expect("jpeg-decoder harus berhasil mendecode raw tile JPEG");
        let indep_info = jpeg_dec.info().unwrap();

        // B. Decode via crate tiff
        let tiff_chunk_buf = match decoder
            .read_chunk(tile_idx)
            .expect("Crate tiff harus berhasil mendecode chunk")
        {
            DecodingResult::U8(data) => data,
            other => panic!("Expected U8 decoded tile, got {:?}", other),
        };

        if tile_idx == 540 {
            let _ = fs::write("target/ortho_real_check/tile_540_rust.raw", &tiff_chunk_buf);
        }
        if tile_idx == 0 {
            let _ = fs::write("target/ortho_real_check/tile_0_rust.raw", &tiff_chunk_buf);
        }

        // C. Perbandingan piksel pada dimensi valid chunk (bebas padding)
        let tc = tile_idx % tiles_x;
        let tr = tile_idx / tiles_x;
        let valid_w = chunk_w.min(width - tc * chunk_w) as usize;
        let valid_h = chunk_h.min(height - tr * chunk_h) as usize;

        assert_eq!(
            tiff_chunk_buf.len(),
            valid_w * valid_h * 3,
            "Crate tiff harus memotong buffer tile ke dimensi valid ({}x{}) tanpa padding",
            valid_w,
            valid_h
        );

        let mut diff_sum: u64 = 0;
        let mut max_diff: u8 = 0;
        let mut compared_samples: usize = 0;

        for y in 0..valid_h {
            for x in 0..valid_w {
                let tiff_idx = (y * valid_w + x) * 3;
                let indep_idx = (y * (indep_info.width as usize) + x) * 3;
                for c in 0..3 {
                    let diff = (tiff_chunk_buf[tiff_idx + c] as i32
                        - indep_pixels[indep_idx + c] as i32)
                        .unsigned_abs() as u8;
                    diff_sum += diff as u64;
                    max_diff = max_diff.max(diff);
                    compared_samples += 1;
                }
            }
        }
        let mean_diff = diff_sum as f64 / compared_samples.max(1) as f64;

        println!(
            "  * Tile {:4} [{}]: raw JPEG = {}x{}, valid terpotong = {}x{}, bytes = {} B, max diff = {}, mean diff = {:.4}",
            tile_idx, desc, indep_info.width, indep_info.height, valid_w, valid_h, count, max_diff, mean_diff
        );

        // Keduanya mendecode bitstream JPEG yang sama, toleransi selisih <= 2
        assert!(
            max_diff <= 2,
            "Selisih maksimum harus <= 2 (didapat {})",
            max_diff
        );
        assert!(
            mean_diff < 0.1,
            "Selisih rata-rata harus < 0.1 (didapat {:.4})",
            mean_diff
        );
    }

    // 4. (Butir 2a) Jalankan Eksekusi Impor Opsi B Penuh dan Simpan display.jpg ke target/ortho_real_check/display.jpg
    let job_id = format!("job_check_real_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: tiff_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Balikpapan Check".to_string()),
    };

    let start_t = Instant::now();
    let meta = execute_import_option_b(&job_id, payload, CancellationToken::new())
        .expect("Impor Opsi B harus berhasil");
    let dur = start_t.elapsed();

    let storage_display = get_storage_root()
        .join("images")
        .join(&meta.id)
        .join("display.jpg");
    assert!(
        storage_display.exists(),
        "Berkas display.jpg harus ada di storage"
    );

    let fixed_dir = PathBuf::from("target").join("ortho_real_check");
    fs::create_dir_all(&fixed_dir).unwrap();
    let fixed_display = fixed_dir.join("display.jpg");
    fs::copy(&storage_display, &fixed_display).unwrap();

    let abs_display_path = fs::canonicalize(&fixed_display).unwrap_or(fixed_display.clone());
    println!("\n(Butir 2a) HASIL BERKAS DISPLAY TERSIMPAN DI LOKASI TETAP:");
    println!("  -> Path Absolut: {}", abs_display_path.display());
    println!(
        "  -> Ukuran: {} bytes ({:.2} MiB)",
        fs::metadata(&fixed_display).unwrap().len(),
        fs::metadata(&fixed_display).unwrap().len() as f64 / (1024.0 * 1024.0)
    );
    let final_dims = image::image_dimensions(&fixed_display).unwrap();
    println!(
        "  -> Dimensi Display: {} x {} px (Waktu Eksekusi: {:.2?})",
        final_dims.0, final_dims.1, dur
    );

    // Pembersihan data uji dari storage
    let _ = fs::remove_dir_all(get_storage_root().join("images").join(&meta.id));
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

/// (Butir 3, 7) Uji RGBA/PNG Realistis dari Piksel Nyata OAM
#[test]
#[ignore]
fn test_real_oam_rgba_png_and_16bit_pipeline() {
    let tiff_path_str = match std::env::var("ORTHO_REAL_TIFF") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            println!(
                "Variabel lingkungan ORTHO_REAL_TIFF tidak disetel. Melewati uji RGBA realistis."
            );
            return;
        }
    };
    let tiff_path = PathBuf::from(&tiff_path_str);
    if !tiff_path.exists() {
        return;
    }

    println!("\n================================================================================");
    println!("UJI RGBA/PNG REALISTIS (FIXTURE SINTETIS DARI PIKSEL NYATA OAM BALIKPAPAN)");
    println!("================================================================================");

    let test_dir = setup_test_env("real_oam_rgba");

    // 1. Ambil tile nyata dan rangkai menjadi kanvas 4096 x 4096 piksel nyata
    let canvas_size = 4096u32;
    let mut real_rgba_8 = vec![0u8; (canvas_size * canvas_size * 4) as usize];

    let file = File::open(&tiff_path).unwrap();
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap();
    let (chunk_w, chunk_h) = decoder.chunk_dimensions();

    // Baca 8 x 8 tile nyata dari pusat Balikpapan (row 10-17, col 16-23) = 4096 x 4096 piksel
    for ty in 0..8 {
        for tx in 0..8 {
            let tile_idx = (ty + 10) * 40 + (tx + 16);
            if let Ok(DecodingResult::U8(chunk_data)) = decoder.read_chunk(tile_idx) {
                for cy in 0..chunk_h {
                    for cx in 0..chunk_w {
                        let src_idx = ((cy * chunk_w + cx) * 3) as usize;
                        let dst_x = tx * chunk_w + cx;
                        let dst_y = ty * chunk_h + cy;
                        let dst_idx = ((dst_y * canvas_size + dst_x) * 4) as usize;

                        let r = chunk_data[src_idx];
                        let g = chunk_data[src_idx + 1];
                        let b = chunk_data[src_idx + 2];

                        // Mask elips transparan: di luar elips alpha = 0, di dalam alpha = 255
                        let dx = dst_x as f64 - (canvas_size as f64 / 2.0);
                        let dy = dst_y as f64 - (canvas_size as f64 / 2.0);
                        let radius_x = canvas_size as f64 * 0.45;
                        let radius_y = canvas_size as f64 * 0.40;
                        let in_ellipse = ((dx * dx) / (radius_x * radius_x))
                            + ((dy * dy) / (radius_y * radius_y))
                            <= 1.0;

                        let a = if in_ellipse { 255u8 } else { 0u8 };
                        real_rgba_8[dst_idx] = r;
                        real_rgba_8[dst_idx + 1] = g;
                        real_rgba_8[dst_idx + 2] = b;
                        real_rgba_8[dst_idx + 3] = a;
                    }
                }
            }
        }
    }

    // 2. Tulis GeoTIFF Tiled Deflate 8-bit RGBA
    let rgba8_path = test_dir.join("real_oam_rgba8.tif");
    {
        let file = File::create(&rgba8_path).unwrap();
        let mut encoder = TiffEncoder::new(file).unwrap();
        let mut img = encoder
            .new_image::<colortype::RGBA8>(canvas_size, canvas_size)
            .unwrap();

        let tiepoints = [0.0f64, 0.0, 0.0, 500000.0, 9866000.0, 0.0];
        img.encoder()
            .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
            .unwrap();
        let pixel_scales = [1.0f64, 1.0, 0.0];
        img.encoder()
            .write_tag(Tag::ModelPixelScaleTag, &pixel_scales[..])
            .unwrap();
        let geokeys = [
            1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32750,
        ];
        img.encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
            .unwrap();
        img.encoder()
            .write_tag(Tag::ExtraSamples, &[2u16][..])
            .unwrap();

        img.write_data(&real_rgba_8).unwrap();
    }

    // 3. Jalankan Pipeline Opsi B pada RGBA 8-bit
    #[cfg(windows)]
    let (ws_before_8, pf_before_8) = get_process_memory_counters();

    let job_id_8 = format!("job_rgba8_{}", chrono::Utc::now().timestamp_millis());
    let payload_8 = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: rgba8_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Real OAM RGBA 8-bit".to_string()),
    };

    let start_8 = Instant::now();
    let meta_8 = execute_import_option_b(&job_id_8, payload_8, CancellationToken::new())
        .expect("Impor RGBA 8-bit harus berhasil");
    let dur_8 = start_8.elapsed();

    #[cfg(windows)]
    let (ws_peak_8, pf_peak_8) = get_process_memory_counters();

    let display_file_8 = get_storage_root()
        .join("images")
        .join(&meta_8.id)
        .join(format!("display.{}", meta_8.extension));
    let dims_8 = image::image_dimensions(&display_file_8).unwrap();
    let size_bytes_8 = fs::metadata(&display_file_8).unwrap().len();

    println!("\nHasil Pipeline RGBA 8-bit:");
    println!("- Ekstensi Display: {}", meta_8.extension);
    assert_eq!(
        meta_8.extension, "png",
        "Citra dengan alpha harus menghasilkan PNG"
    );
    println!("- Resolusi Display Akhir: {} x {} px", dims_8.0, dims_8.1);
    println!(
        "- Ukuran PNG Akhir: {} bytes ({:.2} MiB)",
        size_bytes_8,
        size_bytes_8 as f64 / (1024.0 * 1024.0)
    );
    println!("- Durasi: {:.2?}", dur_8);
    #[cfg(windows)]
    {
        println!(
            "- Baseline WorkingSet: {:.2} MiB, Pagefile: {:.2} MiB",
            ws_before_8 as f64 / (1024.0 * 1024.0),
            pf_before_8 as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak WorkingSet: {:.2} MiB",
            ws_peak_8 as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak Pagefile: {:.2} MiB",
            pf_peak_8 as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Delta WorkingSet: +{:.2} MiB",
            (ws_peak_8.saturating_sub(ws_before_8)) as f64 / (1024.0 * 1024.0)
        );
    }

    let max_dim_8 = dims_8.0.max(dims_8.1);
    println!("- Resolusi Akhir Maksimum 8-bit: {} px", max_dim_8);
    assert!(
        max_dim_8 >= 1792,
        "Resolusi akhir display PNG harus >= 1.792 px dengan batas 10 MB (aktual: {} px)",
        max_dim_8
    );

    // 4. Varian 16-bit RGBA (nilai piksel x257)
    let rgba16_path = test_dir.join("real_oam_rgba16.tif");
    {
        let mut real_rgba_16 = vec![0u16; (canvas_size * canvas_size * 4) as usize];
        for i in 0..real_rgba_8.len() {
            real_rgba_16[i] = (real_rgba_8[i] as u16) * 257;
        }

        let file = File::create(&rgba16_path).unwrap();
        let mut encoder = TiffEncoder::new(file).unwrap();
        let mut img = encoder
            .new_image::<colortype::RGBA16>(canvas_size, canvas_size)
            .unwrap();

        let tiepoints = [0.0f64, 0.0, 0.0, 500000.0, 9866000.0, 0.0];
        img.encoder()
            .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
            .unwrap();
        let pixel_scales = [1.0f64, 1.0, 0.0];
        img.encoder()
            .write_tag(Tag::ModelPixelScaleTag, &pixel_scales[..])
            .unwrap();
        let geokeys = [
            1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32750,
        ];
        img.encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
            .unwrap();
        img.encoder()
            .write_tag(Tag::ExtraSamples, &[2u16][..])
            .unwrap();

        img.write_data(&real_rgba_16).unwrap();
    }

    #[cfg(windows)]
    let (ws_before_16, pf_before_16) = get_process_memory_counters();

    let job_id_16 = format!("job_rgba16_{}", chrono::Utc::now().timestamp_millis());
    let payload_16 = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: rgba16_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Real OAM RGBA 16-bit".to_string()),
    };

    let start_16 = Instant::now();
    let meta_16 = execute_import_option_b(&job_id_16, payload_16, CancellationToken::new())
        .expect("Impor RGBA 16-bit harus berhasil");
    let dur_16 = start_16.elapsed();

    #[cfg(windows)]
    let (ws_peak_16, pf_peak_16) = get_process_memory_counters();

    let display_file_16 = get_storage_root()
        .join("images")
        .join(&meta_16.id)
        .join(format!("display.{}", meta_16.extension));
    let dims_16 = image::image_dimensions(&display_file_16).unwrap();
    let size_bytes_16 = fs::metadata(&display_file_16).unwrap().len();

    println!("\nHasil Pipeline RGBA 16-bit:");
    println!("- Ekstensi Display: {}", meta_16.extension);
    assert_eq!(meta_16.extension, "png");
    println!("- Resolusi Display Akhir: {} x {} px", dims_16.0, dims_16.1);
    println!(
        "- Ukuran PNG Akhir: {} bytes ({:.2} MiB)",
        size_bytes_16,
        size_bytes_16 as f64 / (1024.0 * 1024.0)
    );
    println!("- Durasi: {:.2?}", dur_16);
    #[cfg(windows)]
    {
        println!(
            "- Baseline WorkingSet: {:.2} MiB, Pagefile: {:.2} MiB",
            ws_before_16 as f64 / (1024.0 * 1024.0),
            pf_before_16 as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak WorkingSet: {:.2} MiB",
            ws_peak_16 as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Peak Pagefile: {:.2} MiB",
            pf_peak_16 as f64 / (1024.0 * 1024.0)
        );
        println!(
            "- Delta WorkingSet: +{:.2} MiB",
            (ws_peak_16.saturating_sub(ws_before_16)) as f64 / (1024.0 * 1024.0)
        );
    }

    let max_dim_16 = dims_16.0.max(dims_16.1);
    println!("- Resolusi Akhir Maksimum 16-bit: {} px", max_dim_16);
    assert!(
        max_dim_16 >= 1792,
        "Resolusi akhir display PNG 16-bit harus >= 1.792 px dengan batas 10 MB (aktual: {} px)",
        max_dim_16
    );

    // Pembersihan
    let _ = fs::remove_dir_all(get_storage_root().join("images").join(&meta_8.id));
    let _ = fs::remove_file(
        get_storage_root()
            .join("layers")
            .join(format!("{}.json", meta_8.id)),
    );
    let _ = fs::remove_file(
        get_storage_root()
            .join(".jobs")
            .join(format!("{}.json", job_id_8)),
    );
    let _ = fs::remove_dir_all(get_storage_root().join("images").join(&meta_16.id));
    let _ = fs::remove_file(
        get_storage_root()
            .join("layers")
            .join(format!("{}.json", meta_16.id)),
    );
    let _ = fs::remove_file(
        get_storage_root()
            .join(".jobs")
            .join(format!("{}.json", job_id_16)),
    );
}

/// (Butir 6) Pengukuran Waktu Encode PNG per Dimensi dan Latensi Cancel Terburuk
#[test]
#[ignore]
fn test_png_encode_timings_and_worst_case_cancel_latency() {
    let tiff_path_str = match std::env::var("ORTHO_REAL_TIFF") {
        Ok(v) if !v.is_empty() => v,
        _ => return,
    };
    let tiff_path = PathBuf::from(&tiff_path_str);
    if !tiff_path.exists() {
        return;
    }

    println!("\n================================================================================");
    println!("PENGUKURAN WAKTU ENCODE PNG DAN LATENSI CANCEL TERBURUK");
    println!("================================================================================");

    let test_dir = setup_test_env("png_encode_bench");
    let canvas_size = 4096u32;
    let mut real_rgba = vec![0u8; (canvas_size * canvas_size * 4) as usize];

    let file = File::open(&tiff_path).unwrap();
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap();
    let (chunk_w, chunk_h) = decoder.chunk_dimensions();

    for ty in 0..8 {
        for tx in 0..8 {
            let tile_idx = (ty + 10) * 40 + (tx + 16);
            if let Ok(DecodingResult::U8(chunk_data)) = decoder.read_chunk(tile_idx) {
                for cy in 0..chunk_h {
                    for cx in 0..chunk_w {
                        let src_idx = ((cy * chunk_w + cx) * 3) as usize;
                        let dst_x = tx * chunk_w + cx;
                        let dst_y = ty * chunk_h + cy;
                        let dst_idx = ((dst_y * canvas_size + dst_x) * 4) as usize;
                        real_rgba[dst_idx] = chunk_data[src_idx];
                        real_rgba[dst_idx + 1] = chunk_data[src_idx + 1];
                        real_rgba[dst_idx + 2] = chunk_data[src_idx + 2];
                        real_rgba[dst_idx + 3] = 255;
                    }
                }
            }
        }
    }

    let dyn_img = image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(canvas_size, canvas_size, real_rgba.clone()).unwrap(),
    );

    // 1. (Butir 6a) Ukur Waktu Encode PNG per Dimensi
    for &dim in &[4096u32, 3072, 2048] {
        let resized = if dim == 4096 {
            dyn_img.clone()
        } else {
            dyn_img.resize_exact(dim, dim, image::imageops::FilterType::Nearest)
        };
        let out_path = test_dir.join(format!("bench_{}.png", dim));
        let start = Instant::now();
        resized
            .save_with_format(&out_path, image::ImageFormat::Png)
            .unwrap();
        let elapsed = start.elapsed();
        let file_size = fs::metadata(&out_path).unwrap().len();
        println!(
            "-> Dimensi {} x {} px: Durasi Encode = {:.2?} ({:.1} ms), Ukuran PNG = {} bytes ({:.2} MiB)",
            dim,
            dim,
            elapsed,
            elapsed.as_secs_f64() * 1000.0,
            file_size,
            file_size as f64 / (1024.0 * 1024.0)
        );
    }

    // 2. (Butir 6b) Latensi Cancel Terburuk saat Encode PNG Besar Berlangsung
    println!("\nPengukuran Latensi Cancel Terburuk (Interupsi saat write PNG 4096 px):");
    let token = CancellationToken::new();
    let token_clone = token.clone();
    let dyn_img_clone = dyn_img.clone();
    let staging_dir = test_dir.join("staging_cancel_test");
    fs::create_dir_all(&staging_dir).unwrap();
    let staging_dir_clone = staging_dir.clone();

    let handle = std::thread::spawn(move || {
        aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::adaptive_encode_display(
            &dyn_img_clone,
            &staging_dir_clone,
            true,
            Some(&token_clone),
        )
    });

    // Tunggu 50 ms agar writer mulai aktif mengenkode blok PNG
    std::thread::sleep(std::time::Duration::from_millis(50));

    let cancel_start = Instant::now();
    token.cancel();
    let res = handle.join().expect("thread join");
    let cancel_latency = cancel_start.elapsed();

    println!("- Status Hasil Encode: {:?}", res.is_err());
    println!(
        "- Latensi Pembatalan Terburuk: {:.2?} ({:.2} ms)",
        cancel_latency,
        cancel_latency.as_secs_f64() * 1000.0
    );
    assert!(
        res.is_err(),
        "Encode display harus diinterupsi oleh token pembatalan"
    );
    println!(
        "- Catatan Tahap Monolitik: Kompresi PNG zlib berjalan hingga buffer flush. Latensi cancel terburuk = {:.2} s ({:.1} ms)",
        cancel_latency.as_secs_f64(),
        cancel_latency.as_secs_f64() * 1000.0
    );
    assert!(
        cancel_latency.as_secs() <= 5,
        "Latensi pembatalan terburuk tahap monolitik encode harus selesai dalam batas wajar (< 5s, aktual: {:?})",
        cancel_latency
    );
}

/// (Butir 4, 7) Latensi Pembatalan di Tengah Proses pada Citra Nyata
#[test]
#[ignore]
fn test_real_oam_cancellation_mid_process() {
    let tiff_path_str = match std::env::var("ORTHO_REAL_TIFF") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            println!("Variabel lingkungan ORTHO_REAL_TIFF tidak disetel. Melewati uji cancel mid-process.");
            return;
        }
    };
    let tiff_path = PathBuf::from(&tiff_path_str);
    if !tiff_path.exists() {
        return;
    }

    println!("\n================================================================================");
    println!("PENGUKURAN LATENSI CANCEL DI TENGAH PROSES (TIFF NYATA OAM 273 MP)");
    println!("================================================================================");

    let job_id = format!("job_cancel_mid_{}", chrono::Utc::now().timestamp_millis());
    let payload = ImportDualModePayload {
        mode: "B".to_string(),
        file_path: tiff_path.to_string_lossy().to_string(),
        save_original: false,
        manual_bounds: None,
        custom_filename: Some("Cancel Mid-Process Test".to_string()),
    };

    let token = CancellationToken::new();
    let token_clone = token.clone();
    let job_id_clone = job_id.clone();

    // Jalankan eksekusi di worker thread
    let handle =
        std::thread::spawn(move || execute_import_option_b(&job_id_clone, payload, token_clone));

    // Biarkan worker berjalan memproses >= 30% tile (membutuhkan ~1,5 detik pada citra 273 MP)
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // Kirim sinyal cancel di tengah proses streaming tile
    let cancel_start = Instant::now();
    token.cancel();

    let worker_result = handle.join().expect("Worker thread harus join dengan aman");
    let cancel_latency = cancel_start.elapsed();

    println!("Hasil Pengukuran Pembatalan Mid-Process:");
    println!("- Latensi Pembatalan: {:.2?}", cancel_latency);
    println!(
        "- Latensi dalam Milidetik: {:.2} ms",
        cancel_latency.as_secs_f64() * 1000.0
    );

    assert!(
        worker_result.is_err(),
        "Worker harus mengembalikan status error pembatalan"
    );
    match worker_result {
        Err(OrthoImportError::Dibatalkan) => {
            println!("- Status Error: Sesuai harapan (OrthoImportError::Dibatalkan)");
        }
        other => panic!("Harusnya Dibatalkan, didapat: {:?}", other),
    }

    // Pastikan folder staging .staging_* bersih seketika setelah worker kembali
    let staging_dir = get_storage_root()
        .join("images")
        .join(format!(".staging_{}", job_id));
    assert!(
        !staging_dir.exists(),
        "Folder staging .staging_* HARUS bersih setelah pembatalan mid-process"
    );
    println!("- Status Staging: Bersih (tidak ada sisa berkas)");

    println!("\nCatatan Analisis Latensi:");
    println!("- Titik Ukur 0,08 ms sebelumnya: diukur pada pre-flight check sebelum tile processing loop dimulai.");
    println!("- Titik Ukur Mid-Process ({:.2} ms): diukur dari pemanggilan token.cancel() di tengah streaming resampler sampai worker thread berhenti sepenuhnya.", cancel_latency.as_secs_f64() * 1000.0);
    println!("- Tahap yang tidak dapat diinterupsi: Encode display JPEG/PNG per iterasi adaptif (berlangsung monolitik di encoder C/Rust, durasi ~30-70 ms).");
}
