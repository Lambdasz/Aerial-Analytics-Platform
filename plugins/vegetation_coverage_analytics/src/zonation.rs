//! NxN Spatial grid zonation and GeoJSON feature collection assembly (Module 5 §4).
//!
//! Pure spatial partitioning and GeoJSON conversion.

use serde_json::{json, Value};

use crate::coverage::classify_coverage;
use crate::types::{BinaryMask, Coverage, GridCell, PluginError, Result};

/// Splits the mask into `grid_size` × `grid_size` cells and counts coverage in each.
/// Cell boundaries use integer arithmetic, so uneven dimensions never lose pixels. Pure.
pub fn generate_zonation(
    mask: &BinaryMask,
    keep: Option<&[bool]>,
    grid_size: usize,
) -> Result<Vec<GridCell>> {
    if grid_size == 0 || grid_size > 64 {
        return Err(PluginError::Value(format!(
            "grid_size must be between 1 and 64, got {grid_size}"
        )));
    }
    if mask.width == 0 || mask.height == 0 {
        return Err(PluginError::Value("mask has no pixels".into()));
    }

    let bounds = |total: usize, part: usize, index: usize| -> (usize, usize) {
        let start = index * total / part;
        let end = (index + 1) * total / part;
        (start, end)
    };

    let mut cells = Vec::with_capacity(grid_size * grid_size);
    for row in 0..grid_size {
        let (y0, y1) = bounds(mask.height, grid_size, row);
        for col in 0..grid_size {
            let (x0, x1) = bounds(mask.width, grid_size, col);
            let (mut total, mut vegetation) = (0usize, 0usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * mask.width + x;
                    if let Some(selector) = keep {
                        if !selector.get(i).copied().unwrap_or(false) {
                            continue;
                        }
                    }
                    total += 1;
                    vegetation += mask.at(x, y) as usize;
                }
            }
            cells.push(GridCell {
                row,
                col,
                x0,
                y0,
                x1,
                y1,
                coverage: Coverage {
                    total_pixels: total,
                    vegetation_pixels: vegetation,
                    coverage_percentage: if total == 0 {
                        0.0
                    } else {
                        (vegetation as f64 / total as f64) * 100.0
                    },
                },
            });
        }
    }
    Ok(cells)
}

/// Pixel window of a cell projected into lon/lat. Pure.
pub fn cell_ring(cell: &GridCell, bounds: [f64; 4], width: usize, height: usize) -> Vec<Vec<f64>> {
    let [min_lon, min_lat, max_lon, max_lat] = bounds;
    let lon_at = |x: usize| min_lon + (max_lon - min_lon) * (x as f64 / width as f64);
    let lat_at = |y: usize| max_lat - (max_lat - min_lat) * (y as f64 / height as f64);
    let corners = [
        (lon_at(cell.x0), lat_at(cell.y0)),
        (lon_at(cell.x1), lat_at(cell.y0)),
        (lon_at(cell.x1), lat_at(cell.y1)),
        (lon_at(cell.x0), lat_at(cell.y1)),
        (lon_at(cell.x0), lat_at(cell.y0)),
    ];
    corners.iter().map(|&(lon, lat)| vec![lon, lat]).collect()
}

/// Zone grid as a GeoJSON FeatureCollection with per-cell attributes. Pure.
pub fn zonation_geojson(
    cells: &[GridCell],
    bounds: [f64; 4],
    width: usize,
    height: usize,
    low_threshold: f64,
    medium_threshold: f64,
) -> Result<Value> {
    let features: Vec<Value> = cells
        .iter()
        .map(|cell| {
            let class = classify_coverage(
                cell.coverage.coverage_percentage,
                low_threshold,
                medium_threshold,
            )
            .unwrap_or_else(|_| "unknown".into());
            json!({
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [cell_ring(cell, bounds, width, height)]
                },
                "properties": {
                    "cell_id": format!("r{}c{}", cell.row + 1, cell.col + 1),
                    "row": cell.row,
                    "col": cell.col,
                    "total_pixels": cell.coverage.total_pixels,
                    "vegetation_pixels": cell.coverage.vegetation_pixels,
                    "coverage_percentage": round6(cell.coverage.coverage_percentage),
                    "classification": class,
                }
            })
        })
        .collect();
    Ok(json!({ "type": "FeatureCollection", "features": features }))
}

/// Rounds a float to 6 decimal places for stable JSON output. Pure.
pub fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_from(width: usize, height: usize, data: Vec<u8>) -> BinaryMask {
        BinaryMask {
            width,
            height,
            data,
        }
    }

    #[test]
    fn zonation_covers_every_pixel_once() {
        let mask = mask_from(7, 5, vec![1; 35]);
        let cells = generate_zonation(&mask, None, 3).unwrap();
        assert_eq!(cells.len(), 9);
        let total: usize = cells.iter().map(|c| c.coverage.total_pixels).sum();
        assert_eq!(total, 35);
        assert!(cells
            .iter()
            .all(|c| (c.coverage.coverage_percentage - 100.0).abs() < 1e-9));
    }

    #[test]
    fn zonation_rejects_bad_grid_size() {
        let mask = mask_from(4, 4, vec![0; 16]);
        assert!(generate_zonation(&mask, None, 0).is_err());
        assert!(generate_zonation(&mask, None, 65).is_err());
    }

    #[test]
    fn zonation_geojson_is_a_feature_collection() {
        let mask = mask_from(4, 4, vec![1; 16]);
        let cells = generate_zonation(&mask, None, 2).unwrap();
        let geojson = zonation_geojson(&cells, [0.0, 0.0, 4.0, 4.0], 4, 4, 20.0, 50.0).unwrap();
        assert_eq!(geojson["type"], "FeatureCollection");
        let features = geojson["features"].as_array().unwrap();
        assert_eq!(features.len(), 4);
        assert_eq!(features[0]["geometry"]["type"], "Polygon");
        assert_eq!(features[0]["properties"]["coverage_percentage"], 100.0);
    }
}
