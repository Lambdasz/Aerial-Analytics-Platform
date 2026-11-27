//! Berkas ini berisi implementasi endpoint Tauri IPC Command untuk pengukuran geometri spasial.
//!
//! Komponen dan fungsi command yang didefinisikan dalam berkas ini:
//! - `measure_distance`: Command untuk menghitung jarak lingkaran besar antara dua titik via Haversine.
//! - `measure_area`: Command untuk menghitung luas poligon permukaan bola bumi via Spherical Excess.
//! - `measure_perimeter`: Command untuk menghitung keliling poligon batas terluar dalam meter.

use super::geometry_ipc_struct::{
    MeasureAreaRequest, MeasureAreaResponse, MeasureDistanceRequest, MeasureDistanceResponse,
    MeasurePerimeterRequest, MeasurePerimeterResponse,
};
use crate::modules::module_03::geometry;
use crate::plugin_manager::error::CommandError;

/// Mengukur jarak lingkaran besar (great-circle distance) antara dua titik koordinat via Haversine.
#[tauri::command]
pub fn measure_distance(
    request: MeasureDistanceRequest,
) -> Result<MeasureDistanceResponse, CommandError> {
    geometry::validate_coordinate(request.point_a[0], request.point_a[1])?;
    geometry::validate_coordinate(request.point_b[0], request.point_b[1])?;

    let distance_meters = geometry::calculate_distance(request.point_a, request.point_b);
    Ok(MeasureDistanceResponse { distance_meters })
}

/// Mengukur luas poligon permukaan bola bumi via spherical excess method.
#[tauri::command]
pub fn measure_area(request: MeasureAreaRequest) -> Result<MeasureAreaResponse, CommandError> {
    geometry::validate_polygon(&request.polygon)?;

    let area_square_meters = geometry::calculate_area(&request.polygon);
    Ok(MeasureAreaResponse { area_square_meters })
}

/// Mengukur keliling (total perimeter boundary) sebuah poligon.
#[tauri::command]
pub fn measure_perimeter(
    request: MeasurePerimeterRequest,
) -> Result<MeasurePerimeterResponse, CommandError> {
    geometry::validate_polygon(&request.polygon)?;

    let perimeter_meters = geometry::calculate_perimeter(&request.polygon);
    Ok(MeasurePerimeterResponse { perimeter_meters })
}
