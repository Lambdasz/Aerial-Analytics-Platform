//! Pipeline pemrosesan impor citra orthomosaic JPG/PNG dengan koordinat WGS84 murni Rust.
//! Menerapkan fast-fail validation: batas ukuran file (50 MB), magic bytes, dan dimensi header (16.384 px)
//! sebelum memuat piksel penuh ke memori.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::constants::{
    MAX_DISPLAY_FILE_SIZE_BYTES, MAX_IMAGE_DIMENSION_PX, MAX_IMAGE_FILE_SIZE_BYTES,
    PREVIEW_TARGET_DIMENSION_PX,
};
use super::types::{ImageCategory, ImportImagePayload, StoredImageMetadata};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::ortho_source::store::get_images_dir;
use crate::modules::module_03::storage_utils::write_atomic;

/// Memvalidasi batas ukuran file fisik (maksimal 50 MB) langsung dari filesystem metadata.
pub fn validate_image_file_size(path: &Path) -> Result<u64, MapControllerError> {
    if !path.exists() {
        return Err(MapControllerError::NotFound(format!(
            "Berkas citra tidak ditemukan: {}",
            path.display()
        )));
    }

    let meta = fs::metadata(path)?;
    let size = meta.len();

    if size > MAX_IMAGE_FILE_SIZE_BYTES {
        let size_mb = size as f64 / (1024.0 * 1024.0);
        return Err(MapControllerError::InvalidGeometry(format!(
            "Ukuran file {:.1} MB melebihi batas maksimal 50 MB",
            size_mb
        )));
    }

    Ok(size)
}

/// Memvalidasi format berkas dengan memeriksa signature/magic bytes (bukan hanya ekstensi).
/// Mengembalikan ekstensi normal ("jpg" atau "png").
pub fn validate_magic_bytes(path: &Path) -> Result<String, MapControllerError> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 8];
    let bytes_read = file.read(&mut header)?;

    if bytes_read >= 3 && header[0..3] == [0xFF, 0xD8, 0xFF] {
        return Ok("jpg".to_string());
    }

    if bytes_read >= 8 && header[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Ok("png".to_string());
    }

    Err(MapControllerError::InvalidGeometry(
        "Format berkas tidak valid. Hanya berkas JPG dan PNG asli yang diperbolehkan.".to_string(),
    ))
}

/// Mengekstrak dimensi gambar dari header berkas secara instan tanpa mendecode seluruh piksel ke RAM.
pub fn validate_image_dimensions(path: &Path) -> Result<(u32, u32), MapControllerError> {
    let (width, height) = image::image_dimensions(path).map_err(|e| {
        MapControllerError::InvalidGeometry(format!("Gagal membaca header dimensi citra: {}", e))
    })?;

    if width > MAX_IMAGE_DIMENSION_PX || height > MAX_IMAGE_DIMENSION_PX {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Dimensi citra {}x{} piksel melebihi batas maksimal 16.384 piksel per sisi.",
            width, height
        )));
    }

    if width == 0 || height == 0 {
        return Err(MapControllerError::InvalidGeometry(
            "Dimensi citra tidak valid (0 piksel).".to_string(),
        ));
    }

    Ok((width, height))
}

/// Memvalidasi bounding box WGS84: latitude [-90..90], longitude [-180..180],
/// serta memastikan west < east dan south < north.
pub fn validate_wgs84_bounds(
    west: f64,
    south: f64,
    east: f64,
    north: f64,
) -> Result<[[f64; 2]; 2], MapControllerError> {
    if west.is_nan() || south.is_nan() || east.is_nan() || north.is_nan() {
        return Err(MapControllerError::InvalidGeometry(
            "Nilai koordinat tidak boleh kosong atau NaN.".to_string(),
        ));
    }

    if !(-90.0..=90.0).contains(&south) || !(-90.0..=90.0).contains(&north) {
        return Err(MapControllerError::InvalidGeometry(
            "Nilai lintang (latitude) harus berada dalam rentang -90 hingga 90 derajat."
                .to_string(),
        ));
    }

    if !(-180.0..=180.0).contains(&west) || !(-180.0..=180.0).contains(&east) {
        return Err(MapControllerError::InvalidGeometry(
            "Nilai bujur (longitude) harus berada dalam rentang -180 hingga 180 derajat."
                .to_string(),
        ));
    }

    if south >= north {
        return Err(MapControllerError::InvalidGeometry(
            "Lintang selatan (South) harus lebih kecil dari lintang utara (North).".to_string(),
        ));
    }

    if west >= east {
        return Err(MapControllerError::InvalidGeometry(
            "Bujur barat (West) harus lebih kecil dari bujur timur (East).".to_string(),
        ));
    }

    // Leaflet bounds format: [[lat_min, lon_min], [lat_max, lon_max]]
    Ok([[south, west], [north, east]])
}

/// Menghasilkan pratinjau display.jpg yang di-downsample jika ukuran asli melebihi 2048 px
/// dan memastikan ukuran berkas hasil pratinjau tidak melebihi batas maksimal 5 MB.
pub fn generate_display_preview(
    source_path: &Path,
    target_dir: &Path,
    width: u32,
    height: u32,
) -> Result<(PathBuf, u64), MapControllerError> {
    let display_jpg = target_dir.join("display.jpg");

    // Gunakan ImageReader dengan no_limits() agar citra resolusi tinggi (sampai 16.384 px)
    // dapat di-decode ke memori untuk di-downscale ke display.jpg <= 5 MB
    let decoded_img = image::ImageReader::open(source_path)
        .map_err(|e| MapControllerError::InvalidGeometry(format!("Gagal membuka citra: {}", e)))
        .and_then(|mut reader| {
            reader.no_limits();
            reader.decode().map_err(|e| {
                MapControllerError::InvalidGeometry(format!("Gagal mendecode piksel citra: {}", e))
            })
        });

    if let Ok(img) = decoded_img {
        let mut max_dim = PREVIEW_TARGET_DIMENSION_PX.min(width.max(height));
        loop {
            let scale = max_dim as f32 / width.max(height) as f32;
            let target_w = ((width as f32 * scale).round() as u32).max(1);
            let target_h = ((height as f32 * scale).round() as u32).max(1);

            let resized = img.resize(target_w, target_h, image::imageops::FilterType::Nearest);
            if resized
                .save_with_format(&display_jpg, image::ImageFormat::Jpeg)
                .is_ok()
            {
                let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
                if size <= MAX_DISPLAY_FILE_SIZE_BYTES || max_dim <= 512 {
                    return Ok((display_jpg, size));
                }
            } else {
                break;
            }

            max_dim = (max_dim as f32 * 0.75) as u32;
            if max_dim < 256 {
                break;
            }
        }
    }

    // Jika citra asli sudah <= 5 MB dan decode di atas gagal/tidak perlu:
    let orig_size = fs::metadata(source_path).map(|m| m.len()).unwrap_or(0);
    if orig_size <= MAX_DISPLAY_FILE_SIZE_BYTES {
        let _ = fs::copy(source_path, &display_jpg);
        let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
        return Ok((display_jpg, size));
    }

    Err(MapControllerError::InvalidGeometry(
        "Gagal membuat pratinjau display.jpg berukuran di bawah 5 MB dari citra sumber."
            .to_string(),
    ))
}

/// Alur orkestrasi lengkap impor citra orthomosaic JPG/PNG dengan koordinat WGS84.
pub fn process_image_import(
    payload: ImportImagePayload,
) -> Result<StoredImageMetadata, MapControllerError> {
    let source_path = Path::new(&payload.file_path);

    // 1. Validasi Ukuran File (<= 50 MB)
    let size_bytes = validate_image_file_size(source_path)?;

    // 2. Validasi Magic Bytes (JPEG/PNG)
    let ext = validate_magic_bytes(source_path)?;

    // 3. Validasi Dimensi Header (<= 16.384 px)
    let (width, height) = validate_image_dimensions(source_path)?;

    // 4. Validasi Koordinat Bounding Box
    let bounds = validate_wgs84_bounds(payload.west, payload.south, payload.east, payload.north)?;

    // 5. Susun Direktori Atomik di map_storage/images/{image_id}/
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let image_id = format!("ortho_{}", now_ms);
    let target_dir = get_images_dir().join(&image_id);
    fs::create_dir_all(&target_dir)?;

    let original_file = target_dir.join(format!("original.{}", ext));
    fs::copy(source_path, &original_file)?;

    // 6. Buat pratinjau thumbnail display.jpg (maksimal 5 MB)
    let display_size_bytes =
        match generate_display_preview(&original_file, &target_dir, width, height) {
            Ok((_, size)) => Some(size),
            Err(_) => None,
        };

    let raw_name = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&image_id)
        .to_string();

    let aspect_ratio = width as f64 / height as f64;
    let imported_at = chrono::Utc::now().to_rfc3339();

    let web_display_url = format!("/map_storage/images/{}/display.jpg", image_id);
    let web_original_url = format!("/map_storage/images/{}/original.{}", image_id, ext);

    let meta = StoredImageMetadata {
        schema_version: 1,
        id: image_id.clone(),
        filename: raw_name.clone(),
        category: ImageCategory::Ortho,
        extension: ext,
        file_path: web_original_url,
        size_bytes,
        dimensions: Some([width, height]),
        thumbnail_url: Some(web_display_url),
        bounds: Some(bounds),
        aspect_ratio: Some(aspect_ratio),
        display_size_bytes,
        imported_at: Some(imported_at),
        updated_at: None,
        is_corrupted: Some(false),
    };

    // 7. Simpan meta.json secara atomik (<= 64 KB)
    let meta_json = target_dir.join("meta.json");
    let meta_str = serde_json::to_string_pretty(&meta)?;
    if meta_str.len() > 65536 {
        return Err(MapControllerError::InvalidGeometry(
            "Ukuran meta.json melebihi batas 64 KB".to_string(),
        ));
    }
    write_atomic(&meta_json, &meta_str)?;

    // 8. Registrasikan entitas layer otomatis di map_storage/layers/{image_id}.json
    let layer_state = crate::modules::module_03::layers::types::LayerState {
        layer_id: image_id.clone(),
        layer_name: raw_name,
        category: "orthomosaic".to_string(),
        is_visible: true,
        opacity: 1.0,
        z_index: 5,
        source_type: Some("image_overlay".to_string()),
        source_id: Some(image_id),
    };
    let _ = crate::modules::module_03::layers::store::save_layer(layer_state);

    Ok(meta)
}
