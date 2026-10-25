//! Tauri IPC commands untuk pengukuran spasial dan geometri murni.

use crate::modules::module_03::geometry;
use crate::modules::module_03::types::aoi_type::{Coordinate, Polygon};
use crate::plugin_manager::error::CommandError;

/// Mengukur jarak lingkaran besar (great-circle distance) antara dua titik koordinat via Haversine.
#[tauri::command]
pub fn measure_distance(point_a: Coordinate, point_b: Coordinate) -> Result<f64, CommandError> {
    geometry::validate_coordinate(point_a[0], point_a[1])?;
    geometry::validate_coordinate(point_b[0], point_b[1])?;

    Ok(geometry::calculate_distance(point_a, point_b))
}

/// Mengukur luas poligon permukaan bola bumi via spherical excess method.
#[tauri::command]
pub fn measure_area(polygon: Polygon) -> Result<f64, CommandError> {
    geometry::validate_polygon(&polygon)?;

    Ok(geometry::calculate_area(&polygon))
}

/// Mengukur keliling (total perimeter boundary) sebuah poligon.
#[tauri::command]
pub fn measure_perimeter(polygon: Polygon) -> Result<f64, CommandError> {
    geometry::validate_polygon(&polygon)?;

    Ok(geometry::calculate_perimeter(&polygon))
}
