//! Data Transfer Objects (DTO) untuk pengelolaan citra ortomosaik dan raster.
//!
//! Seluruh DTO mengimplementasikan serialisasi dan deserialisasi JSON dengan konvensi `camelCase`
//! agar selaras dengan kontrak IPC Tauri antara frontend TypeScript dan backend Rust.

use serde::{Deserialize, Serialize};

use super::types::{
    DeleteOrthoSummary, ImportDualModePayload, JobRecord, StoredImageMetadata,
    UpdateOrthoImagePayload,
};

/// Tanggapan daftar seluruh citra ortomosaik yang tersimpan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListOrthoImagesResponse {
    /// Daftar metadata citra ortomosaik tersimpan.
    pub images: Vec<StoredImageMetadata>,
    /// Jumlah total citra yang tersimpan.
    pub total: usize,
}

/// Parameter permintaan untuk mengambil satu metadata citra ortomosaik berdasarkan ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetOrthoImageRequest {
    /// Pengidentifikasi unik citra (misal: `ortho_1729000000`).
    pub id: String,
}

/// Tanggapan yang membungkus metadata satu citra ortomosaik.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetOrthoImageResponse {
    /// Metadata lengkap citra tersimpan.
    pub image: StoredImageMetadata,
}

/// Parameter permintaan untuk memperbarui nama atau koordinat batas (bounds) citra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrthoImageRequest {
    /// Payload perubahan nama atau koordinat batas.
    pub payload: UpdateOrthoImagePayload,
}

/// Tanggapan hasil pembaruan citra ortomosaik.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrthoImageResponse {
    /// Metadata citra setelah berhasil diperbarui.
    pub image: StoredImageMetadata,
}

/// Parameter permintaan kalkulasi dampak penghapusan cascade citra ortomosaik.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetOrthoCascadeSummaryRequest {
    /// ID citra yang akan dihitung dependensinya.
    pub id: String,
}

/// Parameter permintaan penghapusan citra ortomosaik secara cascade tuntas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOrthoImageRequest {
    /// ID citra yang akan dihapus.
    pub id: String,
}

/// Tanggapan laporan hasil penghapusan cascade citra ortomosaik.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOrthoImageResponse {
    /// Ringkasan dampak komponen yang berhasil dihapus dari storage.
    pub summary: DeleteOrthoSummary,
}

/// Parameter permintaan memulai pekerjaan impor citra di background thread.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartImportOrthoJobRequest {
    /// Payload konfigurasi mode impor (Opsi A / Opsi B) beserta path berkas.
    pub payload: ImportDualModePayload,
}

/// Tanggapan berisi ID pekerjaan impor yang baru dimulai.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartImportOrthoJobResponse {
    /// ID pekerjaan unik (misal: `job_1729000000`) untuk pemantauan progres atau pembatalan.
    pub job_id: String,
}

/// Parameter permintaan pembatalan pekerjaan impor citra yang sedang berjalan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CancelImportJobRequest {
    /// ID pekerjaan yang ingin dibatalkan.
    pub job_id: String,
}

/// Tanggapan konfirmasi pembatalan pekerjaan impor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CancelImportJobResponse {
    /// Status apakah sinyal pembatalan berhasil dikirim (`true` jika berhasil).
    pub success: bool,
    /// ID pekerjaan yang dibatalkan.
    pub job_id: String,
}

/// Parameter permintaan pengecekan status pekerjaan impor citra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetImportJobStatusRequest {
    /// ID pekerjaan yang ingin diperiksa status dan progresnya.
    pub job_id: String,
}

/// Tanggapan berisi rekor progres status pekerjaan impor citra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetImportJobStatusResponse {
    /// Catatan status terkini proses impor citra dari storage.
    pub record: JobRecord,
}

/// Parameter permintaan validasi awal header berkas citra GeoTIFF (Opsi B).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InspectGeotiffHeaderRequest {
    /// Path absolut berkas citra GeoTIFF pada disk lokal.
    pub file_path: String,
}
