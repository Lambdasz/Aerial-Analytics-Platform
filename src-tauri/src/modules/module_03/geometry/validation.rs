//! Validasi koordinat dan struktur geometri GeoJSON.

use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::types::aoi_type::Polygon;
use crate::modules::module_03::types::sp_result_type::SpatialGeometry;

/// Memvalidasi satu koordinat geografis WGS84.
///
/// # Arguments
///
/// * `longitude` — bujur, rentang valid `-180.0..=180.0`.
/// * `latitude` — lintang, rentang valid `-90.0..=90.0`.
///
/// # Errors
///
/// Mengembalikan [`MapControllerError::InvalidGeometry`] jika koordinat tidak hingga
/// atau berada di luar batas yang diizinkan.
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::validate_coordinate;
///
/// assert!(validate_coordinate(116.833, -1.270).is_ok());
/// assert!(validate_coordinate(250.0, -1.270).is_err());
/// ```
pub fn validate_coordinate(longitude: f64, latitude: f64) -> Result<(), MapControllerError> {
    if !longitude.is_finite() || !latitude.is_finite() {
        return Err(MapControllerError::InvalidGeometry(
            "Koordinat mengandung NaN atau infinity".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&longitude) {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Longitude {longitude} di luar rentang -180..=180"
        )));
    }
    if !(-90.0..=90.0).contains(&latitude) {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Latitude {latitude} di luar rentang -90..=90"
        )));
    }
    Ok(())
}

/// Memvalidasi satu pasangan koordinat WGS84 pada konteks geometri tertentu.
fn validate_point_coords(lon: f64, lat: f64, context: &str) -> Result<(), MapControllerError> {
    validate_coordinate(lon, lat)
        .map_err(|err| MapControllerError::InvalidGeometry(format!("{context}: {err}")))
}

/// Memvalidasi ring poligon koordinat tetap `&[[f64; 2]]`.
pub fn validate_fixed_ring(ring_idx: usize, ring: &[[f64; 2]]) -> Result<(), MapControllerError> {
    if ring.len() < 4 {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Ring {ring_idx} minimal butuh 4 titik (termasuk penutup)"
        )));
    }
    if ring.first() != ring.last() {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Ring {ring_idx} tidak tertutup"
        )));
    }
    for (point_idx, &[lon, lat]) in ring.iter().enumerate() {
        validate_point_coords(lon, lat, &format!("Ring {ring_idx}, titik {point_idx}"))?;
    }
    Ok(())
}

/// Memvalidasi ring poligon koordinat dinamis `&[Vec<f64>]`.
pub fn validate_dynamic_ring(ring_idx: usize, ring: &[Vec<f64>]) -> Result<(), MapControllerError> {
    if ring.len() < 4 {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Ring {ring_idx} minimal butuh 4 titik (termasuk penutup)"
        )));
    }
    let first = ring.first().unwrap();
    let last = ring.last().unwrap();
    if first != last {
        return Err(MapControllerError::InvalidGeometry(format!(
            "Ring {ring_idx} tidak tertutup"
        )));
    }
    for (point_idx, point) in ring.iter().enumerate() {
        if point.len() != 2 {
            return Err(MapControllerError::InvalidGeometry(format!(
                "Titik ke-{point_idx} pada ring {ring_idx} membutuhkan tepat 2 koordinat [lon, lat]"
            )));
        }
        validate_point_coords(
            point[0],
            point[1],
            &format!("Ring {ring_idx}, titik {point_idx}"),
        )?;
    }
    Ok(())
}

/// Memvalidasi geometri poligon AOI (ring tertutup, minimal 4 titik, koordinat valid).
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::validate_polygon;
///
/// let valid = vec![vec![
///     [116.833, -1.270],
///     [116.834, -1.270],
///     [116.834, -1.271],
///     [116.833, -1.271],
///     [116.833, -1.270],
/// ]];
/// assert!(validate_polygon(&valid).is_ok());
/// ```
pub fn validate_polygon(polygon: &Polygon) -> Result<(), MapControllerError> {
    if polygon.is_empty() {
        return Err(MapControllerError::InvalidGeometry(
            "Polygon tidak memiliki ring".to_string(),
        ));
    }
    for (ring_idx, ring) in polygon.iter().enumerate() {
        validate_fixed_ring(ring_idx, ring)?;
    }
    Ok(())
}

impl SpatialGeometry {
    /// Memvalidasi integritas koordinat geometri spasial GeoJSON secara deterministik.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        match self {
            SpatialGeometry::Point { coordinates } => {
                if coordinates.len() != 2 {
                    return Err(MapControllerError::InvalidGeometry(format!(
                        "Geometri Point membutuhkan tepat 2 koordinat [lon, lat], tapi mendapat {}",
                        coordinates.len()
                    )));
                }
                validate_point_coords(coordinates[0], coordinates[1], "Geometri Point")?;
            }
            SpatialGeometry::Polygon { coordinates } => {
                if coordinates.is_empty() {
                    return Err(MapControllerError::InvalidGeometry(
                        "Polygon tidak memiliki ring".to_string(),
                    ));
                }
                for (ring_idx, ring) in coordinates.iter().enumerate() {
                    validate_dynamic_ring(ring_idx, ring)?;
                }
            }
            SpatialGeometry::LineString { coordinates } => {
                if coordinates.len() < 2 {
                    return Err(MapControllerError::InvalidGeometry(format!(
                        "Geometri LineString membutuhkan setidaknya 2 titik, mendapat {}",
                        coordinates.len()
                    )));
                }
                for (point_idx, point) in coordinates.iter().enumerate() {
                    if point.len() != 2 {
                        return Err(MapControllerError::InvalidGeometry(format!(
                            "Titik ke-{point_idx} pada LineString membutuhkan tepat 2 koordinat [lon, lat]"
                        )));
                    }
                    validate_point_coords(
                        point[0],
                        point[1],
                        &format!("LineString titik ke-{point_idx}"),
                    )?;
                }
            }
        }
        Ok(())
    }
}
