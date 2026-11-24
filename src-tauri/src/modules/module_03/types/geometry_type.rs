//! Kontrak data geometri geospasial dasar (RFC 7946 GeoJSON).
//!
//! Komponen yang didefinisikan dalam berkas ini:
//! - `SpatialGeometry`: Representasi bentuk geometri GeoJSON (Point, Polygon, LineString).

use serde::{Deserialize, Serialize};

/// Bentuk geometri dari hasil spasial yang didukung (GeoJSON RFC 7946).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum SpatialGeometry {
    Point { coordinates: Vec<f64> },
    Polygon { coordinates: Vec<Vec<Vec<f64>>> },
    LineString { coordinates: Vec<Vec<f64>> },
}
