//! Berkas ini berisi orkestrator pipeline impor citra Opsi B (citra GeoTIFF dengan pembacaan georeferensi internal).
//!
//! Alur Eksekusi Pipeline Opsi B:
//! 1. Validasi fast-fail header berkas GeoTIFF (Magic bytes TIFF, IFD).
//! 2. Ekstraksi georeferensi internal (ModelTiepoint, ModelPixelScale, CRS metadata).
//! 3. Salinan berkas asli secara streaming ber-buffer dengan dukungan pembatalan (`CancellationToken`).
//! 4. Dekompresi streaming per-strip / per-tile dengan peregangan 16-bit percentile dan akselerasi overview IFD.
//! 5. Kompresi adaptif display ke berkas display ringan.
//! 6. Staging RAII & registrasi layer atomik ke registri layer.
//!
//! Daftar Fungsi:
//! - `execute_import_option_b`: Fungsi orkestrasi utama impor citra Opsi B (GeoTIFF) secara asinkron.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::ortho_source::constants::DISPLAY_TARGET_DIMENSION_PX;
use crate::modules::module_03::ortho_source::geotiff_reader;
use crate::modules::module_03::ortho_source::job_manager::{
    save_job_record, CancellationToken, JobRegistrationGuard, StagingGuard,
};
use crate::modules::module_03::ortho_source::pipeline::adaptive_encoder_io::adaptive_encode_display;
use crate::modules::module_03::ortho_source::pipeline::pipeline_validation_logic::{
    check_disk_space_preflight, stream_copy_with_cancellation, validate_wgs84_bounds,
};
use crate::modules::module_03::ortho_source::store::{get_images_dir, register_ortho_layer};
use crate::modules::module_03::ortho_source::streaming_resampler::{
    ChannelCount, StreamingResampler,
};
use crate::modules::module_03::ortho_source::types::{
    BoundsMetadata, ImageCategory, ImportDualModePayload, JobRecord, StoredImageMetadata,
    TechnicalMetadata,
};
use crate::modules::module_03::storage_utils::write_atomic;

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

    let header_summary = geotiff_reader::validate_geotiff_header_fast(&source_path)?;
    let (orig_width, orig_height) = (header_summary.width, header_summary.height);

    // Buka berkas untuk membaca georeferensi dan detail teknis
    let file = File::open(&source_path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuka berkas GeoTIFF: {}", e))
    })?;
    let mut decoder = tiff::decoder::Decoder::new(BufReader::new(file))?;

    let georef_res =
        geotiff_reader::extract_geotiff_georeference(&mut decoder, orig_width, orig_height);

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

    let tiff_details =
        geotiff_reader::extract_tiff_technical_details(&mut decoder, orig_width, orig_height)?;

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
        let (lo, hi, is_full_range) = geotiff_reader::compute_16bit_percentiles(
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
    let best_overview_ifd = geotiff_reader::find_best_overview_ifd(&mut stream_decoder)?;
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
                    geotiff_reader::stream_geotiff_to_resampler(
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
                    geotiff_reader::stream_geotiff_to_resampler(
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
        geotiff_reader::stream_geotiff_to_resampler(
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
