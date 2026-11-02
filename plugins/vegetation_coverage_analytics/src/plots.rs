//! Per-plot analysis, polygon masking, ranking, and comparison (Module 5 §5 & §6).
//!
//! Pure spatial clipping and comparative indicators.

use serde_json::{json, Value};

use crate::coverage::{area_metrics, calculate_coverage, classify_coverage};
use crate::geo::pixel_centers;
use crate::types::{BinaryMask, PlotComparison, PlotResult, PluginError, Result};
use crate::zonation::round6;

/// Ray-casting point-in-polygon test on a lon/lat ring. Pure.
pub fn point_in_polygon(lon: f64, lat: f64, ring: &[(f64, f64)]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        let straddles = (yi > lat) != (yj > lat);
        if straddles {
            let x_cross = (xj - xi) * (lat - yi) / (yj - yi) + xi;
            if lon < x_cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Pixel selector for a polygon, using pixel centers. Pure.
pub fn polygon_pixel_mask(
    bounds: [f64; 4],
    width: usize,
    height: usize,
    ring: &[(f64, f64)],
) -> Vec<bool> {
    let mut keep = vec![false; width * height];
    for (index, lon, lat) in pixel_centers(bounds, width, height) {
        keep[index] = point_in_polygon(lon, lat, ring);
    }
    keep
}

/// Coverage inside one plot polygon (Module 5 §5). Pure.
pub fn calculate_plot_coverage(
    mask: &BinaryMask,
    bounds: [f64; 4],
    plot_id: &str,
    ring: &[(f64, f64)],
    gsd_m: Option<f64>,
    low_threshold: f64,
    medium_threshold: f64,
) -> Result<PlotResult> {
    if ring.len() < 3 {
        return Err(PluginError::Value(format!(
            "plot '{plot_id}' polygon needs at least 3 vertices"
        )));
    }
    let keep = polygon_pixel_mask(bounds, mask.width, mask.height, ring);
    let coverage = calculate_coverage(mask, Some(&keep));
    if coverage.total_pixels == 0 {
        return Err(PluginError::Value(format!(
            "plot '{plot_id}' does not overlap the image extent"
        )));
    }
    Ok(PlotResult {
        plot_id: plot_id.to_string(),
        coverage,
        vegetation_area_sqm: area_metrics(&coverage, gsd_m).0,
        classification: classify_coverage(
            coverage.coverage_percentage,
            low_threshold,
            medium_threshold,
        )?,
    })
}

/// Average / extremes / ranking across plots (Module 5 §6). Pure.
pub fn compare_plots(plots: &[PlotResult]) -> PlotComparison {
    let mut ranked: Vec<&PlotResult> = plots.iter().collect();
    ranked.sort_by(|a, b| {
        b.coverage
            .coverage_percentage
            .partial_cmp(&a.coverage.coverage_percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let average = if ranked.is_empty() {
        None
    } else {
        let sum: f64 = ranked.iter().map(|p| p.coverage.coverage_percentage).sum();
        Some(sum / ranked.len() as f64)
    };
    PlotComparison {
        average_coverage: average,
        highest_plot_id: ranked.first().map(|p| p.plot_id.clone()),
        lowest_plot_id: ranked.last().map(|p| p.plot_id.clone()),
        ranking: ranked.iter().map(|p| p.plot_id.clone()).collect(),
    }
}

/// Plot results as a GeoJSON FeatureCollection. Pure.
pub fn plots_geojson(plots: &[PlotResult], rings: &[(String, Vec<(f64, f64)>)]) -> Value {
    let features: Vec<Value> = plots
        .iter()
        .map(|plot| {
            let ring = rings
                .iter()
                .find(|(id, _)| id == &plot.plot_id)
                .map(|(_, ring)| ring.clone())
                .unwrap_or_default();
            let mut coords: Vec<Vec<f64>> = ring.iter().map(|&(lon, lat)| vec![lon, lat]).collect();
            let closed = coords
                .first()
                .zip(coords.last())
                .is_some_and(|(f, l)| f == l);
            if !closed && !coords.is_empty() {
                coords.push(coords[0].clone());
            }
            json!({
                "type": "Feature",
                "geometry": { "type": "Polygon", "coordinates": [coords] },
                "properties": {
                    "plot_id": plot.plot_id,
                    "total_pixels": plot.coverage.total_pixels,
                    "vegetation_pixels": plot.coverage.vegetation_pixels,
                    "coverage_percentage": round6(plot.coverage.coverage_percentage),
                    "vegetation_area_sqm": plot.vegetation_area_sqm.map(round6),
                    "classification": plot.classification,
                }
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

/// One plot description parsed from the `plots` parameter.
pub struct PlotRequest {
    pub plot_id: String,
    pub ring: Vec<(f64, f64)>,
}

/// Parses the `plots` parameter: a JSON array of `{plot_id, polygon}`. Pure.
pub fn parse_plots(parameters: &Value) -> std::result::Result<Vec<PlotRequest>, String> {
    let raw = &parameters["plots"];
    let parsed = match raw {
        Value::Array(items) => Value::Array(items.clone()),
        Value::String(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .map_err(|e| format!("plots parameter is not valid JSON: {e}"))?,
        _ => return Ok(Vec::new()),
    };
    let items = parsed
        .as_array()
        .ok_or_else(|| "plots parameter must be a JSON array".to_string())?;
    let mut requests = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let plot_id = item["plot_id"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("plot_{}", index + 1));
        let polygon = item["polygon"]
            .as_array()
            .ok_or_else(|| format!("plot '{plot_id}' is missing a polygon array"))?;
        let ring: Vec<(f64, f64)> = polygon
            .iter()
            .filter_map(|point| {
                let pair = point.as_array()?;
                Some((pair.first()?.as_f64()?, pair.get(1)?.as_f64()?))
            })
            .collect();
        if ring.len() < 3 {
            return Err(format!(
                "plot '{plot_id}' polygon needs at least 3 lon/lat pairs"
            ));
        }
        requests.push(PlotRequest { plot_id, ring });
    }
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Coverage;

    fn mask_from(width: usize, height: usize, data: Vec<u8>) -> BinaryMask {
        BinaryMask {
            width,
            height,
            data,
        }
    }

    #[test]
    fn point_in_polygon_handles_ring_and_outside() {
        let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        assert!(point_in_polygon(1.0, 1.0, &square));
        assert!(!point_in_polygon(3.0, 1.0, &square));
        assert!(!point_in_polygon(1.0, 1.0, &[(0.0, 0.0), (1.0, 1.0)]));
    }

    #[test]
    fn plot_coverage_uses_polygon_area() {
        // 4x4 mask, left half vegetation, right half bare.
        let data: Vec<u8> = (0..16).map(|i| u8::from(i % 4 < 2)).collect();
        let mask = mask_from(4, 4, data);
        let bounds = [0.0, 0.0, 4.0, 4.0];
        let left = [(0.0, 0.0), (2.0, 0.0), (2.0, 4.0), (0.0, 4.0)];
        let result =
            calculate_plot_coverage(&mask, bounds, "left", &left, Some(1.0), 20.0, 50.0).unwrap();
        assert_eq!(result.coverage.total_pixels, 8);
        assert_eq!(result.coverage.vegetation_pixels, 8);
        assert_eq!(result.classification, "high");
        assert_eq!(result.vegetation_area_sqm, Some(8.0));
    }

    #[test]
    fn plot_outside_extent_is_rejected() {
        let mask = mask_from(2, 2, vec![0; 4]);
        let bounds = [0.0, 0.0, 2.0, 2.0];
        let far = [(10.0, 10.0), (11.0, 10.0), (11.0, 11.0), (10.0, 11.0)];
        assert!(calculate_plot_coverage(&mask, bounds, "far", &far, None, 20.0, 50.0).is_err());
    }

    #[test]
    fn compare_plots_ranks_descending() {
        let plot = |id: &str, pct: f64| PlotResult {
            plot_id: id.into(),
            coverage: Coverage {
                total_pixels: 100,
                vegetation_pixels: pct as usize,
                coverage_percentage: pct,
            },
            vegetation_area_sqm: None,
            classification: "low".into(),
        };
        let comparison = compare_plots(&[plot("a", 10.0), plot("b", 60.0), plot("c", 30.0)]);
        assert_eq!(comparison.highest_plot_id.as_deref(), Some("b"));
        assert_eq!(comparison.lowest_plot_id.as_deref(), Some("a"));
        assert_eq!(comparison.ranking, vec!["b", "c", "a"]);
        assert!((comparison.average_coverage.unwrap() - 100.0 / 3.0).abs() < 1e-9);
        assert_eq!(compare_plots(&[]).average_coverage, None);
    }

    #[test]
    fn plots_parameter_parses_and_validates() {
        let parameters =
            json!({ "plots": "[{\"plot_id\":\"A\",\"polygon\":[[0,0],[1,0],[1,1]]}]" });
        let requests = parse_plots(&parameters).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].plot_id, "A");

        let broken = json!({ "plots": "[{\"plot_id\":\"A\",\"polygon\":[[0,0]]}]" });
        assert!(parse_plots(&broken).is_err());
        assert!(parse_plots(&json!({ "plots": "" })).unwrap().is_empty());
    }
}
