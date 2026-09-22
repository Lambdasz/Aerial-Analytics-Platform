//! # Geometry, AOI & Spatial Measurement
//!
//! Kumpulan **pure function** untuk perhitungan geometri AOI dan alat
//! pengukuran spasial (Distance, Area, Perimeter, Coordinate Inspector).
//!
//! ## Konvensi Koordinat
//!
//! - Format: `[longitude, latitude]` sesuai GeoJSON RFC 7946.
//! - CRS: WGS84 (EPSG:4326).
//! - Satuan: meter (`m`) untuk jarak & perimeter, meter persegi (`m²`) untuk luas.

#![allow(dead_code)]

use crate::map_controller::sp_measurement::validation_input::MapControllerError;

/// Jari-jari bumi rata-rata (meter).
const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Alias error geometri (memakai `MapControllerError` yang sudah ada).
pub type GeometryError = MapControllerError;

/// Satu titik koordinat `[longitude, latitude]` (RFC 7946, WGS84).
pub type Coordinate = [f64; 2];

/// Polygon: daftar *linear ring*. Ring pertama = outer, sisanya = hole.
pub type Polygon = Vec<Vec<Coordinate>>;

/// Metadata AOI untuk `properties` GeoJSON.
#[derive(Debug, Clone)]
pub struct AoiProperties {
    pub aoi_id: String,
    pub name: String,
    pub calculated_area_sq_m: f64,
    pub perimeter_m: f64,
    pub created_at: String,
}

/// Bagian `geometry` dari GeoJSON Feature.
#[derive(Debug, Clone)]
pub struct AoiGeometry {
    pub r#type: String,
    pub coordinates: Polygon,
}

/// AOI standar siap dipertukarkan ke Module 2 dan Module 4–10.
#[derive(Debug, Clone)]
pub struct AoiFeature {
    pub r#type: String,
    pub properties: AoiProperties,
    pub geometry: AoiGeometry,
}

// =============================================================================
// 1. validate_coordinate
// =============================================================================

/// Memvalidasi satu koordinat geografis.
///
/// # Arguments
///
/// * `longitude` — bujur, harus `-180.0..=180.0` dan bukan NaN/infinity.
/// * `latitude` — lintang, harus `-90.0..=90.0` dan bukan NaN/infinity.
///
/// # Returns
///
/// * `Ok(())` — koordinat valid.
/// * `Err(GeometryError::InvalidGeometry)` — di luar rentang.
///
/// # Purity
///
/// **Pure function.**
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::validate_coordinate;
///
/// assert!(validate_coordinate(116.833, -1.270).is_ok());
/// assert!(validate_coordinate(250.0, -1.270).is_err());
/// ```
pub fn validate_coordinate(longitude: f64, latitude: f64) -> Result<(), GeometryError> {
    if !longitude.is_finite() || !latitude.is_finite() {
        return Err(GeometryError::InvalidGeometry(
            "Koordinat mengandung NaN atau infinity".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&longitude) {
        return Err(GeometryError::InvalidGeometry(format!(
            "Longitude {longitude} di luar rentang -180..=180"
        )));
    }
    if !(-90.0..=90.0).contains(&latitude) {
        return Err(GeometryError::InvalidGeometry(format!(
            "Latitude {latitude} di luar rentang -90..=90"
        )));
    }
    Ok(())
}

// =============================================================================
// 2. validate_polygon
// =============================================================================

/// Memvalidasi geometri polygon AOI.
///
/// Cek struktur: ring tertutup, minimal 4 elemen, koordinat valid.
/// Cek self-intersection **belum** ada — akan ditambah bertahap.
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
pub fn validate_polygon(polygon: &Polygon) -> Result<(), GeometryError> {
    if polygon.is_empty() {
        return Err(GeometryError::InvalidGeometry(
            "Polygon tidak memiliki ring".to_string(),
        ));
    }
    for (ring_idx, ring) in polygon.iter().enumerate() {
        if ring.len() < 4 {
            return Err(GeometryError::InvalidGeometry(format!(
                "Ring {ring_idx} minimal butuh 4 titik (termasuk penutup)"
            )));
        }
        if ring.first() != ring.last() {
            return Err(GeometryError::InvalidGeometry(format!(
                "Ring {ring_idx} tidak tertutup"
            )));
        }
        for (point_idx, &[lon, lat]) in ring.iter().enumerate() {
            if validate_coordinate(lon, lat).is_err() {
                return Err(GeometryError::InvalidGeometry(format!(
                    "Ring {ring_idx}, titik {point_idx}: koordinat tidak valid"
                )));
            }
        }
    }
    Ok(())
}

// =============================================================================
// 3. calculate_distance
// =============================================================================

/// Menghitung jarak antara dua titik (formula Haversine).
///
/// # Returns
///
/// Jarak dalam **meter**.
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

// =============================================================================
// 4. calculate_area
// =============================================================================

/// Menghitung luas polygon (spherical excess).
///
/// Ring 0 = outer (ditambah), ring berikutnya = hole (dikurangi).
///
/// # Returns
///
/// Luas dalam **meter persegi**.
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

// =============================================================================
// 5. calculate_perimeter
// =============================================================================

/// Menghitung keliling polygon.
///
/// # Returns
///
/// Keliling dalam **meter**.
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

// =============================================================================
// 6. create_aoi
// =============================================================================

/// Membentuk polygon valid menjadi objek AOI standar.
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::sp_measurement::geometry::create_aoi;
///
/// let polygon = vec![vec![
///     [116.833, -1.270],
///     [116.834, -1.270],
///     [116.834, -1.271],
///     [116.833, -1.271],
///     [116.833, -1.270],
/// ]];
/// let aoi = create_aoi(
///     "aoi_001".into(),
///     "Area Analisis".into(),
///     polygon,
///     "2026-09-14T19:00:15Z".into(),
///     12500.5,
///     500.0,
/// );
/// assert_eq!(aoi.r#type, "Feature");
/// assert_eq!(aoi.geometry.r#type, "Polygon");
/// ```
pub fn create_aoi(
    aoi_id: String,
    name: String,
    polygon: Polygon,
    created_at: String,
    area_sq_m: f64,
    perimeter_m: f64,
) -> AoiFeature {
    AoiFeature {
        r#type: "Feature".to_string(),
        properties: AoiProperties {
            aoi_id,
            name,
            calculated_area_sq_m: area_sq_m,
            perimeter_m,
            created_at,
        },
        geometry: AoiGeometry {
            r#type: "Polygon".to_string(),
            coordinates: polygon,
        },
    }
}

// =============================================================================
// Unit test
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_square() -> Polygon {
        vec![vec![
            [116.833, -1.270],
            [116.834, -1.270],
            [116.834, -1.271],
            [116.833, -1.271],
            [116.833, -1.270],
        ]]
    }

    #[test]
    fn coordinate_valid() {
        assert!(validate_coordinate(116.833, -1.270).is_ok());
    }

    #[test]
    fn coordinate_out_of_range() {
        assert!(validate_coordinate(250.0, -1.270).is_err());
        assert!(validate_coordinate(116.833, -100.0).is_err());
    }

    #[test]
    fn coordinate_rejects_nan() {
        assert!(validate_coordinate(f64::NAN, 0.0).is_err());
    }

    #[test]
    fn polygon_valid() {
        assert!(validate_polygon(&sample_square()).is_ok());
    }

    #[test]
    fn polygon_rejects_empty() {
        let empty: Polygon = vec![];
        assert!(validate_polygon(&empty).is_err());
    }

    #[test]
    fn polygon_rejects_unclosed() {
        let unclosed = vec![vec![
            [116.833, -1.270],
            [116.834, -1.270],
            [116.834, -1.271],
            [116.833, -1.271],
        ]];
        assert!(validate_polygon(&unclosed).is_err());
    }

    #[test]
    fn distance_identical_is_zero() {
        let d = calculate_distance([116.833, -1.270], [116.833, -1.270]);
        assert!(d.abs() < 1e-9);
    }

    #[test]
    fn distance_reasonable() {
        let d = calculate_distance([116.833, -1.270], [116.835, -1.270]);
        assert!((d - 222.71).abs() < 2.0);
    }

    #[test]
    fn area_positive() {
        assert!(calculate_area(&sample_square()) > 0.0);
    }

    #[test]
    fn perimeter_positive() {
        assert!(calculate_perimeter(&sample_square()) > 0.0);
    }

    #[test]
    fn create_aoi_shape() {
        let aoi = create_aoi(
            "aoi_001".into(),
            "Test".into(),
            sample_square(),
            "2026-09-14T19:00:15Z".into(),
            12500.5,
            500.0,
        );
        assert_eq!(aoi.r#type, "Feature");
        assert_eq!(aoi.geometry.r#type, "Polygon");
        assert_eq!(aoi.properties.aoi_id, "aoi_001");
    }
}
