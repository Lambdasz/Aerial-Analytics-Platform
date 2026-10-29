#![allow(dead_code)]

//! Spatial change detection (Module 10.7) between two observation dates.
//!
//! Both observations are reduced to a vegetation mask with the same excess
//! green rule the coverage measurement uses; the `after` mask is resampled onto
//! the `before` grid over the intersection of their footprints, and the pixels
//! that changed class become change regions.
//!
//! Deliberately simple: nearest-neighbour resampling, 4-neighbour connected
//! components and axis-aligned bounding boxes as polygons. That is honest for
//! "simple temporal change analysis" and keeps the result explainable, but it
//! is not orthorectification — flights with very different altitude or heading
//! will show alignment error.

use std::path::Path;

use serde::Serialize;
use serde_json::{json, Map, Value};
use thiserror::Error;

use super::analysis::ImageObservation;
use super::coverage::{is_vegetation, MAX_ANALYSIS_EDGE};

const METERS_PER_DEGREE_LAT: f64 = 111_320.0;
const ASSUMED_HFOV_DEG: f64 = 60.0;

/// Errors from spatial change detection.
#[derive(Debug, Error)]
pub enum SpatialChangeError {
    #[error("gagal membaca citra '{path}': {source}")]
    ReadImage {
        path: String,
        #[source]
        source: image::ImageError,
    },
    #[error("citra tidak memiliki piksel")]
    EmptyImage,
    #[error("observasi tidak memiliki GPS dan/atau altitude, tidak bisa dipetakan")]
    MissingGeoreference,
    #[error("kedua citra tidak saling bertampalan (bounds tidak beririsan)")]
    NoOverlap,
}

/// A vegetation mask over a WGS 84 rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct VegetationMask {
    pub width: usize,
    pub height: usize,
    /// `[min_lon, min_lat, max_lon, max_lat]`.
    pub bounds: [f64; 4],
    /// Row-major, 0 = empty, 1 = vegetation.
    pub data: Vec<u8>,
}

/// Pixels that changed between the two masks over the shared footprint.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeGrid {
    pub width: usize,
    pub height: usize,
    pub bounds: [f64; 4],
    /// 1 where vegetation appeared.
    pub gain: Vec<u8>,
    /// 1 where vegetation disappeared.
    pub loss: Vec<u8>,
    pub gain_pixels: usize,
    pub loss_pixels: usize,
    /// Total pixels in the shared footprint.
    pub total_pixels: usize,
}

/// One connected change region, polygonalised as its bounding box.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeFeature {
    /// `gain` (vegetation appeared) or `loss` (vegetation disappeared).
    pub change_type: String,
    pub pixels: usize,
    pub area_m2: f64,
    /// `[min_lon, min_lat, max_lon, max_lat]`.
    pub bbox: [f64; 4],
}

/// Everything the page needs after one spatial comparison.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialChangeReport {
    pub before_bounds: [f64; 4],
    pub after_bounds: [f64; 4],
    pub change_bounds: [f64; 4],
    pub grid_width: usize,
    pub grid_height: usize,
    pub gain_area_m2: f64,
    pub loss_area_m2: f64,
    pub changed_fraction_percent: f64,
    pub magnitude: f64,
    pub magnitude_level: String,
    pub features: Vec<ChangeFeature>,
    pub geojson: Value,
    pub layer_id: String,
    pub layer_name: String,
}

/// Footprint of an observation from its GPS point, altitude and pixel size.
/// Pure.
pub fn bounds_for_observation(observation: &ImageObservation) -> Option<[f64; 4]> {
    let lat = observation.latitude?;
    let lon = observation.longitude?;
    let altitude = observation.altitude_m.filter(|value| *value > 0.0)?;
    let width = observation.width.filter(|value| *value > 0)?;
    let height = observation.height.filter(|value| *value > 0)?;

    let ground_width = 2.0 * altitude * (ASSUMED_HFOV_DEG / 2.0).to_radians().tan();
    let ground_height = ground_width * (height as f64 / width as f64);
    let delta_lat = (ground_height / 2.0) / METERS_PER_DEGREE_LAT;
    let delta_lon = (ground_width / 2.0) / (METERS_PER_DEGREE_LAT * lat.to_radians().cos());
    Some([
        lon - delta_lon,
        lat - delta_lat,
        lon + delta_lon,
        lat + delta_lat,
    ])
}

/// Builds a mask from an RGB buffer over the given bounds. Pure.
pub fn mask_from_rgb(rgb: &[u8], width: usize, height: usize, bounds: [f64; 4]) -> VegetationMask {
    let data = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|pixel| u8::from(is_vegetation(pixel[0], pixel[1], pixel[2])))
        .collect();
    VegetationMask {
        width,
        height,
        bounds,
        data,
    }
}

/// Decodes an image into a downscaled vegetation mask. I/O edge.
pub fn mask_from_image(
    path: &Path,
    bounds: [f64; 4],
) -> Result<VegetationMask, SpatialChangeError> {
    let image = image::open(path).map_err(|source| SpatialChangeError::ReadImage {
        path: path.display().to_string(),
        source,
    })?;
    let image = if image.width() > MAX_ANALYSIS_EDGE || image.height() > MAX_ANALYSIS_EDGE {
        image.thumbnail(MAX_ANALYSIS_EDGE, MAX_ANALYSIS_EDGE)
    } else {
        image
    };
    let rgb = image.to_rgb8();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    if width == 0 || height == 0 {
        return Err(SpatialChangeError::EmptyImage);
    }
    Ok(mask_from_rgb(rgb.as_raw(), width, height, bounds))
}

/// Intersection of two footprints, or `None` when they do not overlap. Pure.
pub fn intersect_bounds(left: &[f64; 4], right: &[f64; 4]) -> Option<[f64; 4]> {
    let bounds = [
        left[0].max(right[0]),
        left[1].max(right[1]),
        left[2].min(right[2]),
        left[3].min(right[3]),
    ];
    (bounds[2] > bounds[0] && bounds[3] > bounds[1]).then_some(bounds)
}

/// Nearest-neighbour resample of a mask onto `target` bounds and grid. Pure.
pub fn resample_nearest(
    mask: &VegetationMask,
    target: [f64; 4],
    width: usize,
    height: usize,
) -> Vec<u8> {
    let mut data = vec![0u8; width * height];
    let source_lon_span = mask.bounds[2] - mask.bounds[0];
    let source_lat_span = mask.bounds[3] - mask.bounds[1];
    if source_lon_span <= 0.0 || source_lat_span <= 0.0 || width == 0 || height == 0 {
        return data;
    }
    for y in 0..height {
        let lat = target[3] - (target[3] - target[1]) * ((y as f64 + 0.5) / height as f64);
        let source_y = ((mask.bounds[3] - lat) / source_lat_span * mask.height as f64).floor();
        let source_y = (source_y as isize).clamp(0, mask.height as isize - 1) as usize;
        for x in 0..width {
            let lon = target[0] + (target[2] - target[0]) * ((x as f64 + 0.5) / width as f64);
            let source_x = ((lon - mask.bounds[0]) / source_lon_span * mask.width as f64).floor();
            let source_x = (source_x as isize).clamp(0, mask.width as isize - 1) as usize;
            data[y * width + x] = mask.data[source_y * mask.width + source_x];
        }
    }
    data
}

/// Compares two masks over their shared footprint. Pure.
pub fn diff_masks(
    before: &VegetationMask,
    after: &VegetationMask,
) -> Result<ChangeGrid, SpatialChangeError> {
    let bounds =
        intersect_bounds(&before.bounds, &after.bounds).ok_or(SpatialChangeError::NoOverlap)?;
    let width = before.width.min(after.width).max(1);
    let height = before.height.min(after.height).max(1);
    let before_data = resample_nearest(before, bounds, width, height);
    let after_data = resample_nearest(after, bounds, width, height);

    let mut gain = vec![0u8; width * height];
    let mut loss = vec![0u8; width * height];
    let (mut gain_pixels, mut loss_pixels) = (0usize, 0usize);
    for index in 0..width * height {
        match (before_data[index], after_data[index]) {
            (0, 1) => {
                gain[index] = 1;
                gain_pixels += 1;
            }
            (1, 0) => {
                loss[index] = 1;
                loss_pixels += 1;
            }
            _ => {}
        }
    }
    Ok(ChangeGrid {
        width,
        height,
        bounds,
        gain,
        loss,
        gain_pixels,
        loss_pixels,
        total_pixels: width * height,
    })
}

/// Area of one grid pixel in m² over the grid bounds. Pure.
pub fn pixel_area_m2(bounds: [f64; 4], width: usize, height: usize) -> f64 {
    if width == 0 || height == 0 {
        return 0.0;
    }
    let center_lat = (bounds[1] + bounds[3]) / 2.0;
    let width_m = (bounds[2] - bounds[0]) * METERS_PER_DEGREE_LAT * center_lat.to_radians().cos();
    let height_m = (bounds[3] - bounds[1]) * METERS_PER_DEGREE_LAT;
    (width_m * height_m) / (width as f64 * height as f64)
}

/// Longitude/latitude of a pixel edge. Pure.
fn lon_at(bounds: [f64; 4], x: usize, width: usize) -> f64 {
    bounds[0] + (bounds[2] - bounds[0]) * (x as f64 / width as f64)
}

/// Latitude of a pixel edge (row 0 is the north edge). Pure.
fn lat_at(bounds: [f64; 4], y: usize, height: usize) -> f64 {
    bounds[3] - (bounds[3] - bounds[1]) * (y as f64 / height as f64)
}

/// 4-neighbour connected components of one change class, filtered by area. Pure.
pub fn connected_features(
    grid: &ChangeGrid,
    class: &[u8],
    change_type: &str,
    min_area_m2: f64,
) -> Vec<ChangeFeature> {
    let (width, height) = (grid.width, grid.height);
    let pixel_area = pixel_area_m2(grid.bounds, width, height);
    let mut visited = vec![false; width * height];
    let mut features = Vec::new();

    for start in 0..width * height {
        if visited[start] || class[start] == 0 {
            continue;
        }
        let mut queue = vec![start];
        visited[start] = true;
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (width, 0, height, 0);
        let mut pixels = 0usize;
        while let Some(index) = queue.pop() {
            let x = index % width;
            let y = index / width;
            pixels += 1;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            let neighbours = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in neighbours {
                if nx >= width || ny >= height {
                    continue;
                }
                let neighbour = ny * width + nx;
                if !visited[neighbour] && class[neighbour] == 1 {
                    visited[neighbour] = true;
                    queue.push(neighbour);
                }
            }
        }
        let area_m2 = pixels as f64 * pixel_area;
        if area_m2 < min_area_m2 {
            continue;
        }
        features.push(ChangeFeature {
            change_type: change_type.to_string(),
            pixels,
            area_m2,
            bbox: [
                lon_at(grid.bounds, min_x, width),
                lat_at(grid.bounds, max_y + 1, height),
                lon_at(grid.bounds, max_x + 1, width),
                lat_at(grid.bounds, min_y, height),
            ],
        });
    }
    features
}

/// GeoJSON FeatureCollection of the change regions. Pure.
pub fn features_geojson(features: &[ChangeFeature]) -> Value {
    let items: Vec<Value> = features
        .iter()
        .map(|feature| {
            let [min_lon, min_lat, max_lon, max_lat] = feature.bbox;
            let mut properties = Map::new();
            properties.insert("change_type".into(), json!(feature.change_type));
            properties.insert("pixels".into(), json!(feature.pixels));
            properties.insert("area_m2".into(), json!(feature.area_m2.round()));
            json!({
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [[
                        [min_lon, min_lat],
                        [max_lon, min_lat],
                        [max_lon, max_lat],
                        [min_lon, max_lat],
                        [min_lon, min_lat],
                    ]]
                },
                "properties": properties,
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": items })
}

/// Builds the report from two masks, the magnitude level supplied by the caller. Pure.
pub fn build_report(
    before: &VegetationMask,
    after: &VegetationMask,
    min_area_m2: f64,
    magnitude_level: String,
    layer_id: String,
    layer_name: String,
) -> Result<SpatialChangeReport, SpatialChangeError> {
    let grid = diff_masks(before, after)?;
    let pixel_area = pixel_area_m2(grid.bounds, grid.width, grid.height);
    let mut features = connected_features(&grid, &grid.gain, "gain", min_area_m2);
    features.extend(connected_features(&grid, &grid.loss, "loss", min_area_m2));

    let gain_area_m2 = grid.gain_pixels as f64 * pixel_area;
    let loss_area_m2 = grid.loss_pixels as f64 * pixel_area;
    let changed_fraction_percent = if grid.total_pixels == 0 {
        0.0
    } else {
        ((grid.gain_pixels + grid.loss_pixels) as f64 / grid.total_pixels as f64) * 100.0
    };
    let geojson = features_geojson(&features);

    Ok(SpatialChangeReport {
        before_bounds: before.bounds,
        after_bounds: after.bounds,
        change_bounds: grid.bounds,
        grid_width: grid.width,
        grid_height: grid.height,
        gain_area_m2,
        loss_area_m2,
        changed_fraction_percent,
        magnitude: changed_fraction_percent,
        magnitude_level,
        features,
        geojson,
        layer_id,
        layer_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(
        width: usize,
        height: usize,
        bounds: [f64; 4],
        veg: &[(usize, usize)],
    ) -> VegetationMask {
        let mut data = vec![0u8; width * height];
        for &(x, y) in veg {
            data[y * width + x] = 1;
        }
        VegetationMask {
            width,
            height,
            bounds,
            data,
        }
    }

    const GRID: [f64; 4] = [0.0, 0.0, 0.004, 0.004];

    #[test]
    fn intersection_and_resampling_follow_bounds() {
        let left = [0.0, 0.0, 0.002, 0.002];
        let right = [0.001, 0.001, 0.003, 0.003];
        let overlap = intersect_bounds(&left, &right).unwrap();
        assert_eq!(overlap, [0.001, 0.001, 0.002, 0.002]);
        assert!(intersect_bounds(&[0.0, 0.0, 1.0, 1.0], &[2.0, 2.0, 3.0, 3.0]).is_none());

        // A mask fully vegetation stays vegetation after resampling.
        let full: Vec<(usize, usize)> = (0..4).flat_map(|y| (0..4).map(move |x| (x, y))).collect();
        let source = mask(4, 4, GRID, &full);
        let resampled = resample_nearest(&source, GRID, 2, 2);
        assert_eq!(resampled, vec![1, 1, 1, 1]);
    }

    #[test]
    fn diff_marks_gain_and_loss() {
        let before = mask(4, 4, GRID, &[(0, 0), (1, 0), (0, 1), (1, 1)]);
        let after = mask(4, 4, GRID, &[(2, 0), (3, 0), (2, 1), (3, 1)]);
        let grid = diff_masks(&before, &after).unwrap();
        assert_eq!(grid.gain_pixels, 4);
        assert_eq!(grid.loss_pixels, 4);
        assert_eq!(grid.total_pixels, 16);
    }

    #[test]
    fn magnitude_fraction_counts_both_classes() {
        let before = mask(4, 4, GRID, &[(0, 0)]);
        let after = mask(4, 4, GRID, &[(1, 0)]);
        let report = build_report(
            &before,
            &after,
            // Small minimum so both single-pixel regions survive.
            0.0,
            "rendah".into(),
            "layer".into(),
            "Layer".into(),
        )
        .unwrap();
        assert!((report.changed_fraction_percent - 12.5).abs() < 1e-6);
        assert_eq!(report.features.len(), 2);
        assert!(report.gain_area_m2 > 0.0 && report.loss_area_m2 > 0.0);
        let geojson = &report.geojson;
        assert_eq!(geojson["type"], "FeatureCollection");
        assert_eq!(geojson["features"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn small_regions_are_filtered_by_minimum_area() {
        let before = mask(4, 4, GRID, &[(0, 0)]);
        let after = mask(4, 4, GRID, &[(1, 0)]);
        let report = build_report(
            &before,
            &after,
            20_000.0,
            "rendah".into(),
            "layer".into(),
            "Layer".into(),
        )
        .unwrap();
        assert!(report.features.is_empty());
        // Areas are still reported even when no polygon survives the filter.
        assert!(report.gain_area_m2 > 0.0);
    }

    #[test]
    fn bounds_need_gps_and_altitude() {
        let mut observation = ImageObservation::new(
            uuid::Uuid::new_v4(),
            "/tmp/x.jpg",
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        );
        assert!(bounds_for_observation(&observation).is_none());
        observation = observation.with_metadata(
            Some(29.76),
            Some(115.97),
            Some(400.0),
            Some(4000),
            Some(3000),
        );
        let bounds = bounds_for_observation(&observation).unwrap();
        assert!(bounds[2] > bounds[0] && bounds[3] > bounds[1]);
    }
}
