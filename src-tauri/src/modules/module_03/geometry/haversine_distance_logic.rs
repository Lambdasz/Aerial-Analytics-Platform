//! Berkas ini berisi logika matematika murni perhitungan jarak geodesik menggunakan formula Haversine.
//!
//! Komponen dan konstanta yang didefinisikan dalam berkas ini:
//! - `EARTH_RADIUS_M`: Jari-jari rata-rata bola bumi WGS84 (6.371.008,8 meter).
//! - `calculate_distance`: Menghitung jarak lingkaran besar antara dua koordinat geografis (zero I/O).

use crate::modules::module_03::aoi::aoi_entity_struct::Coordinate;

/// Jari-jari bumi rata-rata (meter).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Menghitung jarak lingkaran besar antara dua titik geografis (formula Haversine).
///
/// # Arguments
///
/// * `point_a` — koordinat `[longitude, latitude]` titik awal.
/// * `point_b` — koordinat `[longitude, latitude]` titik akhir.
///
/// # Returns
///
/// Jarak dalam satuan **meter**.
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::calculate_distance;
///
/// let d = calculate_distance([116.833, -1.270], [116.835, -1.270]);
/// assert!((d - 222.71).abs() < 2.0);
/// ```
pub fn calculate_distance(point_a: Coordinate, point_b: Coordinate) -> f64 {
    let (lon1, lat1) = (point_a[0].to_radians(), point_a[1].to_radians());
    let (lon2, lat2) = (point_b[0].to_radians(), point_b[1].to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_M * c
}
