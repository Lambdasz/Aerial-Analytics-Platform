//! Submodul validasi berkas citra, preflight ruang disk, dan streaming copy original.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use super::super::constants::{MAX_IMAGE_DIMENSION_PX, MAX_IMAGE_FILE_SIZE_BYTES};
use super::super::job_manager::CancellationToken;
use crate::modules::module_03::error::{MapControllerError, OrthoImportError};

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
