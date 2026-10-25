//! Manajemen siklus hidup job impor asinkron, registri atomik, pembatalan,
//! guard direktori staging, dan pembersihan crash startup.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::constants::MAX_JOB_HISTORY_RETENTION;
use super::types::JobRecord;
use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::storage_utils::{get_storage_root, write_atomic};

/// Token pembatalan thread-safe untuk alur kerja asinkron.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn check_cancelled(&self) -> Result<(), OrthoImportError> {
        if self.is_cancelled() {
            Err(OrthoImportError::Dibatalkan)
        } else {
            Ok(())
        }
    }
}

/// Guard RAII untuk folder staging.
/// Jika keluar dari scope sebelum `disarm()` dipanggil (mis. karena panic atau error),
/// folder staging otomatis dihapus tanpa meninggalkan sampah di disk.
pub struct StagingGuard {
    staging_dir: PathBuf,
    disarmed: bool,
}

impl StagingGuard {
    pub fn new(staging_dir: PathBuf) -> Self {
        Self {
            staging_dir,
            disarmed: false,
        }
    }

    /// Membuat StagingGuard dengan path folder yang diturunkan secara aman dari job_id
    /// dan dijamin berada tepat di bawah `map_storage/images/.staging_{job_id}`.
    pub fn for_job(job_id: &str) -> Result<Self, OrthoImportError> {
        let safe_id: String = job_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if safe_id.is_empty() {
            return Err(OrthoImportError::InternalError(
                "job_id tidak valid untuk pembuatan folder staging".to_string(),
            ));
        }
        let staging_dir = super::store::get_images_dir().join(format!(".staging_{}", safe_id));
        Ok(Self {
            staging_dir,
            disarmed: false,
        })
    }

    pub fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    /// Menonaktifkan pembersihan otomatis (dipanggil setelah operasi Point-of-No-Return berhasil di-rename).
    pub fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if !self.disarmed && self.staging_dir.exists() {
            // Batasan keamanan: hanya hapus jika berada tepat di bawah images_dir dan diawali .staging_
            let images_dir = super::store::get_images_dir();
            let is_safe = self.staging_dir.starts_with(&images_dir)
                && self
                    .staging_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.starts_with(".staging_"))
                    .unwrap_or(false);

            if is_safe {
                let _ = fs::remove_dir_all(&self.staging_dir);
            } else {
                eprintln!(
                    "[WARN] StagingGuard menolak menghapus direktori di luar batasan aman: {:?}",
                    self.staging_dir
                );
            }
        }
    }
}

/// Guard RAII untuk pelepasan slot job aktif di registri global.
/// Menjamin slot job selalu dilepaskan saat thread pekerja selesai, error, dibatalkan, atau panic.
pub struct JobRegistrationGuard {
    job_id: String,
    disarmed: bool,
}

impl JobRegistrationGuard {
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            disarmed: false,
        }
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for JobRegistrationGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            release_job(&self.job_id);
        }
    }
}

/// Struktur registri internal untuk job aktif.
struct ActiveJobEntry {
    job_id: String,
    token: CancellationToken,
}

/// Registri global untuk membatasi eksekusi hanya 1 job aktif pada satu waktu.
static ACTIVE_JOB_REGISTRY: OnceLock<Mutex<Option<ActiveJobEntry>>> = OnceLock::new();

fn get_registry() -> &'static Mutex<Option<ActiveJobEntry>> {
    ACTIVE_JOB_REGISTRY.get_or_init(|| Mutex::new(None))
}

/// Mencoba mendaftarkan job baru. Jika sudah ada job aktif, mengembalikan error `JobSedangBerjalan`.
pub fn try_acquire_job(job_id: &str) -> Result<CancellationToken, OrthoImportError> {
    let mut reg = get_registry().lock().map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal mengunci registri job: {}", e))
    })?;

    if let Some(existing) = reg.as_ref() {
        return Err(OrthoImportError::JobSedangBerjalan(existing.job_id.clone()));
    }

    let token = CancellationToken::new();
    *reg = Some(ActiveJobEntry {
        job_id: job_id.to_string(),
        token: token.clone(),
    });

    Ok(token)
}

/// Melepaskan lock registri job aktif saat job selesai, gagal, atau dibatalkan.
pub fn release_job(job_id: &str) {
    if let Ok(mut reg) = get_registry().lock() {
        if let Some(existing) = reg.as_ref() {
            if existing.job_id == job_id {
                *reg = None;
            }
        }
    }
}

/// Mengambil token pembatalan dari job aktif jika ID-nya cocok.
pub fn get_active_job_token(job_id: &str) -> Option<CancellationToken> {
    if let Ok(reg) = get_registry().lock() {
        if let Some(existing) = reg.as_ref() {
            if existing.job_id == job_id {
                return Some(existing.token.clone());
            }
        }
    }
    None
}

/// Direktori penyimpanan file status job (`map_storage/.jobs/`).
pub fn get_jobs_dir() -> PathBuf {
    let dir = get_storage_root().join(".jobs");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

/// Menyimpan atau memperbarui data status job secara atomik ke disk.
pub fn save_job_record(record: &JobRecord) -> Result<(), OrthoImportError> {
    let jobs_dir = get_jobs_dir();
    let job_file = jobs_dir.join(format!("{}.json", record.job_id));
    let json_str = serde_json::to_string_pretty(record)
        .map_err(|e| OrthoImportError::InternalError(format!("Gagal serialisasi job: {}", e)))?;
    write_atomic(&job_file, &json_str)
        .map_err(|e| OrthoImportError::InternalError(format!("Gagal menulis berkas job: {}", e)))?;
    Ok(())
}

/// Membaca satu catatan job dari disk berdasarkan `job_id`.
pub fn load_job_record(job_id: &str) -> Result<Option<JobRecord>, OrthoImportError> {
    let job_file = get_jobs_dir().join(format!("{}.json", job_id));
    if !job_file.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&job_file)
        .map_err(|e| OrthoImportError::InternalError(format!("Gagal membaca berkas job: {}", e)))?;
    let record: JobRecord = serde_json::from_str(&content)
        .map_err(|e| OrthoImportError::FileKorup(format!("Berkas status job korup: {}", e)))?;
    Ok(Some(record))
}

/// Pembersihan startup:
/// 1. Mengubah seluruh job dengan status "running" menjadi "interrupted".
/// 2. Menghapus semua folder staging sisa crash `map_storage/images/.staging_*`.
/// 3. Menerapkan retensi 20 job terakhir (menghapus berkas job tertua di luar 20).
pub fn cleanup_stale_import_jobs() -> Result<(), OrthoImportError> {
    let jobs_dir = get_jobs_dir();

    // 1 & 3: Periksa berkas job di .jobs/
    if jobs_dir.is_dir() {
        let mut job_files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        if let Ok(entries) = fs::read_dir(&jobs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let mtime = entry
                        .metadata()
                        .and_then(|m| m.modified().or_else(|_| m.created()))
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                    // Periksa apakah job berstatus "running"
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(mut record) = serde_json::from_str::<JobRecord>(&content) {
                            if record.status == "running" {
                                record.status = "interrupted".to_string();
                                record.message =
                                    "Proses impor terputus akibat penutupan/crash aplikasi"
                                        .to_string();
                                record.updated_at = chrono::Utc::now().to_rfc3339();
                                if let Ok(updated_json) = serde_json::to_string_pretty(&record) {
                                    let _ = write_atomic(&path, &updated_json);
                                }
                            }
                        }
                    }

                    job_files.push((path, mtime));
                }
            }
        }

        // Urutkan descending berdasarkan mtime (terbaru di awal)
        job_files.sort_by(|a, b| b.1.cmp(&a.1));

        // Hapus berkas job di luar retensi MAX_JOB_HISTORY_RETENTION (20)
        if job_files.len() > MAX_JOB_HISTORY_RETENTION {
            for (old_file, _) in &job_files[MAX_JOB_HISTORY_RETENTION..] {
                let _ = fs::remove_file(old_file);
            }
        }
    }

    // 2: Bersihkan folder .staging_* di map_storage/images/
    let images_dir = super::store::get_images_dir();
    if images_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&images_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = entry.file_name().to_string_lossy().to_string();
                    if folder_name.starts_with(".staging_") {
                        let _ = fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }

    Ok(())
}
