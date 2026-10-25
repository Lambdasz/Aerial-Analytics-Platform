//! Penguncian instance aplikasi tunggal (Single-Instance Lock) berbasis file lock eksklusif
//! pada `map_storage/.app.lock` murni tanpa plugin eksternal, menjaga isolasi penuh Module 3.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::storage_utils::get_storage_root;

/// Guard penguncian eksklusif file instance aplikasi.
/// Saat struct ini di-drop, OS otomatis melepaskan lock file.
#[derive(Debug)]
pub struct AppInstanceLock {
    _file: File,
    lock_path: PathBuf,
}

impl AppInstanceLock {
    /// Mencoba mengunci file lock secara eksklusif.
    ///
    /// Pada Windows, menggunakan `share_mode(0)` (dwShareMode = 0) sehingga proses kedua
    /// yang mencoba membuka file ini akan langsung ditolak oleh kernel dengan `ERROR_SHARING_VIOLATION`.
    pub fn try_acquire(lock_path: PathBuf) -> Result<Self, OrthoImportError> {
        if let Some(parent) = lock_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0) // Kunci eksklusif mutlak di level kernel Windows
                .open(&lock_path)
                .map_err(|e| {
                    OrthoImportError::AplikasiSudahBerjalan(format!(
                        "Instansi aplikasi lain sedang berjalan (kunci eksklusif '{}' aktif): {}",
                        lock_path.display(),
                        e
                    ))
                })?;

            // Tulis PID proses saat ini ke file lock
            let pid = std::process::id();
            let _ = file.write_all(format!("PID: {}\n", pid).as_bytes());

            Ok(Self {
                _file: file,
                lock_path,
            })
        }

        #[cfg(not(windows))]
        {
            use fs4::fs_std::FileExt;
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&lock_path)
                .map_err(|e| OrthoImportError::InternalError(e.to_string()))?;

            file.try_lock_exclusive().map_err(|e| {
                OrthoImportError::AplikasiSudahBerjalan(format!(
                    "Instansi aplikasi lain sedang berjalan (advisory flock '{}' aktif): {}",
                    lock_path.display(),
                    e
                ))
            })?;

            let pid = std::process::id();
            let _ = file.write_all(format!("PID: {}\n", pid).as_bytes());

            Ok(Self {
                _file: file,
                lock_path,
            })
        }
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

static GLOBAL_APP_LOCK: OnceLock<Mutex<Option<AppInstanceLock>>> = OnceLock::new();

fn get_lock_holder() -> &'static Mutex<Option<AppInstanceLock>> {
    GLOBAL_APP_LOCK.get_or_init(|| Mutex::new(None))
}

/// Mendapatkan path file kunci instance `map_storage/.app.lock`.
pub fn get_app_lock_path() -> PathBuf {
    get_storage_root().join(".app.lock")
}

/// Mencoba mengunci instance aplikasi tunggal global.
pub fn acquire_global_instance_lock() -> Result<(), OrthoImportError> {
    let mut holder = get_lock_holder().lock().map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal mengunci mutex app lock: {}", e))
    })?;

    if holder.is_some() {
        return Ok(()); // Sudah dikunci oleh proses ini
    }

    let lock = AppInstanceLock::try_acquire(get_app_lock_path())?;
    *holder = Some(lock);
    Ok(())
}

/// Melepaskan kunci instance aplikasi global (misal saat shutdown atau pengujian).
pub fn release_global_instance_lock() {
    if let Ok(mut holder) = get_lock_holder().lock() {
        *holder = None;
    }
}
