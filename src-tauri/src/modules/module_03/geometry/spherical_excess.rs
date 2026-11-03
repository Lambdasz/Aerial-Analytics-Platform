//! Perhitungan luas dan keliling poligon di permukaan bumi (spherical excess).

use super::haversine::{calculate_distance, EARTH_RADIUS_M};
use crate::modules::module_03::types::aoi_type::Polygon;

/// Menghitung luas poligon bola (spherical excess).
///
/// Ring pertama = outer (ditambah), ring berikutnya = hole (dikurangi).
///
/// # Returns
///
/// Luas dalam satuan **meter persegi**.
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::calculate_area;
///
/// let aoi = vec![vec![
///     [116.833, -1.270],
///     [116.834, -1.270],
///     [116.834, -1.271],
///     [116.833, -1.271],
///     [116.833, -1.270],
/// ]];
/// assert!(calculate_area(&aoi) > 0.0);
/// ```
pub fn calculate_area(polygon: &Polygon) -> f64 {
    if polygon.is_empty() || polygon[0].len() < 4 {
        return 0.0;
    }
    let mut total = 0.0;
    for (idx, ring) in polygon.iter().enumerate() {
        let sign = if idx == 0 { 1.0 } else { -1.0 };
        let n = ring.len();
        let mut ring_area = 0.0;
        for i in 0..n - 1 {
            let lon1 = ring[i][0].to_radians();
            let lat1 = ring[i][1].to_radians();
            let lon2 = ring[i + 1][0].to_radians();
            let lat2 = ring[i + 1][1].to_radians();
            ring_area += (lon2 - lon1) * (2.0 + lat1.sin() + lat2.sin());
        }
        total += sign * (ring_area * EARTH_RADIUS_M * EARTH_RADIUS_M / 2.0).abs();
    }
    total.abs()
}

/// Menghitung keliling poligon (total panjang batas).
///
/// # Returns
///
/// Keliling dalam satuan **meter**.
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::calculate_perimeter;
///
/// let aoi = vec![vec![
///     [116.833, -1.270],
///     [116.834, -1.270],
///     [116.834, -1.271],
///     [116.833, -1.271],
///     [116.833, -1.270],
/// ]];
/// assert!(calculate_perimeter(&aoi) > 0.0);
/// ```
pub fn calculate_perimeter(polygon: &Polygon) -> f64 {
    polygon
        .iter()
        .map(|ring| {
            if ring.len() < 2 {
                return 0.0;
            }
            ring.windows(2)
                .map(|w| calculate_distance(w[0], w[1]))
                .sum::<f64>()
        })
        .sum()
}
