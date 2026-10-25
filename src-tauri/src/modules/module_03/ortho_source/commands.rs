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

    // Spawn async background task
    tokio::spawn(async move {
        let res = match worker_payload.mode.as_str() {
            "A" | "a" => super::image_pipeline::execute_import_option_a(
                &worker_job_id,
                worker_payload,
                token,
            ),
            "B" | "b" => {
                // Placeholder Fase 3 untuk GeoTIFF
                Err(
                    crate::modules::module_03::error::OrthoImportError::InternalError(
                        "Opsi B (GeoTIFF) akan tersedia pada Fase 3.".to_string(),
                    ),
                )
            }
            other => Err(
                crate::modules::module_03::error::OrthoImportError::FormatTidakDidukung(format!(
                    "Mode impor '{}' tidak valid. Pilih Opsi 'A' atau 'B'.",
                    other
                )),
            ),
        };

        if let Err(e) = res {
            // Jika terjadi kesalahan dan job belum berstatus cancelled/interrupted, simpan status gagal
            if let Ok(Some(mut rec)) = super::job_manager::load_job_record(&worker_job_id) {
                if rec.status == "running" {
                    if e == crate::modules::module_03::error::OrthoImportError::Dibatalkan {
                        rec.status = "cancelled".to_string();
                        rec.message = "Proses impor telah dibatalkan oleh pengguna.".to_string();
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
    });

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
