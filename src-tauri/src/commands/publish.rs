//! Publishes a completed plugin run into Module 3 map storage.
//!
//! Thin command: it validates the incoming result, converts the first GeoJSON
//! artifact into a `LayerPayload`, and hands it to the shared map state, which
//! persists it and broadcasts `spatial_result:saved` so Map Explorer refreshes
//! itself. Raster-only results (mask/overlay PNGs carrying WGS84 bounds) are
//! copied into `map_storage/overlays/` and published as image-overlay layers.

use crate::modules::module_03::sp_result::types::{
    FieldDef, LayerDisplayMode, LayerPayload, SpatialGeometry, SpatialProperties, SpatialResult,
};
use crate::modules::module_03::state::get_global_state;
use crate::plugin_manager::error::CommandError;
use serde_json::{Map, Value};
use tauri::Emitter;

/// Coverage layer styling: green fill, translucent so the basemap shows through.
const LAYER_COLOR: &str = "#1a7f37";
const LAYER_OPACITY: f64 = 0.5;

/// Sanitizes a plugin execution id into a valid spatial-result layer id.
fn layer_id_from(execution_id: &str) -> String {
    let sanitized: String = execution_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if sanitized.is_empty() {
        "plugin_result".to_string()
    } else {
        sanitized
    }
}

/// Finds the first GeoJSON artifact advertised by a plugin result.
#[cfg(test)]
fn geojson_artifact_path(result: &Value) -> Option<String> {
    result["artifacts"]
        .as_object()?
        .values()
        .find(|artifact| {
            artifact["format"]
                .as_str()
                .is_some_and(|format| format == "application/geo+json")
                || artifact["file_path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(".geojson"))
        })
        .and_then(|artifact| artifact["file_path"].as_str())
        .map(str::to_string)
}

/// Finds a raster artifact that carries WGS84 bounds, preferring the human
/// friendly overlay over the raw mask. Returns `(file_path, bounds)` where
/// bounds are `[min_lon, min_lat, max_lon, max_lat]`.
#[cfg(test)]
fn raster_artifact(result: &Value) -> Option<(String, [f64; 4])> {
    let artifacts = result["artifacts"].as_object()?;
    let preferred = [
        "detection_overlay",
        "overlay",
        "vegetation_mask",
        "mask_image",
    ];
    let mut ordered: Vec<&Value> = preferred
        .iter()
        .filter_map(|key| artifacts.get(*key))
        .collect();
    ordered.extend(artifacts.values());

    ordered.into_iter().find_map(|artifact| {
        let format = artifact["format"].as_str().unwrap_or("");
        if !format.starts_with("image/") {
            return None;
        }
        let path = artifact["file_path"].as_str()?.to_string();
        let bounds: Vec<f64> = artifact["bounds"]
            .as_array()?
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        if bounds.len() != 4 || bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
            return None;
        }
        Some((path, [bounds[0], bounds[1], bounds[2], bounds[3]]))
    })
}

/// Bounds of one raster artifact as `[min_lon, min_lat, max_lon, max_lat]`.
fn raster_artifact_for(artifact: &Value) -> Option<(String, [f64; 4])> {
    let format = artifact["format"].as_str().unwrap_or("");
    if !format.starts_with("image/") {
        return None;
    }
    let path = artifact["file_path"].as_str()?.to_string();
    let bounds: Vec<f64> = artifact["bounds"]
        .as_array()?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    if bounds.len() != 4 || bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
        return None;
    }
    Some((path, [bounds[0], bounds[1], bounds[2], bounds[3]]))
}

/// Finds a raster artifact that carries WGS84 bounds, preferring the human
/// friendly overlay over the raw mask. Returns `(file_path, bounds)` where
/// bounds are `[min_lon, min_lat, max_lon, max_lat]`.
/// Converts one GeoJSON feature into a spatial result. Pure.
fn convert_feature(feature: &Value, source: &str) -> Result<Option<SpatialResult>, CommandError> {
    let geometry: SpatialGeometry = match serde_json::from_value(feature["geometry"].clone()) {
        Ok(geometry) => geometry,
        Err(_) => return Ok(None),
    };
    let properties: Map<String, Value> = feature["properties"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    Ok(Some(SpatialResult {
        source: source.to_string(),
        geometry,
        properties: SpatialProperties::Generic(properties),
        image_url: None,
        edited_fields: None,
        extra: Map::new(),
    }))
}

/// Attributes shown in the popup for coverage cells.
fn coverage_fields() -> Vec<FieldDef> {
    [
        ("cell_id", "Cell", Some("text")),
        ("coverage_percentage", "Coverage", Some("number")),
        ("classification", "Class", Some("text")),
        ("vegetation_pixels", "Vegetation pixels", Some("number")),
        ("total_pixels", "Total pixels", Some("number")),
    ]
    .into_iter()
    .map(|(key, label, kind)| FieldDef {
        key: key.to_string(),
        label: label.to_string(),
        unit: None,
        visible: true,
        custom: false,
        kind: kind.map(str::to_string),
    })
    .collect()
}

/// Publish a finished plugin run so Map Explorer shows its output.
///
/// Every artifact becomes its own layer: GeoJSON artifacts become polygon or
/// point layers, georeferenced rasters become image-overlay layers under
/// `map_storage/overlays/`. Binary masks and vector zonations from the same run
/// therefore stay independently toggleable instead of one hiding the rest.
#[tauri::command]
pub fn publish_plugin_run(
    app: tauri::AppHandle,
    result: Value,
    layer_name: Option<String>,
) -> Result<Vec<LayerPayload>, CommandError> {
    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;

    let execution_id = result["execution_id"]
        .as_str()
        .or_else(|| result["job_id"].as_str())
        .unwrap_or("plugin_result")
        .to_string();
    let base_name = layer_name.unwrap_or_else(|| format!("Plugin result: {execution_id}"));

    let artifacts = result["artifacts"].as_object().cloned().unwrap_or_default();
    if artifacts.is_empty() {
        return Err(CommandError {
            code: "NO_MAP_OUTPUT".to_string(),
            message: "This run produced no artifacts to place on the map".to_string(),
        });
    }

    let mut payloads: Vec<LayerPayload> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for (key, artifact) in &artifacts {
        let path = artifact["file_path"].as_str().unwrap_or("");
        if path.is_empty() {
            skipped.push(format!("{key}: missing file_path"));
            continue;
        }
        let rid = layer_id_from(&format!("{execution_id}_{key}"));
        let name = format!("{base_name} · {key}");

        let built = if artifact["format"]
            .as_str()
            .is_some_and(|format| format == "application/geo+json")
            || path.ends_with(".geojson")
        {
            vector_payload(&execution_id, &rid, name, path, &result)
        } else if let Some((raster_path, bounds)) = raster_artifact_for(artifact) {
            raster_payload(&execution_id, &rid, name, &raster_path, bounds, &result)
        } else {
            skipped.push(format!("{key}: not a georeferenced raster or GeoJSON"));
            continue;
        };

        match built {
            Ok(payload) => payloads.push(payload),
            Err(error) => skipped.push(format!("{key}: {}", error.message)),
        }
    }

    if payloads.is_empty() {
        return Err(CommandError {
            code: "NO_MAP_OUTPUT".to_string(),
            message: format!(
                "No artifact could be placed on the map ({})",
                skipped.join("; ")
            ),
        });
    }

    let mut saved_layers = Vec::with_capacity(payloads.len());
    for payload in payloads {
        let saved = get_global_state().save_spatial_result(payload)?;
        let _ = app.emit("spatial_result:saved", &saved);
        saved_layers.push(saved);
    }
    Ok(saved_layers)
}

/// Builds a polygon/point layer from the run's first GeoJSON artifact.
fn vector_payload(
    execution_id: &str,
    layer_id: &str,
    name: String,
    artifact_path: &str,
    result: &Value,
) -> Result<LayerPayload, CommandError> {
    let text = std::fs::read_to_string(artifact_path).map_err(|error| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("Cannot read artifact '{artifact_path}': {error}"),
    })?;
    let geojson: Value = serde_json::from_str(&text).map_err(|error| CommandError {
        code: "SERIALIZATION_ERROR".to_string(),
        message: format!("Artifact '{artifact_path}' is not valid GeoJSON: {error}"),
    })?;

    let features = geojson["features"].as_array().ok_or_else(|| CommandError {
        code: "INVALID_ARTIFACT".to_string(),
        message: "GeoJSON artifact has no features array".to_string(),
    })?;
    let spatial_results: Vec<SpatialResult> = features
        .iter()
        .filter_map(|feature| convert_feature(feature, execution_id).transpose())
        .collect::<Result<Vec<_>, CommandError>>()?;

    if spatial_results.is_empty() {
        return Err(CommandError {
            code: "EMPTY_ARTIFACT".to_string(),
            message: "GeoJSON artifact contains no features the map can draw".to_string(),
        });
    }

    let mut extra = Map::new();
    extra.insert("metrics".to_string(), result["metrics"].clone());
    extra.insert(
        "plugin_execution_id".to_string(),
        Value::String(execution_id.to_string()),
    );

    Ok(LayerPayload {
        layer_id: layer_id.to_string(),
        layer_name: name,
        display_preference: LayerDisplayMode::Polygon {
            color: LAYER_COLOR.to_string(),
            opacity: LAYER_OPACITY,
            image_url: None,
            fields: Some(coverage_fields()),
        },
        spatial_results,
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
        concurrency_token: None,
        extra,
    })
}

/// Copies a raster artifact into map storage and wraps it as an image overlay.
/// The single spatial result is the overlay's anchor point (its geometry is
/// ignored by the image-overlay renderer).
fn raster_payload(
    execution_id: &str,
    layer_id: &str,
    name: String,
    raster_path: &str,
    bounds: [f64; 4],
    result: &Value,
) -> Result<LayerPayload, CommandError> {
    let storage_dir = get_global_state().storage_dir.clone();
    let overlays_dir = storage_dir.join("overlays");
    std::fs::create_dir_all(&overlays_dir).map_err(|error| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("Cannot create '{}': {error}", overlays_dir.display()),
    })?;
    let destination = overlays_dir.join(format!("{layer_id}.png"));
    std::fs::copy(raster_path, &destination).map_err(|error| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!(
            "Cannot copy '{raster_path}' to '{}': {error}",
            destination.display()
        ),
    })?;

    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let mut extra = Map::new();
    extra.insert("metrics".to_string(), result["metrics"].clone());
    extra.insert(
        "plugin_execution_id".to_string(),
        Value::String(execution_id.to_string()),
    );

    Ok(LayerPayload {
        layer_id: layer_id.to_string(),
        layer_name: name,
        display_preference: LayerDisplayMode::ImageOverlay {
            image_url: format!("map_storage/overlays/{layer_id}.png"),
            opacity: 0.6,
            bounds: [[min_lat, min_lon], [max_lat, max_lon]],
            fields: Some(coverage_fields()),
        },
        spatial_results: vec![SpatialResult {
            source: execution_id.to_string(),
            geometry: SpatialGeometry::Point {
                coordinates: vec![(min_lon + max_lon) / 2.0, (min_lat + max_lat) / 2.0],
            },
            properties: SpatialProperties::Generic(Map::new()),
            image_url: None,
            edited_fields: None,
            extra: Map::new(),
        }],
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
        concurrency_token: None,
        extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn layer_id_is_sanitized_and_bounded() {
        assert_eq!(layer_id_from("exec_123-abc"), "exec_123-abc");
        assert_eq!(layer_id_from("exec/weird id"), "exec_weird_id");
        assert_eq!(layer_id_from(&"x".repeat(100)).len(), 64);
        assert_eq!(layer_id_from(""), "plugin_result");
    }

    #[test]
    fn first_geojson_artifact_is_selected() {
        let result = json!({
            "artifacts": {
                "vegetation_mask": { "file_path": "/tmp/mask.png", "format": "image/png" },
                "coverage_zonation": {
                    "file_path": "/tmp/zonation.geojson",
                    "format": "application/geo+json"
                }
            }
        });
        assert_eq!(
            geojson_artifact_path(&result).as_deref(),
            Some("/tmp/zonation.geojson")
        );
        assert!(geojson_artifact_path(&json!({ "artifacts": {} })).is_none());
    }

    #[test]
    fn raster_artifact_prefers_overlay_and_requires_bounds() {
        let result = json!({
            "artifacts": {
                "vegetation_mask": {
                    "file_path": "/tmp/mask.png",
                    "format": "image/png",
                    "bounds": [115.97, 29.75, 115.98, 29.76]
                },
                "detection_overlay": {
                    "file_path": "/tmp/overlay.png",
                    "format": "image/png",
                    "bounds": [115.97, 29.75, 115.98, 29.76]
                },
                "broken": { "file_path": "/tmp/x.png", "format": "image/png" }
            }
        });
        let (path, bounds) = raster_artifact(&result).unwrap();
        assert_eq!(path, "/tmp/overlay.png");
        assert_eq!(bounds, [115.97, 29.75, 115.98, 29.76]);

        let no_bounds = json!({
            "artifacts": { "mask": { "file_path": "/tmp/m.png", "format": "image/png" } }
        });
        assert!(raster_artifact(&no_bounds).is_none());
    }

    #[test]
    fn polygon_features_convert_with_properties() {
        let feature = json!({
            "type": "Feature",
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]
            },
            "properties": { "cell_id": "r1c1", "coverage_percentage": 42.5 }
        });
        let converted = convert_feature(&feature, "exec_1").unwrap().unwrap();
        assert_eq!(converted.source, "exec_1");
        assert!(matches!(
            converted.geometry,
            SpatialGeometry::Polygon { .. }
        ));
        match converted.properties {
            SpatialProperties::Generic(properties) => {
                assert_eq!(properties["cell_id"], "r1c1");
            }
            other => panic!("expected generic properties, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_geometry_is_skipped() {
        let feature = json!({ "geometry": { "type": "MultiPoint", "coordinates": [[0.0, 0.0]] } });
        assert!(convert_feature(&feature, "exec_1").unwrap().is_none());
    }
}
