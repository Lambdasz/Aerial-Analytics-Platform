//! Kontrak data catatan anotasi inspeksi peta.

use crate::modules::module_03::types::sp_result_type::SpatialGeometry;
use serde::{Deserialize, Serialize};

/// Returns the default schema version (1) for serialized annotations.
fn default_schema_version() -> u32 {
    1
}

/// Representasi anotasi inspeksi pada peta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnnotationFeature {
    #[serde(alias = "annotation_id")]
    pub id: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub text: String,
    pub geometry: SpatialGeometry,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
}

impl AnnotationFeature {
    /// Kompatibilitas field annotation_id untuk kode lama.
    pub fn annotation_id(&self) -> &str {
        &self.id
    }
}
