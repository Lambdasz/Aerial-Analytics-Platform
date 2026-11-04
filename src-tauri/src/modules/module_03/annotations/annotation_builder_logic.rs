//! Berkas ini berisi logika fungsional murni pembentukan objek catatan anotasi peta (zero I/O).
//!
//! Komponen dan fungsi yang didefinisikan dalam berkas ini:
//! - `create_annotation`: Membentuk struct AnnotationFeature secara deterministik tanpa efek samping.

use super::annotation_entity_struct::AnnotationFeature;
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
