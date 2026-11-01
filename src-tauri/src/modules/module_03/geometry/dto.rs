//! Data Transfer Objects (DTO) untuk operasi geometri dan pengukuran spasial.
//!
//! Seluruh DTO mengimplementasikan serialisasi dan deserialisasi JSON dengan konvensi `camelCase`
//! agar selaras dengan kontrak IPC Tauri antara frontend TypeScript dan backend Rust.

use serde::{Deserialize, Serialize};

use crate::modules::module_03::types::aoi_type::{Coordinate, Polygon};

/// Parameter permintaan untuk mengukur jarak lingkaran besar (*great-circle distance*) antara dua titik.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasureDistanceRequest {
    /// Titik koordinat awal `[longitude, latitude]` dalam format derajat desimal WGS84.
    pub point_a: Coordinate,
    /// Titik koordinat tujuan `[longitude, latitude]` dalam format derajat desimal WGS84.
    pub point_b: Coordinate,
}

/// Hasil pengukuran jarak lingkaran besar antara dua titik.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasureDistanceResponse {
    /// Jarak terhitung dalam satuan meter (`m`).
    pub distance_meters: f64,
}

/// Parameter permintaan untuk mengukur luas poligon pada permukaan lengkung bumi.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasureAreaRequest {
    /// Koordinat poligon berformat GeoJSON RFC 7946 (cincin luar dan lubang opsional).
    pub polygon: Polygon,
}

/// Hasil pengukuran luas poligon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasureAreaResponse {
    /// Luas area terhitung dalam satuan meter persegi (`m²`).
    pub area_square_meters: f64,
}

/// Parameter permintaan untuk mengukur panjang keliling poligon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurePerimeterRequest {
    /// Koordinat poligon berformat GeoJSON RFC 7946 (cincin luar dan lubang opsional).
    pub polygon: Polygon,
}

/// Hasil pengukuran keliling poligon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurePerimeterResponse {
    /// Total keliling terhitung dalam satuan meter (`m`).
    pub perimeter_meters: f64,
}
