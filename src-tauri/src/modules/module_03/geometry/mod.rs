//! Modul geometri murni dan pengukuran spasial.

pub mod haversine;
pub mod spherical_excess;
pub mod validation;

pub use haversine::*;
pub use spherical_excess::*;
pub use validation::*;

pub use crate::modules::module_03::error::MapControllerError as GeometryError;

// Re-export untuk kompatibilitas doctest dan kode eksisting
pub use crate::modules::module_03::aoi::builder::create_aoi;
pub use crate::modules::module_03::types::aoi_type::{
    AoiFeature, AoiGeometry, AoiProperties, Coordinate, Polygon,
};

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
