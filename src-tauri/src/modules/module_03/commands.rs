//! Tauri IPC commands for AOI & spatial measurement (Module 3).
//!
//! Each command is **thin**: parse input ➜ delegate to [`geometry`] / [`aoi`] ➜ map
//! errors to [`CommandError`]. No business logic lives here.

use crate::modules::module_03::aoi;
use crate::modules::module_03::geometry;
use crate::modules::module_03::types::aoi_type::{AoiFeature, Coordinate, Polygon};
use crate::plugin_manager::error::CommandError;

/// Measures the great-circle distance between two points using the Haversine formula.
#[tauri::command]
pub fn measure_distance(point_a: Coordinate, point_b: Coordinate) -> Result<f64, CommandError> {
    geometry::validate_coordinate(point_a[0], point_a[1])?;
    geometry::validate_coordinate(point_b[0], point_b[1])?;

    Ok(geometry::calculate_distance(point_a, point_b))
}

/// Measures the area of a polygon using the spherical excess method.
#[tauri::command]
pub fn measure_area(polygon: Polygon) -> Result<f64, CommandError> {
    geometry::validate_polygon(&polygon)?;

    Ok(geometry::calculate_area(&polygon))
}

/// Measures the perimeter (total boundary length) of a polygon.
#[tauri::command]
pub fn measure_perimeter(polygon: Polygon) -> Result<f64, CommandError> {
    geometry::validate_polygon(&polygon)?;

    Ok(geometry::calculate_perimeter(&polygon))
}

/// Creates a validated AOI (Area of Interest) GeoJSON Feature.
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

    Ok(aoi::create_aoi(
        aoi_id,
        name,
        polygon,
        created_at,
        area_sq_m,
        perimeter_m,
    ))
}
