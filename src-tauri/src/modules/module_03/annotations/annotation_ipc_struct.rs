//! Berkas ini berisi definisi struct DTO (Data Transfer Object) untuk komunikasi IPC catatan anotasi inspeksi.
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `CreateAnnotationRequest`: Struct parameter pembuatan anotasi baru.
//! - `UpdateAnnotationRequest`: Struct parameter pembaruan isi teks anotasi.
//! - `DeleteAnnotationRequest`: Struct parameter penghapusan anotasi berdasarkan ID.
//! - `AnnotationFeatureResponse`: Struct respons tunggal yang membungkus AnnotationFeature.
//! - `GetSavedAnnotationsResponse`: Struct respons envelope koleksi anotasi dan total hitungan.
//! - `DeleteAnnotationResponse`: Struct respons konfirmasi penghapusan anotasi.

use serde::{Deserialize, Serialize};

use super::annotation_entity_struct::AnnotationFeature;
use crate::modules::module_03::types::geometry_type::SpatialGeometry;

/// Parameter permintaan pembuatan catatan anotasi baru pada peta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAnnotationRequest {
    /// Pengidentifikasi unik anotasi.
    pub annotation_id: String,
    /// Isi teks atau catatan temuan inspeksi lapangan.
    pub text: String,
    /// Geometri titik, garis, atau poligon letak anotasi.
    pub geometry: SpatialGeometry,
    /// Stempel waktu pembuatan format ISO 8601 (RFC 3339).
    pub created_at: String,
}

/// Tanggapan yang membungkus objek fitur catatan anotasi tunggal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationResponse {
    /// Data lengkap fitur catatan anotasi inspeksi.
    pub annotation: AnnotationFeature,
}

/// Tanggapan daftar seluruh catatan anotasi yang tersimpan di sistem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetSavedAnnotationsResponse {
    /// Kumpulan fitur anotasi tersimpan.
    pub annotations: Vec<AnnotationFeature>,
    /// Jumlah total anotasi yang tersimpan.
    pub total: usize,
}

/// Parameter permintaan untuk mengambil satu catatan anotasi spesifik berdasarkan ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetAnnotationByIdRequest {
    /// ID anotasi yang dicari.
    pub annotation_id: String,
}

/// Parameter permintaan pembaruan catatan anotasi yang sudah ada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAnnotationRequest {
    /// Objek lengkap `AnnotationFeature` yang telah diperbarui isi atau posisinya.
    pub annotation: AnnotationFeature,
}

/// Parameter permintaan penghapusan catatan anotasi berdasarkan ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAnnotationRequest {
    /// ID anotasi yang akan dihapus.
    pub annotation_id: String,
}

/// Tanggapan status hasil eksekusi penghapusan catatan anotasi.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAnnotationResponse {
    /// Status keberhasilan penghapusan berkas (`true` jika berhasil).
    pub success: bool,
    /// ID anotasi yang telah dihapus.
    pub annotation_id: String,
}
