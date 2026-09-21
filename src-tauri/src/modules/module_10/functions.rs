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

/// Provides the spatial result of Change Area Detection from Module 10.7
/// for consumption by Module 3.4.
///
/// This function packages the detected change areas into the
/// `SpatialResultPayload` contract expected by the spatial visualization
/// layer of Module 3.
///
/// # Arguments
///
/// * `layer_id` - Unique identifier of the temporal change result layer.
/// * `layer_name` - Display name of the temporal change result layer.
/// * `spatial_results` - Spatial change results containing source,
///   geometry, and additional properties.
///
/// # Returns
///
/// Returns a [`SpatialResultPayload`] containing the layer metadata,
/// display preference, and spatial change results.
///
/// # Module Contract
///
/// This function represents the cross-module contract:
/// **Module 10.7 → Module 3.4**.
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

/// Provides temporal change analysis results from Module 10.8
/// for consumption by Module 11.
///
/// The result contains the temporal analysis data required by Module 11,
/// including vegetation coverage change, land-cover transition data,
/// and the change in detected tree count.
///
/// # Arguments
///
/// * `vegetation_coverage_change` - Data describing the change in
///   vegetation coverage between observation periods.
/// * `land_cover_transition` - Data describing transitions between
///   land-cover classes across observation periods.
/// * `tree_count_delta` - Difference in detected tree count between
///   observation periods.
///
/// # Returns
///
/// Returns a [`TemporalChangeResult`] containing the temporal change
/// analysis results.
///
/// # Module Contract
///
/// This function represents the cross-module contract:
/// **Module 10.8 → Module 11.5**.
///
/// The data fields follow the Module 11 contract for Module 10:
/// vegetation coverage change, land-cover transition matrix,
/// and tree count delta.
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

        let result =
            get_temporal_change_result(vegetation_coverage_change, land_cover_transition, 15);

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
