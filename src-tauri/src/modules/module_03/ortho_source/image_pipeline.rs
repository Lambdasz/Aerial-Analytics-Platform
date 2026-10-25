//! Pipeline pemrosesan impor citra orthomosaic JPG/PNG dengan koordinat WGS84 murni Rust.
//! Menerapkan fast-fail validation, deteksi JPEG progressive (SOF2), IDCT downscaling (jpeg-decoder),
//! streaming copy original dengan token pembatalan, dan loop kompresi adaptif <= 5 MB.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::imageops;

use super::constants::{
    DISPLAY_MIN_DIMENSION_PX, DISPLAY_TARGET_DIMENSION_PX, MAX_DISPLAY_FILE_SIZE_BYTES,
    MAX_IMAGE_DIMENSION_PX, MAX_IMAGE_FILE_SIZE_BYTES, MAX_PROGRESSIVE_JPEG_PIXELS,
    PREVIEW_TARGET_DIMENSION_PX,
};
use super::job_manager::{save_job_record, CancellationToken, JobRegistrationGuard, StagingGuard};
use super::types::{
    BoundsMetadata, ImageCategory, ImportDualModePayload, ImportImagePayload, JobRecord,
    StoredImageMetadata, TechnicalMetadata,
};
use crate::modules::module_03::error::{MapControllerError, OrthoImportError};
use crate::modules::module_03::ortho_source::store::{get_images_dir, register_ortho_layer};
use crate::modules::module_03::storage_utils::write_atomic;

// ============================================================================
// VALIDASI DASAR & KOMPATIBILITAS KODE LAMA
// ============================================================================

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

    // Deteksi jika user tidak sengaja memasukkan TIFF di jalur Opsi A
    if bytes_read >= 4
        && (header[0..4] == [0x49, 0x49, 0x2A, 0x00] || header[0..4] == [0x4D, 0x4D, 0x00, 0x2A])
    {
        return Err(MapControllerError::InvalidGeometry(
            "Berkas adalah GeoTIFF/TIFF. Harap gunakan Opsi B untuk mengimpor berkas TIFF."
                .to_string(),
        ));
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

/// Menghasilkan pratinjau display.jpg legacy (dipertahankan untuk uji kompatibilitas lama).
pub fn generate_display_preview(
    source_path: &Path,
    target_dir: &Path,
    width: u32,
    height: u32,
) -> Result<(PathBuf, u64), MapControllerError> {
    let display_jpg = target_dir.join("display.jpg");

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

/// Alur orkestrasi lama impor citra orthomosaic JPG/PNG (dipertahankan untuk uji kompatibilitas lama).
pub fn process_image_import(
    payload: ImportImagePayload,
) -> Result<StoredImageMetadata, MapControllerError> {
    let source_path = Path::new(&payload.file_path);
    let size_bytes = validate_image_file_size(source_path)?;
    let ext = validate_magic_bytes(source_path)?;
    let (width, height) = validate_image_dimensions(source_path)?;
    let bounds = validate_wgs84_bounds(payload.west, payload.south, payload.east, payload.north)?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let image_id = format!("ortho_{}", now_ms);
    let target_dir = get_images_dir().join(&image_id);
    fs::create_dir_all(&target_dir)?;

    let original_file = target_dir.join(format!("original.{}", ext));
    fs::copy(source_path, &original_file)?;

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

    let meta_json = target_dir.join("meta.json");
    let meta_str = serde_json::to_string_pretty(&meta)?;
    write_atomic(&meta_json, &meta_str)?;

    let _ = register_ortho_layer(&image_id, &raw_name);

    Ok(meta)
}

// ============================================================================
// OPSI A: PIPELINE KONTROL MEMORI KETAT (PURE RUST)
// ============================================================================

/// Memeriksa ruang disk yang tersedia pada direktori target.
pub fn check_disk_space_preflight(
    target_dir: &Path,
    source_file_size: u64,
    save_original: bool,
) -> Result<(), OrthoImportError> {
    let display_est: u64 = 5 * 1024 * 1024; // 5 MB
    let safety_margin: u64 = 50 * 1024 * 1024; // 50 MB
    let original_space = if save_original { source_file_size } else { 0 };
    let required_bytes = display_est + original_space + safety_margin;

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = target_dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free_bytes_available: u64 = 0;
        let mut total_number_of_bytes: u64 = 0;
        let mut total_number_of_free_bytes: u64 = 0;
        extern "system" {
            fn GetDiskFreeSpaceExW(
                lpDirectoryName: *const u16,
                lpFreeBytesAvailableToCaller: *mut u64,
                lpTotalNumberOfBytes: *mut u64,
                lpTotalNumberOfFreeBytes: *mut u64,
            ) -> i32;
        }
        let res = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_bytes_available,
                &mut total_number_of_bytes,
                &mut total_number_of_free_bytes,
            )
        };
        if res != 0 && free_bytes_available < required_bytes {
            let req_mb = required_bytes as f64 / (1024.0 * 1024.0);
            let free_mb = free_bytes_available as f64 / (1024.0 * 1024.0);
            return Err(OrthoImportError::DiskPenuh(format!(
                "{:.1} MB (tersedia: {:.1} MB)",
                req_mb, free_mb
            )));
        }
    }

    Ok(())
}

/// Mendeteksi apakah berkas JPEG bertipe progressive (SOF2 marker) dan mengekstrak dimensi.
/// Menolak JPEG progressive > 40 MP secara instan tanpa mengalokasikan RAM buffer piksel.
pub fn detect_jpeg_progressive_and_dimensions(
    path: &Path,
) -> Result<(bool, u32, u32), OrthoImportError> {
    let mut file = File::open(path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas JPEG: {}", e)))?;
    let mut buf = Vec::new();
    Read::by_ref(&mut file)
        .take(131072) // Baca header sampai 128 KB
        .read_to_end(&mut buf)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca header JPEG: {}", e)))?;

    if buf.len() < 4 || buf[0] != 0xFF || buf[1] != 0xD8 {
        return Err(OrthoImportError::FormatTidakDidukung(
            "Berkas bukan berkas JPEG yang valid".to_string(),
        ));
    }

    let mut i = 2;
    while i + 8 < buf.len() {
        if buf[i] != 0xFF {
            i += 1;
            continue;
        }

        let marker = buf[i + 1];
        // Lewati padding 0xFF berturut-turut
        if marker == 0xFF || marker == 0x00 {
            i += 1;
            continue;
        }

        // SOI, RST, EOI tidak memiliki panjang segmen
        if (0xD0..=0xD9).contains(&marker) {
            i += 2;
            continue;
        }

        let segment_len = ((buf[i + 2] as usize) << 8) | (buf[i + 3] as usize);

        // SOF2: Progressive DCT
        if marker == 0xC2 {
            let height = ((buf[i + 5] as u32) << 8) | (buf[i + 6] as u32);
            let width = ((buf[i + 7] as u32) << 8) | (buf[i + 8] as u32);
            let total_pixels = width as u64 * height as u64;

            if total_pixels > MAX_PROGRESSIVE_JPEG_PIXELS {
                return Err(OrthoImportError::MelebihiBatas {
                    width,
                    height,
                    max_dimension: 6324,
                    message: format!(
                        "JPEG progressive dengan resolusi {}x{} (total {:.1} MP) melebihi batas aman 40 MP. Harap simpan citra sebagai baseline JPEG atau PNG.",
                        width, height, total_pixels as f64 / 1_000_000.0
                    ),
                });
            }

            return Ok((true, width, height));
        }

        // SOF0 (Baseline) atau SOF1 (Extended sequential)
        if marker == 0xC0 || marker == 0xC1 {
            let height = ((buf[i + 5] as u32) << 8) | (buf[i + 6] as u32);
            let width = ((buf[i + 7] as u32) << 8) | (buf[i + 8] as u32);
            return Ok((false, width, height));
        }

        i += 2 + segment_len;
    }

    // Jika tidak ditemukan dalam 128 KB pertama, gunakan fallback image::image_dimensions
    let (w, h) = image::image_dimensions(path).map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca dimensi berkas JPEG: {}", e))
    })?;
    Ok((false, w, h))
}

/// Mendecode baseline JPEG dengan IDCT Downscaling ke batas target (4.096 px)
/// tanpa mengalokasikan buffer unscaled 144 MP penuh.
pub fn decode_and_scale_jpeg_idct(
    source_path: &Path,
    target_dim: u32,
) -> Result<image::RgbImage, OrthoImportError> {
    let file = File::open(source_path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas JPEG: {}", e)))?;
    let buf_reader = BufReader::new(file);

    let mut decoder = jpeg_decoder::Decoder::new(buf_reader);
    decoder.read_info().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca info header JPEG: {}", e))
    })?;

    let info = decoder
        .info()
        .ok_or_else(|| OrthoImportError::FileKorup("Header JPEG tidak memuat info".to_string()))?;
    let (orig_w, orig_h) = (info.width as u32, info.height as u32);

    let longest = orig_w.max(orig_h);
    let (scaled_w, scaled_h) = if longest > target_dim {
        let scale = target_dim as f32 / longest as f32;
        let tw = ((orig_w as f32 * scale).round() as u16).max(1);
        let th = ((orig_h as f32 * scale).round() as u16).max(1);
        // Terapkan IDCT scaling pada decoder sebelum decoding piksel
        decoder.scale(tw, th).map_err(|e| {
            OrthoImportError::InternalError(format!("Gagal mengatur IDCT scale: {}", e))
        })?
    } else {
        (orig_w as u16, orig_h as u16)
    };

    let pixels = decoder
        .decode()
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal mendecode JPEG: {}", e)))?;

    let rgb_img =
        image::RgbImage::from_raw(scaled_w as u32, scaled_h as u32, pixels).ok_or_else(|| {
            OrthoImportError::InternalError("Ukuran buffer piksel JPEG tidak sesuai".to_string())
        })?;

    // Jika hasil IDCT masih sedikit lebih besar dari target_dim (misal 5.000 px),
    // downsample presisi ke target_dim
    let cur_longest = (scaled_w as u32).max(scaled_h as u32);
    if cur_longest > target_dim {
        let scale = target_dim as f32 / cur_longest as f32;
        let final_w = ((scaled_w as f32 * scale).round() as u32).max(1);
        let final_h = ((scaled_h as f32 * scale).round() as u32).max(1);
        let resized = imageops::resize(&rgb_img, final_w, final_h, imageops::FilterType::Triangle);
        Ok(resized)
    } else {
        Ok(rgb_img)
    }
}

/// Mendecode PNG dan memeriksa channel alpha.
pub fn decode_and_scale_png(
    source_path: &Path,
    target_dim: u32,
) -> Result<(image::DynamicImage, bool), OrthoImportError> {
    let file = File::open(source_path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas PNG: {}", e)))?;
    let buf_reader = BufReader::new(file);

    let decoder = png::Decoder::new(buf_reader);
    let mut reader = decoder
        .read_info()
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca info PNG: {}", e)))?;

    let info = reader.info();
    let (orig_w, orig_h) = (info.width, info.height);
    let color_type = info.color_type;
    let has_alpha = color_type == png::ColorType::Rgba
        || color_type == png::ColorType::GrayscaleAlpha
        || color_type == png::ColorType::Indexed;

    if orig_w > MAX_IMAGE_DIMENSION_PX || orig_h > MAX_IMAGE_DIMENSION_PX {
        return Err(OrthoImportError::MelebihiBatas {
            width: orig_w,
            height: orig_h,
            max_dimension: MAX_IMAGE_DIMENSION_PX,
            message: format!(
                "Dimensi PNG {}x{} melebihi batas aman {} piksel.",
                orig_w, orig_h, MAX_IMAGE_DIMENSION_PX
            ),
        });
    }

    let mut buf = vec![0; reader.output_buffer_size()];
    let output_info = reader
        .next_frame(&mut buf)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal mendecode frame PNG: {}", e)))?;
    buf.truncate(output_info.buffer_size());

    let dynamic_img = if has_alpha {
        let rgba = image::RgbaImage::from_raw(output_info.width, output_info.height, buf)
            .ok_or_else(|| {
                OrthoImportError::InternalError("Buffer RGBA PNG tidak sesuai".to_string())
            })?;
        image::DynamicImage::ImageRgba8(rgba)
    } else {
        let rgb = image::RgbImage::from_raw(output_info.width, output_info.height, buf)
            .ok_or_else(|| {
                OrthoImportError::InternalError("Buffer RGB PNG tidak sesuai".to_string())
            })?;
        image::DynamicImage::ImageRgb8(rgb)
    };

    let longest = orig_w.max(orig_h);
    if longest > target_dim {
        let scale = target_dim as f32 / longest as f32;
        let tw = ((orig_w as f32 * scale).round() as u32).max(1);
        let th = ((orig_h as f32 * scale).round() as u32).max(1);
        let resized = dynamic_img.resize(tw, th, imageops::FilterType::Triangle);
        Ok((resized, has_alpha))
    } else {
        Ok((dynamic_img, has_alpha))
    }
}

/// Salin berkas asli secara streaming ber-buffer dengan dukungan token pembatalan dan pembaruan progres.
pub fn stream_copy_with_cancellation<F>(
    source_path: &Path,
    target_path: &Path,
    cancel_token: &CancellationToken,
    mut on_progress: F,
) -> Result<u64, OrthoImportError>
where
    F: FnMut(f64),
{
    let mut src = File::open(source_path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas sumber: {}", e)))?;
    let total_bytes = src
        .metadata()
        .map(|m| m.len())
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca metadata file: {}", e)))?;

    let dst = File::create(target_path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuat berkas tujuan: {}", e))
    })?;
    let mut writer = BufWriter::new(dst);

    let mut buffer = [0u8; 65536]; // 64 KB buffer
    let mut bytes_copied: u64 = 0;

    loop {
        if cancel_token.is_cancelled() {
            return Err(OrthoImportError::Dibatalkan);
        }

        let bytes_read = src.read(&mut buffer).map_err(|e| {
            OrthoImportError::InternalError(format!("Gagal membaca buffer streaming: {}", e))
        })?;

        if bytes_read == 0 {
            break;
        }

        writer.write_all(&buffer[..bytes_read]).map_err(|e| {
            OrthoImportError::InternalError(format!("Gagal menulis buffer streaming: {}", e))
        })?;

        bytes_copied += bytes_read as u64;

        if total_bytes > 0 {
            let pct = (bytes_copied as f64 / total_bytes as f64) * 100.0;
            on_progress(pct);
        }
    }

    writer.flush().map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal flush buffer streaming: {}", e))
    })?;

    Ok(bytes_copied)
}

/// Hasil dari loop kompresi adaptif display.
pub struct AdaptiveEncodeResult {
    pub display_path: PathBuf,
    pub display_size_bytes: u64,
    pub final_dimensions: [u32; 2],
    pub format: String,
    pub warnings: Vec<String>,
}

/// Loop kompresi adaptif untuk memastikan berkas display <= 5 MB:
/// - Urutan JPEG: Kualitas dulu (85, 80, 75, 70), baru dimensi (3072, 2048, 1536, 1024).
/// - PNG dengan alpha: Tidak menurunkan resolusi diam-diam, catat resolusi akhir di warnings,
///   dan tambahkan peringatan jika < 2.048 px.
pub fn adaptive_encode_display(
    image: &image::DynamicImage,
    target_dir: &Path,
    has_alpha: bool,
) -> Result<AdaptiveEncodeResult, OrthoImportError> {
    let mut warnings = Vec::new();

    if has_alpha {
        // Enkode PNG untuk mempertahankan transparansi
        let display_png = target_dir.join("display.png");
        let (orig_w, orig_h) = (image.width(), image.height());
        let mut cur_dim = DISPLAY_TARGET_DIMENSION_PX.min(orig_w.max(orig_h));

        loop {
            let scale = cur_dim as f32 / orig_w.max(orig_h) as f32;
            let tw = ((orig_w as f32 * scale).round() as u32).max(1);
            let th = ((orig_h as f32 * scale).round() as u32).max(1);

            let resized = if tw == orig_w && th == orig_h {
                image.clone()
            } else {
                image.resize(tw, th, imageops::FilterType::Triangle)
            };

            resized
                .save_with_format(&display_png, image::ImageFormat::Png)
                .map_err(|e| {
                    OrthoImportError::InternalError(format!("Gagal mengenkode display PNG: {}", e))
                })?;

            let size = fs::metadata(&display_png).map(|m| m.len()).unwrap_or(0);
            if size <= MAX_DISPLAY_FILE_SIZE_BYTES || cur_dim <= DISPLAY_MIN_DIMENSION_PX {
                if tw < orig_w || th < orig_h {
                    warnings.push(format!(
                        "Resolusi pratinjau PNG disesuaikan ke {}x{} agar muat dalam batas 5 MB",
                        tw, th
                    ));
                }
                if cur_dim < 2048 {
                    warnings.push(format!(
                        "Peringatan: Resolusi akhir pratinjau PNG ({}x{}) berada di bawah 2.048 piksel",
                        tw, th
                    ));
                }
                return Ok(AdaptiveEncodeResult {
                    display_path: display_png,
                    display_size_bytes: size,
                    final_dimensions: [tw, th],
                    format: "png".to_string(),
                    warnings,
                });
            }

            // Kurangi dimensi bertahap
            cur_dim = match cur_dim {
                d if d > 3072 => 3072,
                d if d > 2048 => 2048,
                d if d > 1536 => 1536,
                d if d > 1024 => 1024,
                _ => break,
            };
        }

        let size = fs::metadata(&display_png).map(|m| m.len()).unwrap_or(0);
        Ok(AdaptiveEncodeResult {
            display_path: display_png,
            display_size_bytes: size,
            final_dimensions: [image.width(), image.height()],
            format: "png".to_string(),
            warnings,
        })
    } else {
        // Enkode JPEG: Kualitas dulu, baru dimensi
        let display_jpg = target_dir.join("display.jpg");
        let rgb_img = image.to_rgb8();
        let (orig_w, orig_h) = (rgb_img.width(), rgb_img.height());

        let quality_candidates = [85u8, 80u8, 75u8, 70u8];
        let dimension_candidates = [
            DISPLAY_TARGET_DIMENSION_PX.min(orig_w.max(orig_h)),
            3072,
            2048,
            1536,
            1024,
        ];

        for &dim in &dimension_candidates {
            let scale = dim as f32 / orig_w.max(orig_h) as f32;
            let tw = ((orig_w as f32 * scale).round() as u32).max(1);
            let th = ((orig_h as f32 * scale).round() as u32).max(1);

            let resized = if tw == orig_w && th == orig_h {
                rgb_img.clone()
            } else {
                imageops::resize(&rgb_img, tw, th, imageops::FilterType::Triangle)
            };

            for &quality in &quality_candidates {
                let file = File::create(&display_jpg).map_err(|e| {
                    OrthoImportError::InternalError(format!(
                        "Gagal membuat berkas display.jpg: {}",
                        e
                    ))
                })?;
                let mut writer = BufWriter::new(file);

                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
                encoder
                    .encode_image(&resized)
                    .map_err(|e| OrthoImportError::InternalError(format!("Gagal encode: {}", e)))?;
                writer
                    .flush()
                    .map_err(|e| OrthoImportError::InternalError(format!("Gagal flush: {}", e)))?;

                let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
                if size <= MAX_DISPLAY_FILE_SIZE_BYTES {
                    if tw < orig_w || th < orig_h {
                        warnings.push(format!(
                            "Resolusi display diadaptasi ke {}x{} (kualitas {})",
                            tw, th, quality
                        ));
                    }
                    return Ok(AdaptiveEncodeResult {
                        display_path: display_jpg,
                        display_size_bytes: size,
                        final_dimensions: [tw, th],
                        format: "jpg".to_string(),
                        warnings,
                    });
                }
            }
        }

        let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
        Ok(AdaptiveEncodeResult {
            display_path: display_jpg,
            display_size_bytes: size,
            final_dimensions: [orig_w, orig_h],
            format: "jpg".to_string(),
            warnings,
        })
    }
}

/// Orkestrasi eksekusi impor Opsi A secara asinkron.
pub fn execute_import_option_a(
    job_id: &str,
    payload: ImportDualModePayload,
    cancel_token: CancellationToken,
) -> Result<StoredImageMetadata, OrthoImportError> {
    let _job_guard = JobRegistrationGuard::new(job_id.to_string());
    let source_path = PathBuf::from(&payload.file_path);

    let update_progress = |stage: &str, pct: f64, msg: &str| {
        let record = JobRecord {
            job_id: job_id.to_string(),
            status: "running".to_string(),
            stage: stage.to_string(),
            progress_percent: pct,
            message: msg.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
            result_image_id: None,
        };
        let _ = save_job_record(&record);
    };

    // 1. TAHAP VALIDASI
    update_progress("validasi", 5.0, "Memvalidasi berkas dan koordinat...");
    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    if !source_path.is_file() {
        return Err(OrthoImportError::FileKorup(format!(
            "Berkas citra tidak ditemukan di disk: {}",
            source_path.display()
        )));
    }

    let source_size_bytes = fs::metadata(&source_path)
        .map(|m| m.len())
        .map_err(|e| OrthoImportError::FileKorup(e.to_string()))?;

    if source_size_bytes > MAX_IMAGE_FILE_SIZE_BYTES {
        return Err(OrthoImportError::MelebihiBatas {
            width: 0,
            height: 0,
            max_dimension: 0,
            message: format!(
                "Ukuran berkas ({:.1} MB) melebihi batas maksimal 50 MB.",
                source_size_bytes as f64 / (1024.0 * 1024.0)
            ),
        });
    }

    let ext = validate_magic_bytes(&source_path)
        .map_err(|e| OrthoImportError::FormatTidakDidukung(e.to_string()))?;

    // Periksa fast-fail JPEG progressive > 40 MP
    if ext == "jpg" {
        let (is_prog, w, h) = detect_jpeg_progressive_and_dimensions(&source_path)?;
        if is_prog && (w as u64 * h as u64) > MAX_PROGRESSIVE_JPEG_PIXELS {
            return Err(OrthoImportError::MelebihiBatas {
                width: w,
                height: h,
                max_dimension: 6324,
                message: "JPEG progressive di atas 40 MP tidak didukung".to_string(),
            });
        }
    }

    let (orig_width, orig_height) = validate_image_dimensions(&source_path)
        .map_err(|e| OrthoImportError::FileKorup(e.to_string()))?;

    // Tentukan Bounding Box (Hierarki: World File -> Sidecar JSON -> Manual)
    let mut bounds_source = "manual".to_string();
    let mut detected_crs_code = 4326u32;
    let mut _prj_path_found = None;

    let bounds = if let Some(world_file) =
        super::world_file_reader::find_candidate_world_file(&source_path)
    {
        let prj = super::world_file_reader::find_candidate_prj_file(&source_path);
        let wf_res = super::world_file_reader::read_world_file_envelope(
            &world_file,
            prj.as_deref(),
            orig_width,
            orig_height,
        )?;
        bounds_source = "world_file".to_string();
        detected_crs_code = wf_res.detected_crs;
        _prj_path_found = wf_res.prj_file_path;
        wf_res.bounds
    } else if let Some(sidecar_file) =
        super::sidecar_reader::find_candidate_sidecar_file(&source_path)
    {
        let s_bounds = super::sidecar_reader::read_sidecar_envelope(&sidecar_file)?;
        bounds_source = "sidecar_json".to_string();
        s_bounds
    } else if let Some(ref manual) = payload.manual_bounds {
        validate_wgs84_bounds(manual.west, manual.south, manual.east, manual.north)
            .map_err(|e| OrthoImportError::KoordinatTidakValid(e.to_string()))?
    } else {
        return Err(OrthoImportError::KoordinatTidakValid(
            "Tidak ditemukan informasi koordinat georeferensi (World File / Sidecar JSON) dan koordinat manual tidak disediakan."
                .to_string(),
        ));
    };

    // Ekstrak EXIF (hanya titik kamera drone)
    let exif_meta = super::exif_reader::read_exif_metadata(&source_path).unwrap_or(None);

    // Siapkan Guard Staging RAII yang aman
    let mut staging_guard = StagingGuard::for_job(job_id)?;
    let staging_dir = staging_guard.staging_dir().to_path_buf();
    fs::create_dir_all(&staging_dir).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuat folder staging: {}", e))
    })?;

    // Preflight cek ruang hard disk
    check_disk_space_preflight(&staging_dir, source_size_bytes, payload.save_original)?;

    // 2. TAHAP SALIN ORIGINAL STREAMING
    let normalized_ext = if ext == "png" { "png" } else { "jpg" };
    let original_staging_file = staging_dir.join(format!("original.{}", normalized_ext));

    if payload.save_original {
        update_progress("salin_asli", 15.0, "Menyalin berkas citra asli...");
        stream_copy_with_cancellation(
            &source_path,
            &original_staging_file,
            &cancel_token,
            |pct| {
                let overall = 15.0 + (pct * 0.20); // 15% sampai 35%
                update_progress("salin_asli", overall, "Menyalin berkas citra asli...");
            },
        )?;
    }

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    // 3. TAHAP DEKODE & DOWNSCALING
    update_progress(
        "dekompresi",
        40.0,
        "Mendekompresi dan meresampling citra...",
    );

    let (dynamic_image, has_alpha) = if ext == "jpg" {
        let rgb = decode_and_scale_jpeg_idct(&source_path, DISPLAY_TARGET_DIMENSION_PX)?;
        (image::DynamicImage::ImageRgb8(rgb), false)
    } else {
        decode_and_scale_png(&source_path, DISPLAY_TARGET_DIMENSION_PX)?
    };

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    // 4. TAHAP LOOP ADAPTIF DISPLAY <= 5 MB
    update_progress(
        "loop_adaptif",
        70.0,
        "Mengoptimalkan ukuran berkas display...",
    );
    let encode_res = adaptive_encode_display(&dynamic_image, &staging_dir, has_alpha)?;

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    // 5. TAHAP POINT OF NO RETURN: RENAME FOLDER & REGISTRASI ATOMIK
    update_progress(
        "registrasi",
        90.0,
        "Mendaftarkan layer dan menyimpan metadata...",
    );

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let image_id = format!("ortho_{}", now_ms);
    let final_image_dir = get_images_dir().join(&image_id);

    // Tulis TechnicalMetadata (metadata.json) ke staging sebelum rename
    let tech_meta = TechnicalMetadata {
        schema_version: 1,
        image_id: image_id.clone(),
        import_mode: "A".to_string(),
        source_format: ext.clone(),
        source_filename: payload.custom_filename.clone().unwrap_or_else(|| {
            source_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("ortho_image")
                .to_string()
        }),
        source_dimensions: [orig_width, orig_height],
        display_dimensions: encode_res.final_dimensions,
        source_file_size_bytes: source_size_bytes,
        display_file_size_bytes: encode_res.display_size_bytes,
        display_format: encode_res.format.clone(),
        bounds: BoundsMetadata {
            south: bounds[0][0],
            west: bounds[0][1],
            north: bounds[1][0],
            east: bounds[1][1],
            source: bounds_source,
        },
        source_crs: if detected_crs_code == 4326 {
            "EPSG:4326 (WGS84)".to_string()
        } else {
            format!("EPSG:{}", detected_crs_code)
        },
        exif_summary: exif_meta,
        tiff_technical_details: None,
        imported_at: chrono::Utc::now().to_rfc3339(),
        warnings: encode_res.warnings.clone(),
    };

    let tech_json_path = staging_dir.join("metadata.json");
    let tech_json_str = serde_json::to_string_pretty(&tech_meta).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal serialisasi metadata.json: {}", e))
    })?;
    write_atomic(&tech_json_path, &tech_json_str).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal menulis metadata.json: {}", e))
    })?;

    // Tulis StoredImageMetadata (meta.json) ke staging sebelum rename
    let final_size_bytes = if payload.save_original {
        source_size_bytes
    } else {
        encode_res.display_size_bytes
    };
    let final_ext = if payload.save_original {
        normalized_ext.to_string()
    } else {
        encode_res.format.clone()
    };

    let display_rel_url = format!(
        "/map_storage/images/{}/display.{}",
        image_id, encode_res.format
    );
    let original_rel_url = if payload.save_original {
        format!(
            "/map_storage/images/{}/original.{}",
            image_id, normalized_ext
        )
    } else {
        display_rel_url.clone()
    };

    let layer_name = payload
        .custom_filename
        .clone()
        .unwrap_or_else(|| tech_meta.source_filename.clone());

    let stored_meta = StoredImageMetadata {
        schema_version: 1,
        id: image_id.clone(),
        filename: layer_name.clone(),
        category: ImageCategory::Ortho,
        extension: final_ext,
        file_path: original_rel_url,
        size_bytes: final_size_bytes,
        dimensions: Some([orig_width, orig_height]),
        thumbnail_url: Some(display_rel_url),
        bounds: Some(bounds),
        aspect_ratio: Some(orig_width as f64 / orig_height as f64),
        display_size_bytes: Some(encode_res.display_size_bytes),
        imported_at: Some(chrono::Utc::now().to_rfc3339()),
        updated_at: None,
        is_corrupted: Some(false),
    };

    let meta_json_path = staging_dir.join("meta.json");
    let meta_json_str = serde_json::to_string_pretty(&stored_meta).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal serialisasi meta.json: {}", e))
    })?;
    write_atomic(&meta_json_path, &meta_json_str)
        .map_err(|e| OrthoImportError::InternalError(format!("Gagal menulis meta.json: {}", e)))?;

    // POINT OF NO RETURN: Rename folder staging ke final
    fs::rename(&staging_dir, &final_image_dir).map_err(|e| {
        OrthoImportError::InternalError(format!(
            "Gagal memindahkan folder staging ke tujuan akhir: {}",
            e
        ))
    })?;
    staging_guard.disarm(); // Matikan guard agar folder final tidak dihapus saat drop

    // Registrasi layer atomik ke map_storage/layers/{id}.json
    let _ = register_ortho_layer(&image_id, &layer_name);

    // 6. TAHAP SELESAI
    let final_record = JobRecord {
        job_id: job_id.to_string(),
        status: "completed".to_string(),
        stage: "selesai".to_string(),
        progress_percent: 100.0,
        message: "Impor citra berhasil diselesaikan".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        error: None,
        result_image_id: Some(image_id),
    };
    let _ = save_job_record(&final_record);

    Ok(stored_meta)
}
