//! Fungsi murni untuk pembentukan objek anotasi inspeksi peta.

use super::types::AnnotationFeature;
use crate::modules::module_03::types::sp_result_type::SpatialGeometry;

/// Membentuk data anotasi secara fungsional murni.
pub fn create_annotation(
    id: String,
    text: String,
    geometry: SpatialGeometry,
    created_at: String,
) -> AnnotationFeature {
    AnnotationFeature {
        id,
        schema_version: 1,
        text,
        geometry,
        created_at,
        image_id: None,
    }
}
