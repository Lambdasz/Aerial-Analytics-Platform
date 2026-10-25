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
                    if e.raw_os_error() == Some(32) {
                        // ERROR_SHARING_VIOLATION (OS error 32): proses lain sedang membuka berkas lock
                        OrthoImportError::AplikasiSudahBerjalan(format!(
                            "Instansi aplikasi lain sedang berjalan (kunci eksklusif '{}' aktif): {}",
                            lock_path.display(),
                            e
                        ))
                    } else {
                        // Error I/O lain (PermissionDenied, ReadOnly, dsb)
                        eprintln!(
                            "[INSTANCE LOCK] Kegagalan I/O saat membuka berkas lock '{}' (bukan sharing violation): {}",
                            lock_path.display(),
                            e
                        );
                        OrthoImportError::InternalError(format!(
                            "Gagal mengakses berkas lock instance '{}': {}",
                            lock_path.display(),
                            e
                        ))
                    }
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
                .map_err(|e| {
                    eprintln!(
                        "[INSTANCE LOCK] Kegagalan I/O saat membuka berkas lock '{}': {}",
                        lock_path.display(),
                        e
                    );
                    OrthoImportError::InternalError(format!(
                        "Gagal membuka berkas lock instance '{}': {}",
                        lock_path.display(),
                        e
                    ))
                })?;

            file.try_lock_exclusive().map_err(|e| {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    OrthoImportError::AplikasiSudahBerjalan(format!(
                        "Instansi aplikasi lain sedang berjalan (advisory flock '{}' aktif): {}",
                        lock_path.display(),
                        e
                    ))
                } else {
                    eprintln!(
                        "[INSTANCE LOCK] Kegagalan I/O saat mengunci flock '{}': {}",
                        lock_path.display(),
                        e
                    );
                    OrthoImportError::InternalError(format!(
                        "Gagal mengunci berkas lock instance '{}': {}",
                        lock_path.display(),
                        e
                    ))
                }
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
static IS_PRIMARY_INSTANCE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Memeriksa apakah proses ini adalah instansi primer pemegang write-lock.
pub fn is_primary_instance() -> bool {
    IS_PRIMARY_INSTANCE.load(std::sync::atomic::Ordering::SeqCst)
}

/// Mengatur status instansi primer (berguna untuk pengujian atau override).
pub fn set_is_primary_instance(val: bool) {
    IS_PRIMARY_INSTANCE.store(val, std::sync::atomic::Ordering::SeqCst);
}

/// Menolak operasi penyimpanan / modifikasi storage jika aplikasi berjalan sebagai instansi sekunder.
pub fn require_primary_instance() -> Result<(), OrthoImportError> {
    if !is_primary_instance() {
        return Err(OrthoImportError::AplikasiSudahBerjalan(
            "Operasi modifikasi storage ditolak: aplikasi dibuka dalam instansi sekunder (mode hanya-baca aktif).".to_string(),
        ));
    }
    Ok(())
}

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
        set_is_primary_instance(true);
        return Ok(()); // Sudah dikunci oleh proses ini
    }

    match AppInstanceLock::try_acquire(get_app_lock_path()) {
        Ok(lock) => {
            *holder = Some(lock);
            set_is_primary_instance(true);
            Ok(())
        }
        Err(e) => {
            set_is_primary_instance(false);
            Err(e)
        }
    }
}

/// Melepaskan kunci instance aplikasi global (misal saat shutdown atau pengujian).
pub fn release_global_instance_lock() {
    if let Ok(mut holder) = get_lock_holder().lock() {
        *holder = None;
        set_is_primary_instance(false);
    }
}
