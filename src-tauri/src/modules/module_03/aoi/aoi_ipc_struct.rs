//! Berkas ini berisi definisi struct DTO (Data Transfer Object) untuk komunikasi IPC Area of Interest (AOI).
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `CreateAoiRequest`: Struct parameter pembuatan fitur AOI baru di memori.
//! - `SaveNewAoiRequest`: Struct parameter pembuatan dan persistensi instan AOI baru.
//! - `SaveAoiRequest`: Struct parameter penyimpanan fitur AOI yang sudah ada ke disk.
//! - `DeleteAoiRequest`: Struct parameter penghapusan AOI berdasarkan ID.
//! - `AoiFeatureResponse`: Struct respons tunggal yang membungkus objek AoiFeature.
//! - `GetSavedAoisResponse`: Struct respons envelope yang memuat koleksi AOI dan total record.
//! - `DeleteAoiResponse`: Struct respons konfirmasi penghapusan AOI.

use serde::{Deserialize, Serialize};

use super::aoi_entity_struct::{AoiFeature, Polygon};

/// Parameter permintaan pembuatan fitur poligon AOI baru (di memori / validasi draft).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAoiRequest {
    /// Pengidentifikasi unik AOI (misal: `aoi_1729000000`).
    pub aoi_id: String,
    /// Nama deskriptif area yang ditentukan oleh pengguna.
    pub name: String,
    /// Kategori atau klasifikasi AOI opsional (misal: `inspection`, `crop_damage`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aoi_type: Option<String>,
    /// Koordinat batas poligon GeoJSON dalam derajat desimal WGS84.
    pub polygon: Polygon,
    /// Stempel waktu pembuatan format ISO 8601 (RFC 3339).
    pub created_at: String,
}

/// Parameter permintaan pembuatan dan penyimpanan AOI baru secara atomik langsung ke storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveNewAoiRequest {
    /// Pengidentifikasi unik AOI.
    pub aoi_id: String,
    /// Nama deskriptif area.
    pub name: String,
    /// Kategori atau tipe AOI opsional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aoi_type: Option<String>,
    /// Koordinat batas poligon GeoJSON dalam derajat desimal WGS84.
    pub polygon: Polygon,
    /// Stempel waktu pembuatan opsional (jika kosong, backend akan mengisi waktu saat ini).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Parameter permintaan penyimpanan fitur AOI yang sudah ada ke penyimpanan disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveAoiRequest {
    /// Objek lengkap `AoiFeature` GeoJSON yang akan disimpan ke `map_storage/aois/{id}.json`.
    pub aoi: AoiFeature,
}

/// Tanggapan yang membungkus objek fitur AOI tunggal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AoiFeatureResponse {
    /// Data lengkap GeoJSON Feature AOI beserta properti kalkulasi luas dan keliling.
    pub aoi: AoiFeature,
}

/// Tanggapan daftar seluruh AOI yang tersimpan pada sistem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetSavedAoisResponse {
    /// Kumpulan fitur AOI yang berhasil dimuat dari storage disk.
    pub aois: Vec<AoiFeature>,
    /// Jumlah total AOI yang tersimpan.
    pub total: usize,
}

/// Parameter permintaan penghapusan berkas AOI berdasarkan ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAoiRequest {
    /// Pengidentifikasi unik AOI yang akan dihapus.
    pub aoi_id: String,
}

/// Tanggapan status hasil eksekusi penghapusan AOI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAoiResponse {
    /// Status apakah operasi penghapusan berkas berhasil dijalankan (`true` jika berhasil).
    pub success: bool,
    /// ID AOI yang telah dihapus.
    pub aoi_id: String,
}
