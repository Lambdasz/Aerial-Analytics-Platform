//! GeoJSON plot import and validation (Module 9).
//!
//! Parses `Feature` or `FeatureCollection` GeoJSON text into [`Plot`]
//! structures. Only `Polygon` geometries are accepted; unsupported or
//! invalid features are reported in [`ImportResult::skipped`] with the
//! rejection reason.
//!
//! ## Validation Rules
//!
//! - Each ring must contain at least 4 positions (including the closing point).
//! - The first and last positions of every ring must be identical (closed ring).
//! - Coordinates must be in `[longitude, latitude]` order (RFC 7946).
//! - Longitude must be in `−180..=180`, latitude in `−90..=90`.
//!
//! ## Unit Tests
//!
//! This module includes tests for valid polygons, default plot names,
//! unclosed rings, swapped coordinates, and non-polygon geometries.

use geojson::{Feature, GeoJson, GeometryValue, Position};
use serde::Serialize;
use uuid::Uuid;

const CRS_WGS84: &str = "EPSG:4326";

#[derive(Serialize, Clone, Debug)]
pub struct Plot {
    pub plot_id: String,
    pub plot_name: String,
    pub crs: String,
    pub rings: Vec<Vec<[f64; 2]>>,
}

#[derive(Serialize, Debug)]
pub struct ImportResult {
    pub plots: Vec<Plot>,
    pub skipped: Vec<String>,
}

/// Parses GeoJSON text (Feature or FeatureCollection) into plots.
///
/// Only Polygon geometries are imported. Features that cannot be imported are
/// reported in `skipped` with the reason; an error is returned only when the
/// text is not valid GeoJSON or no valid plot remains.
pub fn parse_plots(text: &str) -> Result<ImportResult, String> {
    let features =
        match text.parse::<GeoJson>() {
            Ok(GeoJson::FeatureCollection(collection)) => collection.features,
            Ok(GeoJson::Feature(feature)) => vec![feature],
            Ok(GeoJson::Geometry(_)) => return Err(
                "File contains bare geometry without a Feature. Wrap the geometry in a Feature or \
                 FeatureCollection so the plot name can be read."
                    .to_string(),
            ),
            Err(e) => return Err(format!("Not valid GeoJSON: {e}")),
        };

    let (plots, skipped) = features.iter().enumerate().fold(
        (Vec::new(), Vec::new()),
        |(mut plots, mut skipped), (index, feature)| {
            match feature_to_plot(index + 1, feature) {
                Ok(plot) => plots.push(plot),
                Err(reason) => skipped.push(reason),
            }
            (plots, skipped)
        },
    );

    if plots.is_empty() {
        let detail = if skipped.is_empty() {
            "file contains no features.".to_string()
        } else {
            skipped.join(" ")
        };
        return Err(format!("No valid plots to import: {detail}"));
    }

    Ok(ImportResult { plots, skipped })
}

fn feature_to_plot(number: usize, feature: &Feature) -> Result<Plot, String> {
    let label = format!("Feature #{number} skipped:");

    let geometry = feature
        .geometry
        .as_ref()
        .ok_or_else(|| format!("{label} has no geometry."))?;

    let raw_rings = match &geometry.value {
        GeometryValue::Polygon { coordinates } => coordinates,
        other => {
            return Err(format!(
                "{label} geometry type '{}' is not supported, only Polygon is accepted.",
                other.type_name()
            ))
        }
    };

    let rings = validate_rings(raw_rings).map_err(|reason| format!("{label} {reason}"))?;

    Ok(Plot {
        plot_id: Uuid::new_v4().to_string(),
        plot_name: plot_name(number, feature),
        crs: CRS_WGS84.to_string(),
        rings,
    })
}

fn plot_name(number: usize, feature: &Feature) -> String {
    feature
        .properties
        .as_ref()
        .and_then(|properties| properties.get("name"))
        .and_then(|name| name.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(|| format!("Plot {number}"), str::to_string)
}

fn validate_rings(rings: &[Vec<Position>]) -> Result<Vec<Vec<[f64; 2]>>, String> {
    if rings.is_empty() {
        return Err("Polygon has no rings.".to_string());
    }
    rings
        .iter()
        .enumerate()
        .map(|(index, ring)| {
            let name = if index == 0 {
                "outer ring".to_string()
            } else {
                format!("hole #{index}")
            };
            validate_ring(&name, ring)
        })
        .collect()
}

fn validate_ring(name: &str, ring: &[Position]) -> Result<Vec<[f64; 2]>, String> {
    if ring.len() < 4 {
        return Err(format!(
            "{name} has only {} points, minimum 4 required (including closing point).",
            ring.len()
        ));
    }
    if ring.first() != ring.last() {
        return Err(format!(
            "{name} is not closed: first point must equal last point."
        ));
    }
    ring.iter()
        .enumerate()
        .map(|(index, position)| to_point(name, index + 1, position.as_slice()))
        .collect()
}

fn to_point(ring_name: &str, number: usize, position: &[f64]) -> Result<[f64; 2], String> {
    let [lon, lat] = match position {
        [lon, lat, ..] => [*lon, *lat],
        _ => {
            return Err(format!(
                "Point #{number} on {ring_name} must have [longitude, latitude] coordinates."
            ))
        }
    };
    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!(
            "Point #{number} on {ring_name} has longitude {lon} outside range -180..180. \
             GeoJSON coordinate order is [longitude, latitude]; the two values may be swapped."
        ));
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!(
            "Point #{number} on {ring_name} has latitude {lat} outside range -90..90. \
             GeoJSON coordinate order is [longitude, latitude]; the two values may be swapped."
        ));
    }
    Ok([lon, lat])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str =
        "[[116.80,-1.20],[116.81,-1.20],[116.81,-1.21],[116.80,-1.21],[116.80,-1.20]]";

    fn feature(properties: &str, geometry_type: &str, coordinates: &str) -> String {
        format!(
            r#"{{"type":"Feature","properties":{properties},"geometry":{{"type":"{geometry_type}","coordinates":{coordinates}}}}}"#
        )
    }

    fn collection(features: &[String]) -> String {
        format!(
            r#"{{"type":"FeatureCollection","features":[{}]}}"#,
            features.join(",")
        )
    }

    #[test]
    fn reads_valid_polygon() {
        let text = feature(r#"{"name":"BLOK_A1"}"#, "Polygon", &format!("[{SQUARE}]"));
        let result = parse_plots(&text).unwrap();

        assert_eq!(result.plots.len(), 1);
        assert!(result.skipped.is_empty());
        let plot = &result.plots[0];
        assert_eq!(plot.plot_name, "BLOK_A1");
        assert_eq!(plot.crs, "EPSG:4326");
        assert_eq!(plot.rings.len(), 1);
        assert_eq!(plot.rings[0].len(), 5);
        assert_eq!(plot.rings[0][0], [116.80, -1.20]);
    }

    #[test]
    fn each_plot_gets_distinct_uuid() {
        let one = feature("{}", "Polygon", &format!("[{SQUARE}]"));
        let text = collection(&[one.clone(), one]);
        let result = parse_plots(&text).unwrap();

        let (a, b) = (&result.plots[0].plot_id, &result.plots[1].plot_id);
        assert_ne!(a, b);
        assert!(Uuid::parse_str(a).is_ok() && Uuid::parse_str(b).is_ok());
    }

    #[test]
    fn plot_without_name_gets_default_name() {
        let named = feature(r#"{"name":"Sawah"}"#, "Polygon", &format!("[{SQUARE}]"));
        let unnamed = feature("null", "Polygon", &format!("[{SQUARE}]"));
        let result = parse_plots(&collection(&[named, unnamed])).unwrap();

        assert_eq!(result.plots[0].plot_name, "Sawah");
        assert_eq!(result.plots[1].plot_name, "Plot 2");
    }

    #[test]
    fn rejects_unclosed_ring() {
        let open = "[[116.80,-1.20],[116.81,-1.20],[116.81,-1.21],[116.80,-1.21]]";
        let text = feature("{}", "Polygon", &format!("[{open}]"));
        let error = parse_plots(&text).unwrap_err();

        assert!(error.contains("not closed"), "{error}");
    }

    #[test]
    fn rejects_swapped_coordinates() {
        let swapped =
            "[[-1.20,116.80],[-1.20,116.81],[-1.21,116.81],[-1.21,116.80],[-1.20,116.80]]";
        let text = feature("{}", "Polygon", &format!("[{swapped}]"));
        let error = parse_plots(&text).unwrap_err();

        assert!(error.contains("latitude"), "{error}");
        assert!(error.contains("swapped"), "{error}");
    }

    #[test]
    fn rejects_non_json_text() {
        let error = parse_plots("not valid json").unwrap_err();

        assert!(error.contains("Not valid GeoJSON"), "{error}");
    }

    #[test]
    fn skips_non_polygon_geometry_without_error() {
        let point = feature("{}", "Point", "[116.80,-1.20]");
        let polygon = feature("{}", "Polygon", &format!("[{SQUARE}]"));
        let result = parse_plots(&collection(&[point, polygon])).unwrap();

        assert_eq!(result.plots.len(), 1);
        assert_eq!(result.skipped.len(), 1);
        assert!(result.skipped[0].contains("Point"), "{:?}", result.skipped);
    }

    #[test]
    fn errors_when_no_valid_plot_remains() {
        let point = feature("{}", "Point", "[116.80,-1.20]");
        let error = parse_plots(&collection(&[point])).unwrap_err();

        assert!(error.contains("No valid plots"), "{error}");
    }
}
