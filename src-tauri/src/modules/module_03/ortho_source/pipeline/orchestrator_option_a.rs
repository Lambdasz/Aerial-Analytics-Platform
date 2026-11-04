//! Berkas ini berisi orkestrator pipeline impor citra Opsi A (citra JPG/PNG dengan georeferensi eksternal/manual).
//!
//! Alur Eksekusi Pipeline Opsi A:
//! 1. Validasi fast-fail (ukuran berkas, signature magic bytes, dimensi piksel, CRS bounds).
//! 2. Streaming copy berkas asli ke folder penyimpanan dengan dukungan pembatalan (`CancellationToken`).
//! 3. Ekstraksi koordinat georeferensi dari World File (.jgw/.pgw) atau Sidecar JSON.
//! 4. Dekompresi dan downscaling gambar secara hemat memori (IDCT-scaling untuk JPEG / row-by-row PNG).
//! 5. Kompresi adaptif display (menghasilkan berkas display turunan yang optimal).
//! 6. Staging RAII & penulisan atomik metadata citra ke registri layer.
//!
//! Daftar Fungsi:
//! - `execute_import_option_a`: Fungsi orkestrasi utama impor citra Opsi A dari validasi hingga registrasi layer.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::ortho_source::constants::{
    DISPLAY_TARGET_DIMENSION_PX, MAX_IMAGE_DIMENSION_PX, MAX_IMAGE_FILE_SIZE_BYTES,
    MAX_PROGRESSIVE_JPEG_PIXELS,
};
use crate::modules::module_03::ortho_source::job_manager::{
    save_job_record, CancellationToken, JobRegistrationGuard, StagingGuard,
};
use crate::modules::module_03::ortho_source::pipeline::adaptive_encoder_io::adaptive_encode_display;
use crate::modules::module_03::ortho_source::pipeline::image_resampling_logic::{
    decode_and_scale_jpeg_idct, decode_and_scale_png, detect_jpeg_progressive_and_dimensions,
};
use crate::modules::module_03::ortho_source::pipeline::pipeline_validation_logic::{
    check_disk_space_preflight, stream_copy_with_cancellation, validate_image_dimensions,
    validate_magic_bytes, validate_wgs84_bounds,
};
use crate::modules::module_03::ortho_source::store::{get_images_dir, register_ortho_layer};
use crate::modules::module_03::ortho_source::types::{
    BoundsMetadata, ImageCategory, ImportDualModePayload, JobRecord, StoredImageMetadata,
    TechnicalMetadata,
};
use crate::modules::module_03::ortho_source::{exif_reader, sidecar_reader, world_file_reader};
use crate::modules::module_03::storage_utils::write_atomic;

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
        world_file_reader::find_candidate_world_file(&source_path)
    {
        let prj = world_file_reader::find_candidate_prj_file(&source_path);
        let wf_res = world_file_reader::read_world_file_envelope(
            &world_file,
            prj.as_deref(),
            orig_width,
            orig_height,
        )?;
        bounds_source = "world_file".to_string();
        detected_crs_code = wf_res.detected_crs;
        _prj_path_found = wf_res.prj_file_path;
        wf_res.bounds
    } else if let Some(sidecar_file) = sidecar_reader::find_candidate_sidecar_file(&source_path) {
        let s_bounds = sidecar_reader::read_sidecar_envelope(&sidecar_file)?;
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
    let exif_meta = exif_reader::read_exif_metadata(&source_path).unwrap_or(None);
    let mut pipeline_warnings = Vec::new();
    if let Some(orient_warn) = exif_reader::check_exif_orientation_warning(&source_path) {
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
