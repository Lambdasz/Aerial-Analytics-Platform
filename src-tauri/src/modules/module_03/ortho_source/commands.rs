//! Tauri IPC commands untuk pengelolaan citra ortomosaik dan raster JPG/PNG (Module 3).

use super::types::{
    DeleteOrthoSummary, ImportImagePayload, StoredImageMetadata, UpdateOrthoImagePayload,
};
use crate::modules::module_03::state::get_global_state;
use crate::plugin_manager::error::CommandError;

/// Mengambil daftar seluruh citra orthomosaic yang tersimpan di storage Modul 3.
/// Termasuk citra dengan metadata rusak (is_corrupted = true).
#[tauri::command]
pub fn list_ortho_images() -> Result<Vec<StoredImageMetadata>, CommandError> {
    Ok(get_global_state().list_ortho_images()?)
}

/// Mengambil metadata satu citra orthomosaic berdasarkan ID.
#[tauri::command]
pub fn get_ortho_image(id: String) -> Result<StoredImageMetadata, CommandError> {
    Ok(get_global_state().get_ortho_image(&id)?)
}

/// Memperbarui nama atau koordinat bounds citra orthomosaic.
#[tauri::command]
pub fn update_ortho_image(
    payload: UpdateOrthoImagePayload,
) -> Result<StoredImageMetadata, CommandError> {
    super::instance_lock::require_primary_instance()?;
    Ok(get_global_state().update_ortho_image(payload)?)
}

/// Menghitung data dependensi cascade (layer dan anotasi terikat) sebelum penghapusan.
#[tauri::command]
pub fn get_ortho_cascade_summary(id: String) -> Result<DeleteOrthoSummary, CommandError> {
    Ok(get_global_state().get_ortho_cascade_summary(&id)?)
}

/// Menghapus satu citra orthomosaic secara tuntas beserta layer dan anotasi terikat.
#[tauri::command]
pub fn delete_ortho_image(id: String) -> Result<DeleteOrthoSummary, CommandError> {
    super::instance_lock::require_primary_instance()?;
    Ok(get_global_state().delete_ortho_image(&id)?)
}

/// Status instansi aplikasi untuk visualisasi banner UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstanceStatus {
    pub is_primary: bool,
    pub is_read_only: bool,
}

/// Memeriksa status instansi aplikasi (primer / sekunder baca-saja).
#[tauri::command]
pub fn get_instance_status() -> Result<InstanceStatus, CommandError> {
    let is_primary = super::instance_lock::is_primary_instance();
    Ok(InstanceStatus {
        is_primary,
        is_read_only: !is_primary,
    })
}

/// Mengimpor berkas citra orthomosaic JPG/PNG secara sinkron (DINONAKTIFKAN).
/// Jalur ini dinonaktifkan demi keamanan staging, cancel token, dan single job guard.
#[tauri::command]
pub fn import_ortho_image(
    _payload: ImportImagePayload,
) -> Result<StoredImageMetadata, CommandError> {
    super::instance_lock::require_primary_instance()?;
    Err(CommandError {
        code: "LEGACY_ENDPOINT_DISABLED".to_string(),
        message: "Endpoint import_ortho_image lama telah dinonaktifkan. Gunakan start_import_ortho_job untuk proses asinkron yang aman dengan staging dan pembatalan.".to_string(),
    })
}

/// Menyimpan file citra ke storage dari path file yang sudah ada di sistem (DINONAKTIFKAN).
#[tauri::command]
pub fn save_image_from_path(_source_path: String) -> Result<StoredImageMetadata, CommandError> {
    super::instance_lock::require_primary_instance()?;
    Err(CommandError {
        code: "LEGACY_ENDPOINT_DISABLED".to_string(),
        message: "Endpoint save_image_from_path lama telah dinonaktifkan. Gunakan start_import_ortho_job untuk proses asinkron yang aman.".to_string(),
    })
}

/// Mengambil daftar seluruh citra yang tersimpan di storage Modul 3 (legacy).
#[tauri::command]
pub fn get_saved_images() -> Result<Vec<StoredImageMetadata>, CommandError> {
    Ok(get_global_state().get_saved_images())
}

/// Menghapus citra dari storage berdasarkan kategori dan nama file / image_id (legacy).
#[tauri::command]
pub fn delete_saved_image(category: String, filename: String) -> Result<bool, CommandError> {
    super::instance_lock::require_primary_instance()?;
    Ok(get_global_state().delete_saved_image(&category, &filename)?)
}

/// Memulai job impor citra orthomosaic secara asinkron (Dual-mode: Opsi A atau Opsi B).
/// Mengembalikan `job_id` unik untuk memantau status atau membatalkan proses.
#[tauri::command]
pub fn start_import_ortho_job(
    payload: super::types::ImportDualModePayload,
) -> Result<String, CommandError> {
    super::instance_lock::require_primary_instance()?;
    let job_id = format!("job_{}", chrono::Utc::now().timestamp_millis());
    let token = super::job_manager::try_acquire_job(&job_id)?;

    // Catat status awal di disk (.jobs/{job_id}.json)
    let initial_record = super::types::JobRecord {
        job_id: job_id.clone(),
        status: "running".to_string(),
        stage: "validasi".to_string(),
        progress_percent: 0.0,
        message: "Memulai proses impor citra...".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        error: None,
        result_image_id: None,
    };
    super::job_manager::save_job_record(&initial_record)?;

    let worker_job_id = job_id.clone();
    let worker_payload = payload.clone();

    // Spawn background worker thread
    std::thread::Builder::new()
        .name(format!("ortho-worker-{}", worker_job_id))
        .spawn(move || {
            let res = match worker_payload.mode.as_str() {
                "A" | "a" => super::image_pipeline::execute_import_option_a(
                    &worker_job_id,
                    worker_payload,
                    token,
                ),
                "B" | "b" => super::image_pipeline::execute_import_option_b(
                    &worker_job_id,
                    worker_payload,
                    token,
                ),
                other => Err(
                    crate::modules::module_03::error::OrthoImportError::FormatTidakDidukung(
                        format!(
                            "Mode impor '{}' tidak valid. Pilih Opsi 'A' atau 'B'.",
                            other
                        ),
                    ),
                ),
            };

            if let Err(e) = res {
                // Jika terjadi kesalahan dan job belum berstatus cancelled/interrupted, simpan status gagal
                if let Ok(Some(mut rec)) = super::job_manager::load_job_record(&worker_job_id) {
                    if rec.status == "running" {
                        if e == crate::modules::module_03::error::OrthoImportError::Dibatalkan {
                            rec.status = "cancelled".to_string();
                            rec.message =
                                "Proses impor telah dibatalkan oleh pengguna.".to_string();
                        } else {
                            rec.status = "failed".to_string();
                            rec.error = Some(e.code().to_string());
                            rec.message = e.user_friendly_message();
                        }
                        rec.updated_at = chrono::Utc::now().to_rfc3339();
                        let _ = super::job_manager::save_job_record(&rec);
                    }
                }
            }
        })
        .map_err(|e| CommandError {
            code: "THREAD_ERROR".to_string(),
            message: format!("Gagal membuat thread pekerja: {}", e),
        })?;

    Ok(job_id)
}

/// Membatalkan proses impor citra yang sedang berlangsung.
#[tauri::command]
pub fn cancel_import_job(job_id: String) -> Result<(), CommandError> {
    if let Some(token) = super::job_manager::get_active_job_token(&job_id) {
        token.cancel();
    }

    if let Ok(Some(mut rec)) = super::job_manager::load_job_record(&job_id) {
        if rec.status == "running" {
            rec.status = "cancelled".to_string();
            rec.message = "Proses dibatalkan oleh pengguna".to_string();
            rec.updated_at = chrono::Utc::now().to_rfc3339();
            let _ = super::job_manager::save_job_record(&rec);
        }
    }

    super::job_manager::release_job(&job_id);

    Ok(())
}

/// Mengambil status terkini dan persentase progres pekerjaan impor dari disk.
#[tauri::command]
pub fn get_import_job_status(job_id: String) -> Result<super::types::JobRecord, CommandError> {
    match super::job_manager::load_job_record(&job_id)? {
        Some(record) => Ok(record),
        None => Err(CommandError {
            code: "NOT_FOUND".to_string(),
            message: format!("Pekerjaan impor dengan ID '{}' tidak ditemukan.", job_id),
        }),
    }
}

/// Hasil inspeksi awal berkas GeoTIFF untuk tampilan UI Tab B sebelum impor dimulai.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeotiffInspectionResult {
    pub file_path: String,
    pub filename: String,
    pub file_size_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    pub has_alpha: bool,
    pub compression: String,
    pub has_georeference: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_code: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_name: Option<String>,
    pub is_crs_supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_bounds: Option<super::types::ImportBoundsPayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Melakukan validasi header GeoTIFF dan mengekstrak proyeksi/bounding box sebelum job dimulai.
#[tauri::command]
pub fn inspect_geotiff_header(file_path: String) -> Result<GeotiffInspectionResult, CommandError> {
    let path = std::path::PathBuf::from(&file_path);
    if !path.is_file() {
        return Err(CommandError {
            code: "NOT_FOUND".to_string(),
            message: format!("Berkas GeoTIFF tidak ditemukan di disk: {}", file_path),
        });
    }

    let summary = super::geotiff_reader::validate_geotiff_header_fast(&path)?;
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("geotiff.tif")
        .to_string();

    let file_size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    let file = std::fs::File::open(&path).map_err(|e| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("Gagal membuka berkas GeoTIFF: {}", e),
    })?;
    let mut decoder =
        tiff::decoder::Decoder::new(std::io::BufReader::new(file)).map_err(|e| CommandError {
            code: "TIFF_DECODE_ERROR".to_string(),
            message: format!("Gagal membaca struktur berkas GeoTIFF: {}", e),
        })?;

    let georef_res = super::geotiff_reader::extract_geotiff_georeference(
        &mut decoder,
        summary.width,
        summary.height,
    );

    match georef_res {
        Ok(georef) => Ok(GeotiffInspectionResult {
            file_path,
            filename,
            file_size_bytes,
            width: summary.width,
            height: summary.height,
            bit_depth: summary.bit_depth,
            has_alpha: summary.has_alpha,
            compression: summary.compression_name,
            has_georeference: true,
            crs_code: Some(georef.crs_code),
            crs_name: Some(georef.crs_string),
            is_crs_supported: true,
            detected_bounds: Some(super::types::ImportBoundsPayload {
                south: georef.bounds[0][0],
                west: georef.bounds[0][1],
                north: georef.bounds[1][0],
                east: georef.bounds[1][1],
            }),
            error_code: None,
            error_message: None,
        }),
        Err(e) => {
            let is_crs_error = matches!(
                e,
                crate::modules::module_03::error::OrthoImportError::CrsTidakDidukung(_)
            );
            Ok(GeotiffInspectionResult {
                file_path,
                filename,
                file_size_bytes,
                width: summary.width,
                height: summary.height,
                bit_depth: summary.bit_depth,
                has_alpha: summary.has_alpha,
                compression: summary.compression_name,
                has_georeference: !is_crs_error,
                crs_code: None,
                crs_name: None,
                is_crs_supported: false,
                detected_bounds: None,
                error_code: Some(e.code().to_string()),
                error_message: Some(e.user_friendly_message()),
            })
        }
    }
}
