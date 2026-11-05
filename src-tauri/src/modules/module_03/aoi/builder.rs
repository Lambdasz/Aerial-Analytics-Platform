//! Pembuatan dan validasi poligon Area of Interest (AOI).

use crate::modules::module_03::types::aoi_type::{AoiFeature, AoiGeometry, AoiProperties, Polygon};

/// Membentuk polygon yang valid menjadi objek GeoJSON AOI standar.
///
/// # Example
///
/// ```
/// use aerial_analytics_platform_lib::map_controller::aoi::create_aoi;
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
///     Some("Analisis".into()),
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
    aoi_type: Option<String>,
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
            aoi_type,
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
