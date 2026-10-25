#![allow(dead_code)]

//! Error types for the Map Controller module (Module 3).

use thiserror::Error;

/// Domain error for map controller operations.
///
/// Maps to [`CommandError`](crate::plugin_manager::error::CommandError) for
/// transmission to the frontend via Tauri IPC.
#[derive(Error, Debug)]
pub enum MapControllerError {
    /// The provided geometry failed validation (e.g. self-intersecting ring,
    /// invalid coordinate bounds).
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),

    /// An I/O operation failed while reading or writing map data.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A storage or transaction operation failed.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Kesalahan khusus alur impor citra orthomosaic.
    #[error(transparent)]
    OrthoImport(#[from] OrthoImportError),
}

impl MapControllerError {
    /// Returns a short error code for client-side programmatic matching.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidGeometry(_) => "INVALID_GEOMETRY",
            Self::Io(_) => "IO_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::StorageError(_) => "STORAGE_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::OrthoImport(err) => err.code(),
        }
    }
}

impl From<MapControllerError> for crate::plugin_manager::error::CommandError {
    fn from(err: MapControllerError) -> Self {
        crate::plugin_manager::error::CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}

/// Kesalahan spesifik pada alur impor citra orthomosaic dual-mode (Opsi A dan Opsi B).
#[derive(Error, Debug, Clone, PartialEq)]
pub enum OrthoImportError {
    /// Format berkas tidak dikenali atau tidak didukung.
    #[error("Format berkas tidak dikenali atau tidak didukung: {0}")]
    FormatTidakDidukung(String),

    /// Kompresi TIFF tidak didukung (mis. JPEG-in-TIFF kompleks atau WebP).
    #[error("Kompresi TIFF '{0}' belum didukung")]
    KompresiTidakDidukung(String),

    /// Berkas terpotong atau korup.
    #[error("Berkas citra rusak atau tidak lengkap: {0}")]
    FileKorup(String),

    /// Dimensi, resolusi, atau ukuran berkas melampaui batas aman.
    #[error("Melebihi batas aman: {message}")]
    MelebihiBatas {
        width: u32,
        height: u32,
        max_dimension: u32,
        message: String,
    },

    /// Koordinat WGS84 tidak valid (mis. south >= north atau west >= east).
    #[error("Batas koordinat tidak valid: {0}")]
    KoordinatTidakValid(String),

    /// Kode CRS belum didukung oleh converter murni.
    #[error("Sistem koordinat citra ({0}) belum didukung otomatis")]
    CrsTidakDidukung(String),

    /// Berkas memuat nilai rotasi/shear affine non-nol.
    #[error("Citra memiliki rotasi/kemiringan ({0} rad). Peta hanya mendukung citra tegak lurus (North-up)")]
    RotasiTidakDidukung(f64),

    /// Ruang hard disk tidak mencukupi.
    #[error("Penyimpanan disk tidak mencukupi: {0}")]
    DiskPenuh(String),

    /// Ada job impor lain yang sedang aktif.
    #[error("Proses impor lain sedang berlangsung (Job ID: {0})")]
    JobSedangBerjalan(String),

    /// Proses dibatalkan oleh pengguna via tombol Batalkan.
    #[error("Proses impor citra telah dibatalkan oleh pengguna")]
    Dibatalkan,

    /// Instansi aplikasi lain sedang berjalan dan memegang kunci eksklusif.
    #[error("Instansi aplikasi lain sedang berjalan: {0}")]
    AplikasiSudahBerjalan(String),

    /// Kesalahan internal tak terduga.
    #[error("Terjadi kesalahan internal: {0}")]
    InternalError(String),
}

impl OrthoImportError {
    /// Mengembalikan kode error standar untuk matching programmatic di frontend.
    pub fn code(&self) -> &'static str {
        match self {
            Self::FormatTidakDidukung(_) => "FORMAT_TIDAK_DIDUKUNG",
            Self::KompresiTidakDidukung(_) => "KOMPRESI_TIDAK_DIDUKUNG",
            Self::FileKorup(_) => "FILE_KORUP",
            Self::MelebihiBatas { .. } => "MELEBIHI_BATAS",
            Self::KoordinatTidakValid(_) => "KOORDINAT_TIDAK_VALID",
            Self::CrsTidakDidukung(_) => "CRS_TIDAK_DIDUKUNG",
            Self::RotasiTidakDidukung(_) => "ROTASI_TIDAK_DIDUKUNG",
            Self::DiskPenuh(_) => "DISK_PENUH",
            Self::JobSedangBerjalan(_) => "JOB_SEDANG_BERJALAN",
            Self::AplikasiSudahBerjalan(_) => "APLIKASI_SUDAH_BERJALAN",
            Self::Dibatalkan => "DIBATALKAN",
            Self::InternalError(_) => "INTERNAL_ERROR",
        }
    }

    /// Menentukan apakah error ini dapat dipulihkan oleh pengguna (misal dengan mengubah input manual).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::KoordinatTidakValid(_)
                | Self::CrsTidakDidukung(_)
                | Self::DiskPenuh(_)
                | Self::JobSedangBerjalan(_)
        )
    }

    /// Mengembalikan pesan ramah pengguna dalam Bahasa Indonesia sesuai matriks dokumen desain.
    pub fn user_friendly_message(&self) -> String {
        match self {
            Self::FormatTidakDidukung(_) => {
                "Format berkas tidak dikenali atau tidak didukung. Harap gunakan berkas TIFF, Baseline JPEG, atau PNG dalam format warna RGB/RGBA.".to_string()
            }
            Self::KompresiTidakDidukung(kompresi) => {
                format!(
                    "Kompresi TIFF '{}' belum didukung oleh decoder pure Rust. Harap ekspor ulang berkas dengan kompresi Deflate, LZW, atau Uncompressed (Tiled GeoTIFF).",
                    kompresi
                )
            }
            Self::FileKorup(_) => {
                "Berkas citra rusak atau tidak lengkap sehingga tidak dapat diproses.".to_string()
            }
            Self::MelebihiBatas { message, .. } => message.clone(),
            Self::KoordinatTidakValid(_) => {
                "Batas koordinat tidak valid: Lintang Selatan harus lebih kecil dari Lintang Utara, dan Bujur Barat harus lebih kecil dari Bujur Timur.".to_string()
            }
            Self::CrsTidakDidukung(crs) => {
                format!(
                    "Sistem koordinat citra ({}) belum didukung otomatis. Anda dapat memasukkan koordinat batas WGS84 secara manual.",
                    crs
                )
            }
            Self::RotasiTidakDidukung(_) => {
                "Citra memiliki orientasi rotasi/kemiringan. Peta hanya mendukung citra tegak lurus (North-up).".to_string()
            }
            Self::DiskPenuh(req) => {
                format!(
                    "Penyimpanan disk tidak mencukupi. Membutuhkan setidaknya {} ruang kosong.",
                    req
                )
            }
            Self::JobSedangBerjalan(_) => {
                "Proses impor lain sedang berlangsung. Harap tunggu hingga selesai atau batalkan job aktif.".to_string()
            }
            Self::AplikasiSudahBerjalan(_) => {
                "Instansi aplikasi lain sedang berjalan. Operasi penyimpanan dan impor dibatasi untuk mencegah konflik data.".to_string()
            }
            Self::Dibatalkan => {
                "Proses impor citra telah dibatalkan oleh pengguna.".to_string()
            }
            Self::InternalError(d) => {
                format!("Terjadi kesalahan internal saat memproses citra: {}", d)
            }
        }
    }
}

impl From<OrthoImportError> for crate::plugin_manager::error::CommandError {
    fn from(err: OrthoImportError) -> Self {
        crate::plugin_manager::error::CommandError {
            code: err.code().to_string(),
            message: err.user_friendly_message(),
        }
    }
}
