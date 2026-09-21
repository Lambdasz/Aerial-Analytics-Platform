use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SpatialResultPayload {
    pub layer_id: String,
    pub layer_name: String,
    pub display_preference: DisplayPreference,
    pub spatial_results: Vec<SpatialResultItem>,
}

#[derive(Debug, Serialize)]
pub struct DisplayPreference {
    #[serde(rename = "type")]
    pub display_type: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SpatialResultItem {
    pub source: String,
    pub geometry: serde_json::Value,
    pub properties: serde_json::Value,
}

/// Menyediakan hasil Change Area Detection (M10.7)
/// dengan format yang dapat dikonsumsi Module 3.
pub fn get_change_area_result(
    layer_id: String,
    layer_name: String,
    spatial_results: Vec<SpatialResultItem>,
) -> SpatialResultPayload {
    SpatialResultPayload {
        layer_id,
        layer_name,
        display_preference: DisplayPreference {
            display_type: "polygon".to_string(),
            color: Some("#FF0000".to_string()),
            icon: None,
        },
        spatial_results,
    }
}