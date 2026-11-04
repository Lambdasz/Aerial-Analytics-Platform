//! Berkas ini adalah modul agregator dan re-export untuk submodul Geometri & Pengukuran Spasial.
//!
//! Submodul yang terdaftar:
//! - `geometry_ipc_cmd`: Endpoint Tauri Command untuk pengukuran jarak, luas, dan perimeter.
//! - `geometry_ipc_struct`: Struct DTO Request/Response untuk IPC pengukuran geometri.
//! - `geometry_validation_logic`: Validasi murni koordinat geografis dan cincin GeoJSON.
//! - `geospatial_projection_logic`: Algoritma murni reproyeksi koordinat (WGS84, EPSG:3857, UTM).
//! - `haversine_distance_logic`: Formula trigonometri murni jarak lingkaran besar Haversine.
//! - `spherical_excess_logic`: Formula matematika murni luas poligon bola dan keliling.

pub mod geometry_ipc_cmd;
pub mod geometry_ipc_struct;
pub mod geometry_validation_logic;
pub mod geospatial_projection_logic;
pub mod haversine_distance_logic;
pub mod spherical_excess_logic;

pub use geometry_ipc_cmd::*;
pub use geometry_ipc_struct::*;
pub use geometry_validation_logic::*;
pub use geospatial_projection_logic::*;
pub use haversine_distance_logic::*;
pub use spherical_excess_logic::*;

pub use crate::modules::module_03::error::MapControllerError as GeometryError;

// Re-export untuk kompatibilitas doctest dan kode eksisting
pub use crate::modules::module_03::aoi::aoi_builder_logic::create_aoi;
pub use crate::modules::module_03::aoi::aoi_entity_struct::{
    AoiFeature, AoiGeometry, AoiProperties, Coordinate, Polygon,
};

// Alias kompatibilitas internal
pub use geometry_ipc_cmd as commands;
pub use geometry_ipc_struct as dto;
pub use geometry_validation_logic as validation;
pub use geospatial_projection_logic as projection;
pub use haversine_distance_logic as haversine;
pub use spherical_excess_logic as spherical_excess;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_03::types::SpatialGeometry;

    /// Helper function generating a sample square polygon for unit tests.
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
            None,
            sample_square(),
            "2026-09-14T19:00:15Z".into(),
            12500.5,
            500.0,
        );
        assert_eq!(aoi.r#type, "Feature");
        assert_eq!(aoi.geometry.r#type, "Polygon");
        assert_eq!(aoi.properties.aoi_id, "aoi_001");
    }

    #[test]
    fn spatial_geometry_polygon_validation() {
        let valid_geom = SpatialGeometry::Polygon {
            coordinates: vec![vec![
                vec![116.833, -1.270],
                vec![116.834, -1.270],
                vec![116.834, -1.271],
                vec![116.833, -1.271],
                vec![116.833, -1.270],
            ]],
        };
        assert!(valid_geom.validate().is_ok());

        let invalid_range_geom = SpatialGeometry::Polygon {
            coordinates: vec![vec![
                vec![195.833, -1.270],
                vec![116.834, -1.270],
                vec![116.834, -1.271],
                vec![116.833, -1.271],
                vec![195.833, -1.270],
            ]],
        };
        assert!(invalid_range_geom.validate().is_err());
    }

    #[test]
    fn spatial_geometry_point_validation() {
        let valid_point = SpatialGeometry::Point {
            coordinates: vec![116.833, -1.270],
        };
        assert!(valid_point.validate().is_ok());

        let invalid_coord = SpatialGeometry::Point {
            coordinates: vec![116.833, -95.0],
        };
        assert!(invalid_coord.validate().is_err());
    }
}
