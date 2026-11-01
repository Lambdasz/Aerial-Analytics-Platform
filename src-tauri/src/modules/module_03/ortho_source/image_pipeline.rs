//! Facade dan fungsi utilitas impor citra orthomosaic JPG/PNG.
//!
//! Logika utama telah dipecah ke dalam modul-modul modular di sub-modul [`pipeline`]:
//! - [`pipeline::validation`]: Fast-fail validation & space preflight.
//! - [`pipeline::resampling`]: JPEG progressive check, IDCT scaling, streaming decode & resampling.
//! - [`pipeline::encoding`]: Adaptive display encoding & cancellable writer.
//! - [`pipeline::orchestrator_a`]: Eksekusi impor mode Opsi A.
//! - [`pipeline::orchestrator_b`]: Eksekusi impor mode Opsi B (GeoTIFF).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use super::pipeline::*;

use super::constants::{MAX_DISPLAY_FILE_SIZE_BYTES, PREVIEW_TARGET_DIMENSION_PX};
use super::store::{get_images_dir, register_ortho_layer};
use super::types::{ImageCategory, ImportImagePayload, StoredImageMetadata};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::write_atomic;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_03::ortho_source::job_manager::CancellationToken;
    use crate::modules::module_03::ortho_source::types::ImportDualModePayload;
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
