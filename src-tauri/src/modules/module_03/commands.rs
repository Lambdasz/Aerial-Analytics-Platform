//! Tauri IPC commands for AOI & spatial measurement (Module 3 — Person 2).
//!
//! Each command is **thin**: parse input ➜ delegate to [`geometry`] ➜ map
//! errors to [`CommandError`]. No business logic lives here.
//!
//! [`geometry`]: crate::map_controller::sp_measurement::geometry
//! [`CommandError`]: crate::plugin_manager::error::CommandError

use crate::map_controller::sp_measurement::geometry::{self, AoiFeature, Coordinate, Polygon};
use crate::plugin_manager::error::CommandError;

/// Measures the great-circle distance between two points using the Haversine
/// formula.
///
/// # Arguments
///
/// * `point_a` — `[longitude, latitude]` of the first point (WGS84).
/// * `point_b` — `[longitude, latitude]` of the second point (WGS84).
///
/// # Returns
///
/// Distance in **meters**.
///
/// # Errors
///
/// Returns [`CommandError`] if either coordinate fails validation.
#[tauri::command]
pub fn measure_distance(point_a: Coordinate, point_b: Coordinate) -> Result<f64, CommandError> {
    geometry::validate_coordinate(point_a[0], point_a[1])?;
    geometry::validate_coordinate(point_b[0], point_b[1])?;

    Ok(geometry::calculate_distance(point_a, point_b))
}

/// Measures the area of a polygon using the spherical excess method.
///
/// # Arguments
///
/// * `polygon` — GeoJSON-style polygon rings (`Vec<Vec<[lon, lat]>>`).
///
/// # Returns
///
/// Area in **square meters**.
///
/// # Errors
///
/// Returns [`CommandError`] if the polygon fails structural validation.
#[tauri::command]
pub fn measure_area(polygon: Polygon) -> Result<f64, CommandError> {
    geometry::validate_polygon(&polygon)?;

    Ok(geometry::calculate_area(&polygon))
}

/// Measures the perimeter (total boundary length) of a polygon.
///
/// # Arguments
///
/// * `polygon` — GeoJSON-style polygon rings (`Vec<Vec<[lon, lat]>>`).
///
/// # Returns
///
/// Perimeter in **meters**.
///
/// # Errors
///
/// Returns [`CommandError`] if the polygon fails structural validation.
#[tauri::command]
pub fn measure_perimeter(polygon: Polygon) -> Result<f64, CommandError> {
    geometry::validate_polygon(&polygon)?;

    Ok(geometry::calculate_perimeter(&polygon))
}

/// Creates a validated AOI (Area of Interest) GeoJSON Feature.
///
/// Validates the polygon, computes area & perimeter via the pure geometry
/// functions, and assembles a complete [`AoiFeature`].
///
/// # Arguments
///
/// * `aoi_id`     — unique identifier for this AOI.
/// * `name`       — human-readable name.
/// * `polygon`    — GeoJSON-style polygon rings.
/// * `created_at` — ISO 8601 timestamp string.
///
/// # Returns
///
/// A fully populated [`AoiFeature`] ready for IPC serialisation.
///
/// # Errors
///
/// Returns [`CommandError`] if the polygon is structurally invalid.
#[tauri::command]
pub fn create_aoi_cmd(
    aoi_id: String,
    name: String,
    polygon: Polygon,
    created_at: String,
) -> Result<AoiFeature, CommandError> {
    geometry::validate_polygon(&polygon)?;

    let area_sq_m = geometry::calculate_area(&polygon);
    let perimeter_m = geometry::calculate_perimeter(&polygon);

    Ok(geometry::create_aoi(
        aoi_id,
        name,
        polygon,
        created_at,
        area_sq_m,
        perimeter_m,
    ))
}
