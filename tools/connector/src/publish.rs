//! Publishes a validated plugin run into Module 3 map storage so Map
//! Explorer shows the result without touching app code.
//!
//! Mirrors `module_03::sp_result::save_spatial_result_in_dir` output in the
//! flat-file flavour the dual reader accepts: one `spatial_results/{id}.json`
//! holding the full `LayerPayload`, plus the `layers/{id}.json` visibility
//! registration the writer also creates.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

const LAYER_COLOR: &str = "#1a7f37"; // outputs.json layer_hint.stroke_color
const LAYER_ICON: &str = "tree_marker";
const SOURCE: &str = "connector";

/// `--storage` flag > `AAP_STORAGE_DIR` > `./map_storage` (same hierarchy as
/// `module_03::storage_utils::get_storage_root`, minus the cwd-parent probes
/// — the connector is always run from the repo root).
pub fn resolve_storage_root(flag: Option<&Path>) -> PathBuf {
    if let Some(dir) = flag {
        return dir.to_path_buf();
    }
    if let Ok(dir) = std::env::var("AAP_STORAGE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from("map_storage")
}

#[derive(Debug)]
pub struct PublishOutcome {
    pub layer_id: String,
    pub layer_path: PathBuf,
    pub feature_count: usize,
}

/// Layer id must pass `validate_spatial_result_id`: `^[A-Za-z0-9_-]{1,64}$`.
fn layer_id_for(execution_id: &str) -> String {
    // execution_id is `conn_<uuid>` — already valid; just clamp + sanitize.
    execution_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Map one GeoJSON feature to a Module 3 `SpatialResult`.
fn convert_feature(feature: &Value) -> Result<Option<Value>> {
    let geometry = feature
        .get("geometry")
        .context("feature missing geometry")?;
    let gtype = geometry["type"]
        .as_str()
        .context("geometry.type not a string")?;
    let coords = &geometry["coordinates"];
    let geom = match gtype {
        "Point" => {
            let c = coords
                .as_array()
                .context("Point coordinates not an array")?;
            if c.len() < 2 || !c[0].is_number() || !c[1].is_number() {
                bail!("Point coordinates must be [lon, lat] numbers: {coords}");
            }
            serde_json::json!({ "type": "Point", "coordinates": [c[0], c[1]] })
        }
        "Polygon" => serde_json::json!({ "type": "Polygon", "coordinates": coords }),
        "LineString" => serde_json::json!({ "type": "LineString", "coordinates": coords }),
        other => {
            eprintln!("# skipping feature with unsupported geometry {other:?}");
            return Ok(None);
        }
    };
    let properties = feature
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(Some(serde_json::json!({
        "source": SOURCE,
        "geometry": geom,
        "properties": Value::Object(properties),
    })))
}

/// Reads a GeoJSON FeatureCollection artifact into `SpatialResult` values.
fn features_from_geojson(path: &Path) -> Result<Vec<Value>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read artifact {}", path.display()))?;
    let doc: Value =
        serde_json::from_str(&raw).with_context(|| format!("parse artifact {}", path.display()))?;
    if doc["type"] != "FeatureCollection" {
        bail!("{} is not a GeoJSON FeatureCollection", path.display());
    }
    let features = doc["features"]
        .as_array()
        .context("features not an array")?;
    let mut out = Vec::with_capacity(features.len());
    for f in features {
        if let Some(converted) = convert_feature(f)? {
            out.push(converted);
        }
    }
    Ok(out)
}

/// Build the flat `LayerPayload` JSON for a run directory
/// (`runs/<execution_id>/` containing `payload.json` + `result.json`).
fn build_layer_payload(run_dir: &Path, layer_name: Option<&str>) -> Result<(String, Value)> {
    let result: Value = serde_json::from_str(
        &std::fs::read_to_string(run_dir.join("result.json"))
            .with_context(|| format!("read {}/result.json", run_dir.display()))?,
    )
    .context("parse result.json")?;
    let payload: Value = serde_json::from_str(
        &std::fs::read_to_string(run_dir.join("payload.json"))
            .with_context(|| format!("read {}/payload.json", run_dir.display()))?,
    )
    .context("parse payload.json")?;

    let execution_id = result["execution_id"]
        .as_str()
        .context("result.execution_id missing")?
        .to_string();
    match result["status"].as_str() {
        Some("success") | Some("warning") => {}
        other => bail!("cannot publish run with status {other:?}"),
    }

    // Prefer the geojson artifact; fall back to any artifact whose path ends
    // in .geojson.
    let artifacts = result["artifacts"]
        .as_object()
        .context("artifacts not an object")?;
    let geojson_path = artifacts
        .values()
        .find(|a| a["format"] == "application/geo+json")
        .or_else(|| {
            artifacts.values().find(|a| {
                a["file_path"]
                    .as_str()
                    .is_some_and(|p| p.ends_with(".geojson"))
            })
        })
        .and_then(|a| a["file_path"].as_str())
        .map(PathBuf::from)
        .context("result has no GeoJSON artifact to publish")?;
    let spatial_results = features_from_geojson(&geojson_path)?;

    let image_file = payload["target"]["image_path"]
        .as_str()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();
    let name = layer_name
        .map(str::to_string)
        .unwrap_or_else(|| format!("Connector: {image_file}"));

    let layer_id = layer_id_for(&execution_id);
    let metrics = result
        .get("metrics")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    // `LayerPayload.extra` is #[serde(flatten)] — extra keys sit at the top
    // level of the JSON file, not under an "extra" object.
    let mut layer = Map::new();
    layer.insert("layer_id".into(), Value::String(layer_id.clone()));
    layer.insert("layer_name".into(), Value::String(name));
    layer.insert(
        "display_preference".into(),
        serde_json::json!({
            "type": "point",
            "color": LAYER_COLOR,
            "icon": LAYER_ICON,
            "fields": [
                { "key": "tree_id", "label": "Tree ID", "visible": true, "custom": false, "kind": "text" },
                { "key": "confidence", "label": "Confidence", "visible": true, "custom": false, "kind": "number" }
            ]
        }),
    );
    layer.insert("spatial_results".into(), Value::Array(spatial_results));
    layer.insert("updated_at".into(), Value::String(iso_now()));
    layer.insert("metrics".into(), metrics);
    layer.insert(
        "plugin_execution_id".into(),
        Value::String(execution_id.clone()),
    );
    layer.insert("image_file".into(), Value::String(image_file));
    Ok((layer_id, Value::Object(layer)))
}

/// Write the flat layer file + the `layers/` visibility registration.
pub fn publish(
    run_dir: &Path,
    storage_root: &Path,
    layer_name: Option<&str>,
) -> Result<PublishOutcome> {
    let (layer_id, layer) = build_layer_payload(run_dir, layer_name)?;
    let feature_count = layer["spatial_results"].as_array().map_or(0, Vec::len);

    let sr_dir = storage_root.join("spatial_results");
    std::fs::create_dir_all(&sr_dir).with_context(|| format!("create {}", sr_dir.display()))?;
    let layer_path = sr_dir.join(format!("{layer_id}.json"));
    std::fs::write(&layer_path, serde_json::to_string_pretty(&layer)?)
        .with_context(|| format!("write {}", layer_path.display()))?;

    let layers_dir = storage_root.join("layers");
    std::fs::create_dir_all(&layers_dir)
        .with_context(|| format!("create {}", layers_dir.display()))?;
    let layer_name_owned = layer["layer_name"]
        .as_str()
        .unwrap_or(&layer_id)
        .to_string();
    let state = serde_json::json!({
        "layer_id": layer_id,
        "layer_name": layer_name_owned,
        "category": "point",
        "is_visible": true,
        "opacity": 1.0,
        "z_index": 10,
        "source_type": "spatial_result",
        "source_id": layer_id,
    });
    let state_path = layers_dir.join(format!("{layer_id}.json"));
    std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)
        .with_context(|| format!("write {}", state_path.display()))?;

    Ok(PublishOutcome {
        layer_id,
        layer_path,
        feature_count,
    })
}
