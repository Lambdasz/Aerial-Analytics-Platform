//! Berkas ini berisi definisi struct DTO (Data Transfer Object) untuk pengukuran geometri spasial.
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `MeasureDistanceRequest`: Parameter koordinat dua titik [longitude, latitude] untuk hitung jarak.
//! - `MeasureDistanceResponse`: Hasil perhitungan jarak dalam satuan meter.
//! - `MeasureAreaRequest`: Parameter poligon GeoJSON untuk hitung luas.
//! - `MeasureAreaResponse`: Hasil perhitungan luas dalam meter persegi.
//! - `MeasurePerimeterRequest`: Parameter poligon GeoJSON untuk hitung keliling.
//! - `MeasurePerimeterResponse`: Hasil perhitungan keliling dalam satuan meter.

use serde::{Deserialize, Serialize};

use crate::modules::module_03::aoi::aoi_entity_struct::{Coordinate, Polygon};

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
