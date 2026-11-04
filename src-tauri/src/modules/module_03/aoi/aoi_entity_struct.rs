//! Berkas ini berisi definisi struct entitas Area of Interest (AOI) standar GeoJSON.
//!
//! Komponen dan tipe data yang didefinisikan dalam berkas ini:
//! - `Coordinate`: Tipe alias untuk pasangan koordinat geografis [longitude, latitude].
//! - `Polygon`: Tipe alias untuk cincin-cincin poligon GeoJSON.
//! - `AoiProperties`: Struct metadata atribut AOI (nama, jenis, luas meter persegi, keliling).
//! - `AoiGeometry`: Struct geometri poligon GeoJSON.
//! - `AoiFeature`: Struct GeoJSON Feature lengkap yang merepresentasikan AOI.

use serde::{Deserialize, Serialize};

/// Satu titik koordinat `[longitude, latitude]` (RFC 7946, WGS84).
pub type Coordinate = [f64; 2];

/// Polygon: daftar *linear ring*. Ring pertama = outer, sisanya = hole.
pub type Polygon = Vec<Vec<Coordinate>>;

/// Metadata AOI untuk `properties` GeoJSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AoiProperties {
    pub aoi_id: String,
    pub name: String,
    #[serde(default, alias = "tpe", alias = "tipe", alias = "type")]
    pub aoi_type: Option<String>,
    pub calculated_area_sq_m: f64,
    pub perimeter_m: f64,
    pub created_at: String,
}

/// Bagian `geometry` dari GeoJSON Feature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AoiGeometry {
    pub r#type: String,
    pub coordinates: Polygon,
}

/// AOI standar siap dipertukarkan ke Module 2 dan Module 4–10.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AoiFeature {
    pub r#type: String,
    pub properties: AoiProperties,
    pub geometry: AoiGeometry,
}
