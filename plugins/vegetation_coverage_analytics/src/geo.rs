//! Geospatial georeferencing, ground sample distance (GSD), and bounds resolution.
//!
//! Pure coordinates and projection calculations (WGS84).

use serde_json::Value;

use crate::types::{ASSUMED_HFOV_DEG, METERS_PER_DEGREE_LAT};

/// Ground sample distance from altitude and 35 mm-equivalent focal length. Pure.
pub fn calculate_gsd(
    relative_altitude_m: f64,
    focal_length_35mm: f64,
    image_width_px: usize,
    image_height_px: usize,
) -> Option<f64> {
    const SENSOR_DIAGONAL_35MM_MM: f64 = 43.27;
    if relative_altitude_m <= 0.0 || focal_length_35mm <= 0.0 {
        return None;
    }
    let diagonal_px = (image_width_px as f64).hypot(image_height_px as f64);
    if diagonal_px <= 0.0 {
        return None;
    }
    Some((relative_altitude_m * SENSOR_DIAGONAL_35MM_MM) / (focal_length_35mm * diagonal_px))
}

/// Ground sample distance from altitude and an assumed horizontal field of view. Pure.
pub fn calculate_gsd_from_hfov(
    relative_altitude_m: f64,
    hfov_deg: f64,
    image_width_px: usize,
) -> Option<f64> {
    if relative_altitude_m <= 0.0 || image_width_px == 0 || !(1.0..179.0).contains(&hfov_deg) {
        return None;
    }
    let ground_width_m = 2.0 * relative_altitude_m * (hfov_deg / 2.0).to_radians().tan();
    Some(ground_width_m / image_width_px as f64)
}

/// `[min_lon, min_lat, max_lon, max_lat]` centered on a GPS point. Pure.
pub fn bounds_from_gps(
    lat: f64,
    lon: f64,
    gsd_m: f64,
    image_width_px: usize,
    image_height_px: usize,
) -> Option<[f64; 4]> {
    if gsd_m <= 0.0 || image_width_px == 0 || image_height_px == 0 {
        return None;
    }
    let ground_width = image_width_px as f64 * gsd_m;
    let ground_height = image_height_px as f64 * gsd_m;
    let delta_lat = (ground_height / 2.0) / METERS_PER_DEGREE_LAT;
    let delta_lon = (ground_width / 2.0) / (METERS_PER_DEGREE_LAT * lat.to_radians().cos());
    Some([
        lon - delta_lon,
        lat - delta_lat,
        lon + delta_lon,
        lat + delta_lat,
    ])
}

/// Pixel centers expressed in WGS84 lon/lat across the image bounds. Pure.
pub fn pixel_centers(
    bounds: [f64; 4],
    width: usize,
    height: usize,
) -> impl Iterator<Item = (usize, f64, f64)> {
    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let lon_span = max_lon - min_lon;
    let lat_span = max_lat - min_lat;
    (0..height).flat_map(move |y| {
        (0..width).map(move |x| {
            let lon = min_lon + lon_span * ((x as f64 + 0.5) / width as f64);
            let lat = max_lat - lat_span * ((y as f64 + 0.5) / height as f64);
            (y * width + x, lon, lat)
        })
    })
}

/// Outer ring of an AOI geometry (`Polygon`) as lon/lat pairs. Pure.
pub fn aoi_ring(payload: &Value) -> Option<Vec<(f64, f64)>> {
    let geometry = &payload["aoi"]["geometry"];
    let coordinates = geometry["coordinates"].as_array()?;
    let ring = coordinates.first()?.as_array()?;
    let points: Vec<(f64, f64)> = ring
        .iter()
        .filter_map(|point| {
            let pair = point.as_array()?;
            Some((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
        })
        .collect();
    (points.len() >= 3).then_some(points)
}

/// Bounds of an AOI: polygon ring or bbox array. Pure.
pub fn bounds_from_aoi(payload: &Value) -> Option<[f64; 4]> {
    if let Some(ring) = aoi_ring(payload) {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for (lon, lat) in ring {
            bounds[0] = bounds[0].min(lon);
            bounds[1] = bounds[1].min(lat);
            bounds[2] = bounds[2].max(lon);
            bounds[3] = bounds[3].max(lat);
        }
        return Some(bounds);
    }
    let bbox = payload["aoi"]["geometry"]["coordinates"].as_array()?;
    let values: Vec<f64> = bbox.iter().filter_map(Value::as_f64).collect();
    (values.len() == 4).then(|| [values[0], values[1], values[2], values[3]])
}

pub fn metadata_number(metadata: &Value, key: &str) -> Option<f64> {
    metadata[key].as_f64()
}

/// GSD from parameters first, then metadata (focal length, then assumed HFOV). Pure.
pub fn resolve_gsd(
    parameters: &Value,
    metadata: &Value,
    width: usize,
    height: usize,
) -> Option<f64> {
    if let Some(gsd) = crate::pipeline::number_param(parameters, "gsd_m").filter(|g| *g > 0.0) {
        return Some(gsd);
    }
    let altitude = metadata_number(metadata, "relative_altitude")
        .or_else(|| metadata_number(metadata, "altitude"));
    let altitude = altitude.filter(|a| *a > 0.0)?;
    if let Some(focal) = metadata_number(metadata, "focal_length_35mm") {
        if focal > 0.0 {
            return calculate_gsd(altitude, focal, width, height);
        }
    }
    calculate_gsd_from_hfov(altitude, ASSUMED_HFOV_DEG, width)
}

/// Explicit `[min_lon, min_lat, max_lon, max_lat]` override supplied by the
/// caller (the app sends the visible flight-photo tile so the grid lines up
/// with the imagery instead of the full camera footprint). Pure.
pub fn parse_bounds_override(parameters: &Value) -> Option<[f64; 4]> {
    let raw = &parameters["image_bounds"];
    let parsed = match raw {
        Value::Array(_) => raw.clone(),
        Value::String(text) if !text.trim().is_empty() => {
            serde_json::from_str::<Value>(text).ok()?
        }
        _ => return None,
    };
    let values: Vec<f64> = parsed
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    if values.len() != 4 || values[2] <= values[0] || values[3] <= values[1] {
        return None;
    }
    Some([values[0], values[1], values[2], values[3]])
}

/// Bounds from the explicit override, else GPS + GSD, else AOI. Pure.
pub fn resolve_bounds(
    payload: &Value,
    parameters: &Value,
    metadata: &Value,
    gsd_m: Option<f64>,
    width: usize,
    height: usize,
) -> Option<[f64; 4]> {
    if let Some(bounds) = parse_bounds_override(parameters) {
        return Some(bounds);
    }
    let gps = &metadata["gps"];
    if let (Some(lat), Some(lon), Some(gsd)) = (gps["lat"].as_f64(), gps["lon"].as_f64(), gsd_m) {
        if let Some(bounds) = bounds_from_gps(lat, lon, gsd, width, height) {
            return Some(bounds);
        }
    }
    bounds_from_aoi(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gsd_fallbacks_behave() {
        assert!(calculate_gsd(0.0, 24.0, 4000, 3000).is_none());
        let gsd = calculate_gsd(100.0, 24.0, 4000, 3000).unwrap();
        assert!(gsd > 0.0);
        let hfov = calculate_gsd_from_hfov(100.0, 60.0, 4000).unwrap();
        assert!((hfov - (2.0 * 100.0 * 30.0_f64.to_radians().tan() / 4000.0)).abs() < 1e-12);
        assert!(calculate_gsd_from_hfov(-1.0, 60.0, 4000).is_none());
    }

    #[test]
    fn bounds_center_on_gps_point() {
        let bounds = bounds_from_gps(0.0, 0.0, 1.0, 100, 50).unwrap();
        assert!((bounds[0] + bounds[2]).abs() < 1e-9);
        assert!((bounds[1] + bounds[3]).abs() < 1e-9);
        assert!(bounds_from_gps(0.0, 0.0, 0.0, 10, 10).is_none());
    }

    #[test]
    fn resolve_bounds_prefers_gps_over_aoi() {
        let payload = json!({
            "aoi": {
                "type": "geojson_polygon",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [[[10.0, 10.0], [10.0, 11.0], [11.0, 11.0], [11.0, 10.0], [10.0, 10.0]]]
                }
            }
        });
        let parameters = json!({});
        let metadata = json!({
            "gps": { "lat": 0.0, "lon": 0.0 }
        });
        let bounds = resolve_bounds(&payload, &parameters, &metadata, Some(1.0), 100, 50).unwrap();
        // Bounds should be centered on GPS (0, 0), not AOI (10..11)
        assert!(bounds[0] < 0.0 && bounds[2] > 0.0);
        assert!(bounds[1] < 0.0 && bounds[3] > 0.0);
    }
}
