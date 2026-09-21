#![allow(dead_code)]

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

#[derive(Debug, Serialize)]
pub struct TemporalChangeResult {
    pub vegetation_coverage_change: serde_json::Value,
    pub land_cover_transition: serde_json::Value,
    pub tree_count_delta: i32,
}

/// Menyediakan hasil Simple Temporal Change Analysis (M10.8)
/// dengan data yang dapat dikonsumsi Module 11.
pub fn get_temporal_change_result(
    vegetation_coverage_change: serde_json::Value,
    land_cover_transition: serde_json::Value,
    tree_count_delta: i32,
) -> TemporalChangeResult {
    TemporalChangeResult {
        vegetation_coverage_change,
        land_cover_transition,
        tree_count_delta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_change_area_result() {
        let result = get_change_area_result(
            "temporal_change_001".to_string(),
            "Temporal Change Area".to_string(),
            Vec::new(),
        );

        assert_eq!(result.layer_id, "temporal_change_001");
        assert_eq!(result.layer_name, "Temporal Change Area");
        assert_eq!(result.display_preference.display_type, "polygon");
        assert!(result.spatial_results.is_empty());
    }

    #[test]
    fn test_get_temporal_change_result() {
        let vegetation_coverage_change = serde_json::json!({
            "before": 45.0,
            "after": 52.0,
            "delta": 7.0
        });

        let land_cover_transition = serde_json::json!({
            "vegetation_to_bare_land": 120.5,
            "bare_land_to_vegetation": 80.2
        });

        let result = get_temporal_change_result(
            vegetation_coverage_change,
            land_cover_transition,
            15,
        );

        assert_eq!(result.tree_count_delta, 15);
        assert_eq!(
            result.vegetation_coverage_change["delta"],
            serde_json::json!(7.0)
        );
        assert_eq!(
            result.land_cover_transition["vegetation_to_bare_land"],
            serde_json::json!(120.5)
        );
    }
}