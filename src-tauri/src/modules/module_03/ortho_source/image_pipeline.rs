//! Pipeline pemrosesan impor citra orthomosaic JPG/PNG dengan koordinat WGS84 murni Rust.
//! Menerapkan fast-fail validation, deteksi JPEG progressive (SOF2), IDCT downscaling (jpeg-decoder),
//! streaming copy original dengan token pembatalan, dan loop kompresi adaptif (JPEG <= 5 MiB, PNG alpha <= 10 MiB).

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::constants::{
    DISPLAY_MAX_BYTES_JPEG, DISPLAY_MAX_BYTES_PNG_ALPHA, DISPLAY_MIN_DIMENSION_PX,
    DISPLAY_TARGET_DIMENSION_PX, MAX_DISPLAY_FILE_SIZE_BYTES, MAX_IMAGE_DIMENSION_PX,
    MAX_IMAGE_FILE_SIZE_BYTES, MAX_PROGRESSIVE_JPEG_PIXELS, PNG_DIMENSION_LADDER,
    PREVIEW_TARGET_DIMENSION_PX,
};
use super::job_manager::{save_job_record, CancellationToken, JobRegistrationGuard, StagingGuard};
use super::streaming_resampler::{ChannelCount, StreamingResampler};
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
            let resized = if max_dim >= width.max(height) {
                img.clone()
            } else if let Ok(r) = resample_dynamic_image_box_filter(&img, max_dim) {
                r
            } else {
                break;
            };
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
        "Gagal membuat pratinjau display.jpg berukuran di bawah 5 MiB dari citra sumber."
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

    let (display_path, display_size_bytes) =
        match generate_display_preview(&original_file, &target_dir, width, height) {
            Ok((p, size)) => (Some(p), Some(size)),
            Err(_) => (None, None),
        };
    let display_dimensions =
        display_path.and_then(|p| image::image_dimensions(p).ok().map(|(w, h)| [w, h]));

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
        display_dimensions,
        warnings: None,
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
            /// Windows API call to retrieve free disk space on a volume.
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

    #[cfg(not(windows))]
    {
        if let Ok(free_bytes) = fs4::available_space(target_dir) {
            if free_bytes < required_bytes {
                let req_mb = required_bytes as f64 / (1024.0 * 1024.0);
                let free_mb = free_bytes as f64 / (1024.0 * 1024.0);
                return Err(OrthoImportError::DiskPenuh(format!(
                    "{:.1} MB (tersedia: {:.1} MB)",
                    req_mb, free_mb
                )));
            }
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

        // Penolakan Kasus Tepi: Arithmetic Coding (SOF9 = 0xC9, SOF10 = 0xCA, SOF11 = 0xCB, SOF13..=SOF15)
        if marker == 0xC9 || marker == 0xCA || marker == 0xCB || (0xCD..=0xCF).contains(&marker) {
            return Err(OrthoImportError::FormatTidakDidukung(
                "JPEG dengan pengkodean arithmetic (SOF9/SOF10) tidak didukung. Harap simpan citra dengan Huffman coding standar."
                    .to_string(),
            ));
        }

        // Marker SOF (Start of Frame) umum: 0xC0 (Baseline), 0xC1 (Extended), 0xC2 (Progressive), 0xC3 (Lossless)
        if (0xC0..=0xC7).contains(&marker) && marker != 0xC4 && i + 9 < buf.len() {
            let precision = buf[i + 4];
            if precision == 12 {
                return Err(OrthoImportError::FormatTidakDidukung(
                    "JPEG 12-bit tidak didukung. Hanya JPEG 8-bit yang didukung.".to_string(),
                ));
            }

            let components = buf[i + 9];
            if components == 4 {
                return Err(OrthoImportError::FormatTidakDidukung(
                    "JPEG dengan 4 komponen warna (CMYK/YCCK) tidak didukung. Harap konversi citra ke ruang warna RGB."
                        .to_string(),
                ));
            }
        }

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
/// lalu disuapkan per baris ke StreamingResampler (Box filter / Area Averaging)
/// tanpa mengalokasikan intermediate resize buffer.
pub fn decode_and_scale_jpeg_idct(
    source_path: &Path,
    target_dim: u32,
) -> Result<image::RgbImage, OrthoImportError> {
    let decoded =
        crate::modules::module_03::ortho_source::jpeg_wrapper::decode_jpeg_with_idct_scale(
            source_path,
            target_dim,
        )?;
    let (scaled_w, scaled_h) = (decoded.width, decoded.height);

    let cur_longest = scaled_w.max(scaled_h);
    if cur_longest > target_dim && target_dim > 0 {
        let mut resampler =
            StreamingResampler::new(scaled_w, scaled_h, target_dim, ChannelCount::Rgb)?;
        let stride = scaled_w as usize * 3;
        for y in 0..scaled_h {
            let row_start = y as usize * stride;
            let row_end = row_start + stride;
            resampler.push_row_u8(y, &decoded.pixels[row_start..row_end], None)?;
        }
        let resampled = resampler.finish()?;
        image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data).ok_or_else(
            || {
                OrthoImportError::InternalError(
                    "Ukuran buffer hasil resampling JPEG tidak sesuai".to_string(),
                )
            },
        )
    } else {
        image::RgbImage::from_raw(scaled_w, scaled_h, decoded.pixels).ok_or_else(|| {
            OrthoImportError::InternalError("Ukuran buffer piksel JPEG tidak sesuai".to_string())
        })
    }
}

/// Mendecode PNG dan memeriksa channel alpha menggunakan StreamingResampler berbasis baris.
/// Mencegah alokasi buffer frame 100 MP penuh untuk PNG non-interlaced (Reader::next_row).
/// Untuk PNG interlaced (Adam7), batas aman ditetapkan <= 25 MP karena butuh frame penuh.
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
    let is_interlaced = info.interlaced;

    // Penolakan Kasus Tepi: PNG 16-bit
    if info.bit_depth == png::BitDepth::Sixteen {
        return Err(OrthoImportError::FormatTidakDidukung(
            "PNG 16-bit tidak didukung pada Opsi A. Harap simpan citra sebagai PNG 8-bit atau gunakan Opsi B untuk citra sensor kedalaman tinggi."
                .to_string(),
        ));
    }

    // Penolakan Kasus Tepi: PNG Palette / Indexed
    if color_type == png::ColorType::Indexed {
        return Err(OrthoImportError::FormatTidakDidukung(
            "PNG Palette (Indexed Color) tidak didukung. Harap konversi citra ke format warna RGB atau RGBA."
                .to_string(),
        ));
    }

    let has_alpha =
        color_type == png::ColorType::Rgba || color_type == png::ColorType::GrayscaleAlpha;

    let total_pixels = orig_w as u64 * orig_h as u64;

    // Batas piksel Opsi A umum (144 MP)
    if orig_w > MAX_IMAGE_DIMENSION_PX
        || orig_h > MAX_IMAGE_DIMENSION_PX
        || total_pixels > 144_000_000
    {
        return Err(OrthoImportError::MelebihiBatas {
            width: orig_w,
            height: orig_h,
            max_dimension: MAX_IMAGE_DIMENSION_PX,
            message: format!(
                "Dimensi PNG {}x{} melebihi batas aman 144 MP.",
                orig_w, orig_h
            ),
        });
    }

    // Batas khusus PNG Interlaced (Adam7) <= 25 MP:
    // Karena Adam7 memerlukan dekompresi buffer frame penuh di memori untuk mengumpulkan semua pass.
    if is_interlaced && total_pixels > 25_000_000 {
        return Err(OrthoImportError::MelebihiBatas {
            width: orig_w,
            height: orig_h,
            max_dimension: 5000,
            message: format!(
                "PNG interlaced (Adam7) dengan resolusi {}x{} ({:.1} MP) melebihi batas aman 25 MP karena memerlukan alokasi frame penuh di memori. Harap simpan citra sebagai PNG non-interlaced atau JPEG.",
                orig_w, orig_h, total_pixels as f64 / 1_000_000.0
            ),
        });
    }

    let channel_count = if has_alpha {
        ChannelCount::Rgba
    } else {
        ChannelCount::Rgb
    };

    let mut resampler = StreamingResampler::new(orig_w, orig_h, target_dim, channel_count)?;

    if is_interlaced {
        // Interlaced (Adam7) <= 25 MP: decode frame penuh lalu disuapkan per baris ke resampler
        let mut buf = vec![0; reader.output_buffer_size()];
        let output_info = reader.next_frame(&mut buf).map_err(|e| {
            OrthoImportError::FileKorup(format!("Gagal mendecode frame PNG interlaced: {}", e))
        })?;
        buf.truncate(output_info.buffer_size());

        let stride = match color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            _ => 3,
        };

        for y in 0..orig_h {
            let row_start = (y as usize * orig_w as usize) * stride;
            let row_end = row_start + orig_w as usize * stride;
            let row_slice = &buf[row_start..row_end];

            match color_type {
                png::ColorType::Rgba | png::ColorType::Rgb => {
                    resampler.push_row_u8(y, row_slice, None)?;
                }
                png::ColorType::Grayscale => {
                    let mut rgb_row = Vec::with_capacity(orig_w as usize * 3);
                    for &g in row_slice {
                        rgb_row.extend_from_slice(&[g, g, g]);
                    }
                    resampler.push_row_u8(y, &rgb_row, None)?;
                }
                png::ColorType::GrayscaleAlpha => {
                    let mut rgba_row = Vec::with_capacity(orig_w as usize * 4);
                    for chunk in row_slice.chunks_exact(2) {
                        rgba_row.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
                    }
                    resampler.push_row_u8(y, &rgba_row, None)?;
                }
                _ => {}
            }
        }
    } else {
        // Non-interlaced: streaming langsung per baris via reader.next_row()!
        // Alokasi memori HANYA 1 baris input (< 50 KB), bukan 400 MB!
        let mut y_in = 0u32;
        while let Some(row) = reader
            .next_row()
            .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca baris PNG: {}", e)))?
        {
            let row_slice = row.data();
            match color_type {
                png::ColorType::Rgba | png::ColorType::Rgb => {
                    resampler.push_row_u8(y_in, row_slice, None)?;
                }
                png::ColorType::Grayscale => {
                    let mut rgb_row = Vec::with_capacity(orig_w as usize * 3);
                    for &g in row_slice {
                        rgb_row.extend_from_slice(&[g, g, g]);
                    }
                    resampler.push_row_u8(y_in, &rgb_row, None)?;
                }
                png::ColorType::GrayscaleAlpha => {
                    let mut rgba_row = Vec::with_capacity(orig_w as usize * 4);
                    for chunk in row_slice.chunks_exact(2) {
                        rgba_row.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
                    }
                    resampler.push_row_u8(y_in, &rgba_row, None)?;
                }
                _ => {}
            }
            y_in += 1;
        }
    }

    let resampled = resampler.finish()?;
    if has_alpha {
        let rgba = image::RgbaImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError(
                    "Buffer RGBA hasil resampling tidak sesuai".to_string(),
                )
            })?;
        Ok((image::DynamicImage::ImageRgba8(rgba), true))
    } else {
        let rgb = image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError(
                    "Buffer RGB hasil resampling tidak sesuai".to_string(),
                )
            })?;
        Ok((image::DynamicImage::ImageRgb8(rgb), false))
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

/// Resample sebuah DynamicImage ke batas target menggunakan StreamingResampler (Box filter / Area Averaging).
pub fn resample_dynamic_image_box_filter(
    image: &image::DynamicImage,
    target_dim: u32,
) -> Result<image::DynamicImage, OrthoImportError> {
    let (orig_w, orig_h) = (image.width(), image.height());
    let longest = orig_w.max(orig_h);
    if longest <= target_dim || target_dim == 0 {
        return Ok(image.clone());
    }

    let has_alpha = image.color().has_alpha();
    let channel_count = if has_alpha {
        ChannelCount::Rgba
    } else {
        ChannelCount::Rgb
    };

    let mut resampler = StreamingResampler::new(orig_w, orig_h, target_dim, channel_count)?;
    let stride = orig_w as usize * channel_count.channels();

    if has_alpha {
        let rgba = image.to_rgba8();
        let raw = rgba.as_raw();
        for y in 0..orig_h {
            let start = y as usize * stride;
            let end = start + stride;
            resampler.push_row_u8(y, &raw[start..end], None)?;
        }
        let resampled = resampler.finish()?;
        let out_img = image::RgbaImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError("Buffer RGBA resample gagal".to_string())
            })?;
        Ok(image::DynamicImage::ImageRgba8(out_img))
    } else {
        let rgb = image.to_rgb8();
        let raw = rgb.as_raw();
        for y in 0..orig_h {
            let start = y as usize * stride;
            let end = start + stride;
            resampler.push_row_u8(y, &raw[start..end], None)?;
        }
        let resampled = resampler.finish()?;
        let out_img = image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError("Buffer RGB resample gagal".to_string())
            })?;
        Ok(image::DynamicImage::ImageRgb8(out_img))
    }
}

/// Resample sebuah RgbImage ke batas target menggunakan StreamingResampler (Box filter / Area Averaging).
pub fn resample_rgb_image_box_filter(
    rgb: &image::RgbImage,
    target_dim: u32,
) -> Result<image::RgbImage, OrthoImportError> {
    let (orig_w, orig_h) = (rgb.width(), rgb.height());
    let longest = orig_w.max(orig_h);
    if longest <= target_dim || target_dim == 0 {
        return Ok(rgb.clone());
    }

    let mut resampler = StreamingResampler::new(orig_w, orig_h, target_dim, ChannelCount::Rgb)?;
    let stride = orig_w as usize * 3;
    let raw = rgb.as_raw();

    for y in 0..orig_h {
        let start = y as usize * stride;
        let end = start + stride;
        resampler.push_row_u8(y, &raw[start..end], None)?;
    }

    let resampled = resampler.finish()?;
    image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
        .ok_or_else(|| OrthoImportError::InternalError("Buffer RGB resample gagal".to_string()))
}

/// Wrapper `std::io::Write` + `std::io::Seek` yang memeriksa `CancellationToken` pada setiap blok write.
/// Memungkinkan pembatalan segera (latensi rendah) bahkan saat proses encode JPEG / PNG
/// monolitik berukuran besar sedang berjalan.
pub struct CancellableWriter<'a, W: Write> {
    inner: W,
    token: Option<&'a CancellationToken>,
}

impl<'a, W: Write> CancellableWriter<'a, W> {
    /// Creates a new `CancellableWriter` wrapping an inner writer with an optional cancellation token.
    pub fn new(inner: W, token: Option<&'a CancellationToken>) -> Self {
        Self { inner, token }
    }
}

impl<'a, W: Write> Write for CancellableWriter<'a, W> {
    /// Writes buffer to inner writer, returning error if cancelled.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(t) = self.token {
            if t.is_cancelled() {
                return Err(std::io::Error::other(
                    "Operasi dibatalkan saat proses kompresi display",
                ));
            }
        }
        self.inner.write(buf)
    }

    /// Flushes inner writer, returning error if cancelled.
    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(t) = self.token {
            if t.is_cancelled() {
                return Err(std::io::Error::other(
                    "Operasi dibatalkan saat proses kompresi display",
                ));
            }
        }
        self.inner.flush()
    }
}

impl<'a, W: Write + Seek> Seek for CancellableWriter<'a, W> {
    /// Seeks inner writer, returning error if cancelled.
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        if let Some(t) = self.token {
            if t.is_cancelled() {
                return Err(std::io::Error::other(
                    "Operasi dibatalkan saat proses kompresi display",
                ));
            }
        }
        self.inner.seek(pos)
    }
}

/// Batas ukuran berkas display dalam bytes (MiB, basis 1024): JPEG 5 MiB, PNG ber-alpha 10 MiB.
pub fn display_limit_bytes(has_alpha: bool) -> u64 {
    if has_alpha {
        DISPLAY_MAX_BYTES_PNG_ALPHA
    } else {
        DISPLAY_MAX_BYTES_JPEG
    }
}

/// Anak tangga awal: dimensi tangga terbesar yang tidak melebihi sisi terpanjang sumber;
/// jika sumber lebih kecil dari lantai, kembalikan lantai (1.024 px).
pub fn png_initial_dimension(max_orig_dim: u32) -> u32 {
    PNG_DIMENSION_LADDER
        .iter()
        .copied()
        .find(|&d| d <= max_orig_dim)
        .unwrap_or(DISPLAY_MIN_DIMENSION_PX)
}

/// Anak tangga berikutnya setelah encode berukuran `size` pada dimensi `cur_dim`.
/// Estimasi kuadratik (ukuran ~ dimensi^2) dengan margin 5%, melompat ke anak tangga terdekat
/// yang diperkirakan muat; minimal turun satu anak tangga; tidak pernah di bawah lantai 1.024 px.
pub fn png_next_dimension(cur_dim: u32, size: u64, limit_bytes: u64) -> u32 {
    if cur_dim <= DISPLAY_MIN_DIMENSION_PX {
        return DISPLAY_MIN_DIMENSION_PX;
    }
    let ratio = (limit_bytes as f64 * 0.95 / size.max(1) as f64).sqrt();
    let est_dim = (cur_dim as f64 * ratio) as u32;
    PNG_DIMENSION_LADDER
        .iter()
        .copied()
        .filter(|&d| d < cur_dim)
        .find(|&d| d <= est_dim)
        .unwrap_or_else(|| {
            PNG_DIMENSION_LADDER
                .iter()
                .copied()
                .find(|&d| d < cur_dim)
                .unwrap_or(DISPLAY_MIN_DIMENSION_PX)
        })
}

/// Parameter encode PNG (kompresi dan filter).
#[derive(Debug, Clone, Copy)]
pub struct PngEncodeSettings {
    pub compression: png::Compression,
    pub filter: png::FilterType,
    pub adaptive: png::AdaptiveFilterType,
}

impl Default for PngEncodeSettings {
    /// Returns default PNG encoding settings (Default compression, Sub filter, Adaptive).
    fn default() -> Self {
        Self {
            compression: png::Compression::Default,
            filter: png::FilterType::Sub,
            adaptive: png::AdaptiveFilterType::Adaptive,
        }
    }
}

/// Jumlah baris per band saat menulis PNG; token pembatalan diperiksa di antara band.
pub const PNG_ENCODE_BAND_ROWS: usize = 64;

/// Encode RGBA8 ke PNG per band baris lewat `png::StreamWriter` sehingga pembatalan dapat
/// terjadi di antara band (bukan satu panggilan monolitik). Mengembalikan ukuran berkas (bytes).
pub fn encode_png_banded(
    image: &image::DynamicImage,
    path: &Path,
    settings: PngEncodeSettings,
    cancel_token: Option<&CancellationToken>,
) -> Result<u64, OrthoImportError> {
    let is_cancelled = || cancel_token.map(|t| t.is_cancelled()).unwrap_or(false);
    let owned;
    let rgba: &image::RgbaImage = match image.as_rgba8() {
        Some(r) => r,
        None => {
            owned = image.to_rgba8();
            &owned
        }
    };
    let (w, h) = rgba.dimensions();
    let file = File::create(path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuat berkas display.png: {}", e))
    })?;
    let writer = CancellableWriter::new(BufWriter::new(file), cancel_token);
    let mut encoder = png::Encoder::new(writer, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(settings.compression);
    encoder.set_filter(settings.filter);
    encoder.set_adaptive_filter(settings.adaptive);

    let map_err = |what: &str, e: String| -> OrthoImportError {
        if is_cancelled() {
            OrthoImportError::Dibatalkan
        } else {
            OrthoImportError::InternalError(format!("{}: {}", what, e))
        }
    };

    let mut png_writer = encoder
        .write_header()
        .map_err(|e| map_err("Gagal menulis header PNG", e.to_string()))?;
    {
        let mut stream = png_writer
            .stream_writer()
            .map_err(|e| map_err("Gagal membuka stream PNG", e.to_string()))?;
        let row_bytes = w as usize * 4;
        for band in rgba.as_raw().chunks(row_bytes * PNG_ENCODE_BAND_ROWS) {
            if is_cancelled() {
                return Err(OrthoImportError::Dibatalkan);
            }
            stream
                .write_all(band)
                .map_err(|e| map_err("Gagal menulis band PNG", e.to_string()))?;
        }
        stream
            .finish()
            .map_err(|e| map_err("Gagal menyelesaikan stream PNG", e.to_string()))?;
    }
    png_writer
        .finish()
        .map_err(|e| map_err("Gagal menyelesaikan PNG", e.to_string()))?;

    Ok(fs::metadata(path).map(|m| m.len()).unwrap_or(0))
}

/// Loop adaptif PNG ber-alpha: encode, lalu lompat kuadratik ke anak tangga berikutnya
/// sampai ukuran <= `limit_bytes` atau mencapai lantai 1.024 px. Selalu mengubah ukuran
/// dari kanvas asli di memori.
pub fn adaptive_encode_png(
    image: &image::DynamicImage,
    target_dir: &Path,
    cancel_token: Option<&CancellationToken>,
    limit_bytes: u64,
    settings: PngEncodeSettings,
) -> Result<AdaptiveEncodeResult, OrthoImportError> {
    let mut warnings = Vec::new();
    let display_png = target_dir.join("display.png");
    let (orig_w, orig_h) = (image.width(), image.height());
    let max_orig_dim = orig_w.max(orig_h);
    let limit_mib = limit_bytes as f64 / (1024 * 1024) as f64;
    let mut cur_dim = png_initial_dimension(max_orig_dim);

    loop {
        if let Some(t) = cancel_token {
            if t.is_cancelled() {
                return Err(OrthoImportError::Dibatalkan);
            }
        }

        let resized = if cur_dim >= max_orig_dim {
            image.clone()
        } else {
            resample_dynamic_image_box_filter(image, cur_dim)?
        };
        let (tw, th) = (resized.width(), resized.height());
        let size = encode_png_banded(&resized, &display_png, settings, cancel_token)?;

        if size <= limit_bytes || cur_dim <= DISPLAY_MIN_DIMENSION_PX {
            if tw < orig_w || th < orig_h {
                warnings.push(format!(
                    "Resolusi pratinjau PNG disesuaikan ke {}x{} agar muat dalam batas {} MiB",
                    tw, th, limit_mib
                ));
            }
            if tw.max(th) < 2048 {
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

        cur_dim = png_next_dimension(cur_dim, size, limit_bytes);
    }
}

/// Menghasilkan berkas display adaptif (display.jpg atau display.png) dengan kontrol ketat:
/// - JPEG: batas maksimal 5 MiB (`DISPLAY_MAX_BYTES_JPEG`).
/// - PNG (citra dengan transparansi alpha): batas maksimal 10 MiB (`DISPLAY_MAX_BYTES_PNG_ALPHA`).
///   Alasan: PNG lossless pada foto udara berentropi tinggi tidak dapat mencapai 2.048 px dalam 5 MiB,
///   sedangkan JPEG bisa.
/// - Tangga dimensi (sisi terpanjang): 4096, 3584, 3072, 2560, 2304, 2048, 1792, 1536, 1280, 1024.
/// - Estimasi kuadratik: lihat [`png_next_dimension`].
/// - Downscale selalu dilakukan langsung dari kanvas asli di memori (`image`).
pub fn adaptive_encode_display(
    image: &image::DynamicImage,
    target_dir: &Path,
    has_alpha: bool,
    cancel_token: Option<&CancellationToken>,
) -> Result<AdaptiveEncodeResult, OrthoImportError> {
    let mut warnings = Vec::new();

    if has_alpha {
        adaptive_encode_png(
            image,
            target_dir,
            cancel_token,
            display_limit_bytes(true),
            PngEncodeSettings::default(),
        )
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
            if let Some(t) = cancel_token {
                if t.is_cancelled() {
                    return Err(OrthoImportError::Dibatalkan);
                }
            }

            let resized = if dim >= orig_w.max(orig_h) {
                rgb_img.clone()
            } else {
                resample_rgb_image_box_filter(&rgb_img, dim)?
            };
            let (tw, th) = (resized.width(), resized.height());

            for &quality in &quality_candidates {
                if let Some(t) = cancel_token {
                    if t.is_cancelled() {
                        return Err(OrthoImportError::Dibatalkan);
                    }
                }

                let file = File::create(&display_jpg).map_err(|e| {
                    OrthoImportError::InternalError(format!(
                        "Gagal membuat berkas display.jpg: {}",
                        e
                    ))
                })?;
                let buf_writer = BufWriter::new(file);
                let mut writer = CancellableWriter::new(buf_writer, cancel_token);

                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
                encoder.encode_image(&resized).map_err(|e| {
                    if let Some(t) = cancel_token {
                        if t.is_cancelled() {
                            return OrthoImportError::Dibatalkan;
                        }
                    }
                    OrthoImportError::InternalError(format!("Gagal encode: {}", e))
                })?;
                writer.flush().map_err(|e| {
                    if let Some(t) = cancel_token {
                        if t.is_cancelled() {
                            return OrthoImportError::Dibatalkan;
                        }
                    }
                    OrthoImportError::InternalError(format!("Gagal flush: {}", e))
                })?;

                let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
                if size <= DISPLAY_MAX_BYTES_JPEG {
                    if tw < orig_w || th < orig_h {
                        warnings.push(format!(
                            "Resolusi display diadaptasi ke {}x{} (kualitas {})",
                            tw, th, quality
                        ));
                    }
                    if tw.max(th) < 2048 {
                        warnings.push(format!(
                            "Peringatan: Resolusi akhir pratinjau JPEG ({}x{}) berada di bawah 2.048 piksel",
                            tw, th
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

    // Periksa fast-fail dimensi dan batas 144 MP serta progressive JPEG > 40 MP
    let (orig_width, orig_height) = if ext == "jpg" {
        let (is_prog, w, h) = detect_jpeg_progressive_and_dimensions(&source_path)?;
        let total_pixels = w as u64 * h as u64;
        if is_prog && total_pixels > MAX_PROGRESSIVE_JPEG_PIXELS {
            return Err(OrthoImportError::MelebihiBatas {
                width: w,
                height: h,
                max_dimension: 6324,
                message: format!(
                    "JPEG progressive dengan resolusi {}x{} melebihi batas aman 40 MP.",
                    w, h
                ),
            });
        }
        if total_pixels > 144_000_000 || w > MAX_IMAGE_DIMENSION_PX || h > MAX_IMAGE_DIMENSION_PX {
            return Err(OrthoImportError::MelebihiBatas {
                width: w,
                height: h,
                max_dimension: MAX_IMAGE_DIMENSION_PX,
                message: format!("Dimensi JPEG {}x{} melebihi batas aman 144 MP.", w, h),
            });
        }
        (w, h)
    } else {
        let (w, h) = validate_image_dimensions(&source_path)
            .map_err(|e| OrthoImportError::FileKorup(e.to_string()))?;
        let total_pixels = w as u64 * h as u64;
        if total_pixels > 144_000_000 || w > MAX_IMAGE_DIMENSION_PX || h > MAX_IMAGE_DIMENSION_PX {
            return Err(OrthoImportError::MelebihiBatas {
                width: w,
                height: h,
                max_dimension: MAX_IMAGE_DIMENSION_PX,
                message: format!("Dimensi PNG {}x{} melebihi batas aman 144 MP.", w, h),
            });
        }
        (w, h)
    };

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
    let mut pipeline_warnings = Vec::new();
    if let Some(orient_warn) = super::exif_reader::check_exif_orientation_warning(&source_path) {
        pipeline_warnings.push(orient_warn);
    }

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

    // 4. TAHAP LOOP ADAPTIF DISPLAY (JPEG <= 5 MiB, PNG alpha <= 10 MiB)
    update_progress(
        "loop_adaptif",
        70.0,
        "Mengoptimalkan ukuran berkas display...",
    );
    let encode_res =
        adaptive_encode_display(&dynamic_image, &staging_dir, has_alpha, Some(&cancel_token))?;

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
        warnings: {
            let mut w = encode_res.warnings.clone();
            w.extend(pipeline_warnings);
            w
        },
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
        display_dimensions: Some(encode_res.final_dimensions),
        warnings: Some(encode_res.warnings.clone()),
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

/// Orkestrasi eksekusi impor Opsi B (GeoTIFF -> Konversi) secara asinkron.
pub fn execute_import_option_b(
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

    let t_total_start = Instant::now();

    // 1. TAHAP VALIDASI FAST-FAIL SEBELUM STAGING
    let t_header_start = Instant::now();
    update_progress(
        "validasi",
        5.0,
        "Memvalidasi header berkas GeoTIFF dan batas dimensi...",
    );
    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    if !source_path.is_file() {
        return Err(OrthoImportError::FileKorup(format!(
            "Berkas GeoTIFF tidak ditemukan di disk: {}",
            source_path.display()
        )));
    }

    let source_size_bytes = fs::metadata(&source_path)
        .map(|m| m.len())
        .map_err(|e| OrthoImportError::FileKorup(e.to_string()))?;

    let header_summary = super::geotiff_reader::validate_geotiff_header_fast(&source_path)?;
    let (orig_width, orig_height) = (header_summary.width, header_summary.height);

    // Buka berkas untuk membaca georeferensi dan detail teknis
    let file = File::open(&source_path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuka berkas GeoTIFF: {}", e))
    })?;
    let mut decoder = tiff::decoder::Decoder::new(BufReader::new(file))?;

    let georef_res =
        super::geotiff_reader::extract_geotiff_georeference(&mut decoder, orig_width, orig_height);

    let (bounds, bounds_source, detected_crs_name) = match georef_res {
        Ok(ref georef) => (
            georef.bounds,
            "geotiff".to_string(),
            georef.crs_string.clone(),
        ),
        Err(e) => {
            if let Some(ref manual) = payload.manual_bounds {
                let manual_b =
                    validate_wgs84_bounds(manual.west, manual.south, manual.east, manual.north)
                        .map_err(|err| OrthoImportError::KoordinatTidakValid(err.to_string()))?;
                (manual_b, "manual".to_string(), "Manual (WGS84)".to_string())
            } else {
                return Err(e);
            }
        }
    };

    let tiff_details = super::geotiff_reader::extract_tiff_technical_details(
        &mut decoder,
        orig_width,
        orig_height,
    )?;

    let mut pipeline_warnings = Vec::new();

    // Siapkan Guard Staging RAII yang aman
    let mut staging_guard = StagingGuard::for_job(job_id)?;
    let staging_dir = staging_guard.staging_dir().to_path_buf();
    fs::create_dir_all(&staging_dir).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuat folder staging: {}", e))
    })?;

    // Preflight cek ruang hard disk
    check_disk_space_preflight(&staging_dir, source_size_bytes, payload.save_original)?;
    let t_header_ms = t_header_start.elapsed().as_millis();
    println!("STAGE_TIMING|stage=header_georef|ms={}", t_header_ms);

    // 2. TAHAP SALIN ORIGINAL STREAMING (JIKA OPSI "Simpan Berkas Asli" DIAKTIFKAN)
    let original_staging_file = staging_dir.join("original.tif");
    if payload.save_original {
        update_progress("salin_asli", 15.0, "Menyalin berkas GeoTIFF asli...");
        stream_copy_with_cancellation(
            &source_path,
            &original_staging_file,
            &cancel_token,
            |pct| {
                let overall = 15.0 + (pct * 0.20); // 15% sampai 35%
                update_progress("salin_asli", overall, "Menyalin berkas GeoTIFF asli...");
            },
        )?;
    }

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    // 3. TAHAP DEKODE & STREAMING RESAMPLING
    update_progress(
        "dekompresi",
        40.0,
        "Mendekompresi tile GeoTIFF dan meresampling...",
    );

    // Buka ulang decoder untuk streaming (posisi cursor fresh)
    let file_stream = File::open(&source_path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuka berkas untuk streaming: {}", e))
    })?;
    let mut stream_decoder = tiff::decoder::Decoder::new(BufReader::new(file_stream))?;

    // Analisis peregangan 16-bit jika bit_depth == 16
    let t_sample_start = Instant::now();
    let u16_stretch = if header_summary.bit_depth == 16 {
        let (lo, hi, is_full_range) = super::geotiff_reader::compute_16bit_percentiles(
            &mut stream_decoder,
            orig_width,
            orig_height,
            header_summary.has_alpha,
            header_summary.nodata_value,
        )?;
        if is_full_range {
            pipeline_warnings.push(
                "Citra 16-bit terdeteksi full-range, peregangan kontras dilewati (linear mapping)."
                    .to_string(),
            );
            Some((0.0, 65535.0))
        } else {
            pipeline_warnings.push(format!(
                "Peregangan kontras 16-bit diterapkan: p0,5 = {:.1}, p99,5 = {:.1}",
                lo, hi
            ));
            Some((lo, hi))
        }
    } else {
        None
    };
    let t_sample_ms = t_sample_start.elapsed().as_millis();
    println!("STAGE_TIMING|stage=sampling_16bit|ms={}", t_sample_ms);

    let has_alpha_or_nodata = header_summary.has_alpha || header_summary.nodata_value.is_some();
    if header_summary.nodata_value.is_some() && !header_summary.has_alpha {
        pipeline_warnings.push(
            "Batas hitam / NoData pada GeoTIFF dikonversi menjadi transparansi (kanal alpha)."
                .to_string(),
        );
    }
    let channel_type = if has_alpha_or_nodata {
        ChannelCount::Rgba
    } else {
        ChannelCount::Rgb
    };

    let mut resampler = StreamingResampler::new(
        orig_width,
        orig_height,
        DISPLAY_TARGET_DIMENSION_PX,
        channel_type,
    )?;

    let t_stream_start = Instant::now();
    // Periksa apakah overview sub-IFD piramida dapat dimanfaatkan
    let best_overview_ifd = super::geotiff_reader::find_best_overview_ifd(&mut stream_decoder)?;
    if let Some(ifd_idx) = best_overview_ifd {
        if stream_decoder.seek_to_image(ifd_idx).is_ok() {
            if let Ok((ow, oh)) = stream_decoder.dimensions() {
                if ow >= DISPLAY_TARGET_DIMENSION_PX || oh >= DISPLAY_TARGET_DIMENSION_PX {
                    resampler =
                        StreamingResampler::new(ow, oh, DISPLAY_TARGET_DIMENSION_PX, channel_type)?;
                    pipeline_warnings.push(format!(
                        "Piramida internal IFD {} ({}x{}) digunakan untuk akselerasi dekompresi display.",
                        ifd_idx, ow, oh
                    ));
                    super::geotiff_reader::stream_geotiff_to_resampler(
                        &mut stream_decoder,
                        ow,
                        oh,
                        &mut resampler,
                        u16_stretch,
                        header_summary.nodata_value,
                        header_summary.is_min_is_white,
                        Some(&cancel_token),
                    )?;
                } else {
                    let _ = stream_decoder.seek_to_image(0);
                    super::geotiff_reader::stream_geotiff_to_resampler(
                        &mut stream_decoder,
                        orig_width,
                        orig_height,
                        &mut resampler,
                        u16_stretch,
                        header_summary.nodata_value,
                        header_summary.is_min_is_white,
                        Some(&cancel_token),
                    )?;
                }
            }
        }
    } else {
        super::geotiff_reader::stream_geotiff_to_resampler(
            &mut stream_decoder,
            orig_width,
            orig_height,
            &mut resampler,
            u16_stretch,
            header_summary.nodata_value,
            header_summary.is_min_is_white,
            Some(&cancel_token),
        )?;
    }

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    let resampled = resampler.finish()?;
    let t_stream_ms = t_stream_start.elapsed().as_millis();
    println!("STAGE_TIMING|stage=decode_resample|ms={}", t_stream_ms);

    let dynamic_image = if has_alpha_or_nodata {
        let rgba = image::RgbaImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError(
                    "Buffer RGBA hasil resampling tidak sesuai".to_string(),
                )
            })?;
        image::DynamicImage::ImageRgba8(rgba)
    } else {
        let rgb = image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError(
                    "Buffer RGB hasil resampling tidak sesuai".to_string(),
                )
            })?;
        image::DynamicImage::ImageRgb8(rgb)
    };

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    // 4. TAHAP LOOP ADAPTIF DISPLAY (JPEG <= 5 MiB, PNG alpha <= 10 MiB)
    update_progress(
        "loop_adaptif",
        70.0,
        "Mengoptimalkan ukuran berkas display GeoTIFF...",
    );
    let t_encode_start = Instant::now();
    let encode_res = adaptive_encode_display(
        &dynamic_image,
        &staging_dir,
        has_alpha_or_nodata,
        Some(&cancel_token),
    )?;
    let t_encode_ms = t_encode_start.elapsed().as_millis();
    println!("STAGE_TIMING|stage=encode_display|ms={}", t_encode_ms);

    if cancel_token.is_cancelled() {
        return Err(OrthoImportError::Dibatalkan);
    }

    // 5. TAHAP POINT OF NO RETURN: RENAME FOLDER & REGISTRASI ATOMIK
    let t_write_start = Instant::now();
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
        import_mode: "B".to_string(),
        source_format: "geotiff".to_string(),
        source_filename: payload.custom_filename.clone().unwrap_or_else(|| {
            source_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("geotiff_image")
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
        source_crs: detected_crs_name,
        exif_summary: None,
        tiff_technical_details: Some(tiff_details),
        imported_at: chrono::Utc::now().to_rfc3339(),
        warnings: {
            let mut w = encode_res.warnings.clone();
            w.extend(pipeline_warnings);
            w
        },
    };

    let tech_json_path = staging_dir.join("metadata.json");
    let tech_json_str = serde_json::to_string_pretty(&tech_meta).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal serialisasi metadata.json: {}", e))
    })?;
    write_atomic(&tech_json_path, &tech_json_str).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal menulis metadata.json: {}", e))
    })?;

    let t_write_ms = t_write_start.elapsed().as_millis();
    let t_total_ms = t_total_start.elapsed().as_millis();
    println!("STAGE_TIMING|stage=write_register|ms={}", t_write_ms);
    println!("STAGE_TIMING|stage=total|ms={}", t_total_ms);

    // Tulis StoredImageMetadata (meta.json) ke staging sebelum rename
    let final_size_bytes = if payload.save_original {
        source_size_bytes
    } else {
        encode_res.display_size_bytes
    };
    let final_ext = if payload.save_original {
        "tif".to_string()
    } else {
        encode_res.format.clone()
    };

    let display_rel_url = format!(
        "/map_storage/images/{}/display.{}",
        image_id, encode_res.format
    );
    let original_rel_url = if payload.save_original {
        format!("/map_storage/images/{}/original.tif", image_id)
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
        display_dimensions: Some(encode_res.final_dimensions),
        warnings: Some(tech_meta.warnings.clone()),
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
        message: "Impor GeoTIFF berhasil diselesaikan".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        error: None,
        result_image_id: Some(image_id),
    };
    let _ = save_job_record(&final_record);

    Ok(stored_meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_03::storage_utils::get_storage_root;

    #[test]
    fn test_import_option_b_with_black_border_transparency() {
        let tiff_path =
            std::path::Path::new(r"C:\Users\x0776\Downloads\600fcc9055481e0005e312a1.tif");
        if !tiff_path.exists() {
            return;
        }

        let job_id = format!("job_test_trans_{}", chrono::Utc::now().timestamp_millis());
        let payload = ImportDualModePayload {
            mode: "B".to_string(),
            file_path: tiff_path.to_string_lossy().to_string(),
            save_original: false,
            manual_bounds: None,
            custom_filename: Some("Test Transparent GeoTIFF".to_string()),
        };

        let result = execute_import_option_b(&job_id, payload, CancellationToken::new())
            .expect("Eksekusi Opsi B dengan transparansi harus berhasil");

        assert_eq!(
            result.extension, "png",
            "Format display harus PNG dengan transparansi"
        );
        assert!(
            result.display_size_bytes.unwrap() <= 10 * 1024 * 1024,
            "Ukuran PNG harus <= 10 MiB"
        );

        let image_dir = get_images_dir().join(&result.id);
        let display_png = image_dir.join("display.png");
        assert!(display_png.exists(), "Berkas display.png harus ada");

        let img = image::open(&display_png)
            .expect("Gagal membuka display.png")
            .to_rgba8();
        let corner_px = img.get_pixel(0, 0);
        assert_eq!(
            corner_px[3], 0,
            "Piksel sudut (0,0) harus memiliki Alpha = 0 (transparan)"
        );

        // Salin display.png ke ortho_1792982125519 agar peta pengguna langsung terupdate
        let user_ortho_dir = get_images_dir().join("ortho_1792982125519");
        if user_ortho_dir.exists() {
            let user_display_png = user_ortho_dir.join("display.png");
            let _ = fs::copy(&display_png, &user_display_png);

            let user_meta_path = user_ortho_dir.join("meta.json");
            if let Ok(meta_content) = fs::read_to_string(&user_meta_path) {
                if let Ok(mut meta_json) = serde_json::from_str::<serde_json::Value>(&meta_content)
                {
                    meta_json["extension"] = serde_json::Value::String("png".to_string());
                    meta_json["file_path"] = serde_json::Value::String(
                        "/map_storage/images/ortho_1792982125519/display.png".to_string(),
                    );
                    meta_json["thumbnail_url"] = serde_json::Value::String(
                        "/map_storage/images/ortho_1792982125519/display.png".to_string(),
                    );
                    if let Some(size) = result.display_size_bytes {
                        meta_json["display_size_bytes"] = serde_json::Value::Number(size.into());
                    }
                    if let Some(dims) = result.display_dimensions {
                        meta_json["display_dimensions"] = serde_json::json!(dims);
                    }
                    let _ = fs::write(
                        &user_meta_path,
                        serde_json::to_string_pretty(&meta_json).unwrap(),
                    );
                }
            }

            let user_metadata_path = user_ortho_dir.join("metadata.json");
            if let Ok(metadata_content) = fs::read_to_string(&user_metadata_path) {
                if let Ok(mut metadata_json) =
                    serde_json::from_str::<serde_json::Value>(&metadata_content)
                {
                    metadata_json["display_format"] = serde_json::Value::String("png".to_string());
                    if let Some(size) = result.display_size_bytes {
                        metadata_json["display_file_size_bytes"] =
                            serde_json::Value::Number(size.into());
                    }
                    if let Some(dims) = result.display_dimensions {
                        metadata_json["display_dimensions"] = serde_json::json!(dims);
                    }
                    let _ = fs::write(
                        &user_metadata_path,
                        serde_json::to_string_pretty(&metadata_json).unwrap(),
                    );
                }
            }
        }

        let _ = fs::remove_dir_all(&image_dir);
        let _ = fs::remove_file(
            get_storage_root()
                .join("layers")
                .join(format!("{}.json", result.id)),
        );
        let _ = fs::remove_file(
            get_storage_root()
                .join(".jobs")
                .join(format!("{}.json", job_id)),
        );
    }
}
