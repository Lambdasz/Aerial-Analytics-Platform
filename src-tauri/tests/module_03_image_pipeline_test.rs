//! Pengujian unit untuk pipeline impor citra JPG/PNG Module 3 (Fast-fail, Magic bytes, Batas Ukuran, Dimensi, Bounding Box WGS84).

use std::fs;
use tempfile::tempdir;

use aerial_analytics_platform_lib::map_controller::ortho_source::constants::{
    MAX_IMAGE_DIMENSION_PX, MAX_IMAGE_FILE_SIZE_BYTES,
};
use aerial_analytics_platform_lib::map_controller::ortho_source::image_pipeline::{
    process_image_import, validate_image_dimensions, validate_image_file_size,
    validate_magic_bytes, validate_wgs84_bounds,
};
use aerial_analytics_platform_lib::map_controller::ortho_source::types::ImportImagePayload;

#[test]
fn test_magic_bytes_detection() {
    let temp = tempdir().expect("Gagal membuat tempdir");

    // 1. Valid JPEG
    let jpg_path = temp.path().join("test.jpg");
    fs::write(&jpg_path, [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46]).unwrap();
    assert_eq!(validate_magic_bytes(&jpg_path).unwrap(), "jpg");

    // 2. Valid PNG
    let png_path = temp.path().join("test.png");
    fs::write(
        &png_path,
        [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00],
    )
    .unwrap();
    assert_eq!(validate_magic_bytes(&png_path).unwrap(), "png");

    // 3. Fake TIFF / Non-Image file
    let tiff_path = temp.path().join("test.tif");
    fs::write(&tiff_path, [0x49, 0x49, 0x2A, 0x00]).unwrap(); // 'II*' Little-endian TIFF
    assert!(validate_magic_bytes(&tiff_path).is_err());

    // 4. Random Text file
    let txt_path = temp.path().join("test.txt");
    fs::write(&txt_path, b"Hello World").unwrap();
    assert!(validate_magic_bytes(&txt_path).is_err());
}

#[test]
fn test_wgs84_bounds_validation() {
    // 1. Valid bounds (Balikpapan area)
    let valid = validate_wgs84_bounds(116.85, -1.25, 116.88, -1.22);
    assert!(valid.is_ok());
    let bounds = valid.unwrap();
    assert_eq!(bounds, [[-1.25, 116.85], [-1.22, 116.88]]);

    // 2. Invalid lat out of range (> 90)
    assert!(validate_wgs84_bounds(116.85, 95.0, 116.88, 96.0).is_err());

    // 3. Invalid lon out of range (> 180)
    assert!(validate_wgs84_bounds(185.0, -1.25, 190.0, -1.22).is_err());

    // 4. Inverted latitudes (south >= north)
    assert!(validate_wgs84_bounds(116.85, -1.20, 116.88, -1.25).is_err());

    // 5. Inverted longitudes (west >= east)
    assert!(validate_wgs84_bounds(116.90, -1.25, 116.85, -1.22).is_err());

    // 6. NaN values
    assert!(validate_wgs84_bounds(f64::NAN, -1.25, 116.88, -1.22).is_err());
}

#[test]
fn test_file_size_limit_rejection() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    let file_path = temp.path().join("small.jpg");
    fs::write(&file_path, [0u8; 1024]).unwrap();

    let size = validate_image_file_size(&file_path).unwrap();
    assert_eq!(size, 1024);

    // Assert konstanta ukuran
    assert_eq!(MAX_IMAGE_FILE_SIZE_BYTES, 52_428_800);
    assert_eq!(MAX_IMAGE_DIMENSION_PX, 16_384);
}

#[test]
fn test_full_image_import_lifecycle() {
    let temp = tempdir().expect("Gagal membuat tempdir");
    std::env::set_var("AAP_STORAGE_DIR", temp.path());

    // Buat gambar PNG 100x100 nyata menggunakan crate image
    let sample_png = temp.path().join("sample_ortho.png");
    let img = image::RgbImage::new(100, 100);
    img.save_with_format(&sample_png, image::ImageFormat::Png)
        .expect("Gagal menyimpan gambar uji");

    // Periksa dimensi via fast-fail
    let (w, h) = validate_image_dimensions(&sample_png).unwrap();
    assert_eq!(w, 100);
    assert_eq!(h, 100);

    // Eksekusi import
    let payload = ImportImagePayload {
        file_path: sample_png.to_string_lossy().to_string(),
        west: 116.856,
        south: -1.225,
        east: 116.865,
        north: -1.218,
    };

    let imported = process_image_import(payload).expect("Harus berhasil mengimpor");
    assert_eq!(imported.extension, "png");
    assert_eq!(imported.dimensions, Some([100, 100]));
    assert!(imported.bounds.is_some());

    // Verifikasi struktur atomik di disk:
    let image_dir = temp.path().join("images").join(&imported.id);
    assert!(image_dir.exists(), "Folder image harus dibuat");
    assert!(
        image_dir.join("original.png").exists(),
        "original.png harus ada"
    );
    assert!(
        image_dir.join("display.jpg").exists(),
        "display.jpg harus dibuat"
    );
    assert!(image_dir.join("meta.json").exists(), "meta.json harus ada");

    // Verifikasi layer terdaftar otomatis di layers/
    let layer_file = temp
        .path()
        .join("layers")
        .join(format!("{}.json", imported.id));
    assert!(layer_file.exists(), "Layer JSON harus terdaftar");
}
