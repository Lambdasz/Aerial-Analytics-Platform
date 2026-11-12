//! Vegetation coverage percentage, classification, and surface area metrics (Module 5 §3).
//!
//! Pure functional aggregation on binary canopy masks.

use crate::types::{BinaryMask, Coverage, PluginError, Result};

/// Counts coverage over the whole mask, or only where `keep` is true (AOI). Pure function.
pub fn calculate_coverage(mask: &BinaryMask, keep: Option<&[bool]>) -> Coverage {
    let (total, vegetation) = mask //declaring total & vegetation var
        .data
        .iter()
        .enumerate()
        // filtering, apakah area yang dihitung adalah semua area?(keep.map_or(true)) atau hanya yang piksel yang bernilai true
        // misal di dalam plot (keep.map_or(|selector|))
        .filter(|&(i, _)| keep.is_none_or(|selector| selector.get(i).copied().unwrap_or(false))) // filter disini nge gantiin blok if let Some(selector) == keep {continue}
        .fold((0usize, 0usize), |(tot, veg), (_, &flag)| {
            (tot + 1, veg + flag as usize)
        });
    Coverage {
        total_pixels: total,
        vegetation_pixels: vegetation,
        coverage_percentage: if total == 0 {
            0.0
        } else {
            (vegetation as f64 / total as f64) * 100.0
        },
    }
}

// ========== OLD calculate_coverage code ============================================================
// Counts coverage over the whole mask, or only where `keep` is true (AOI). Pure function.
// pub fn calculate_coverage(mask: &BinaryMask, keep: Option<&[bool]>) -> Coverage {
//     let (mut total, mut vegetation) = (0usize, 0usize);
//     for (i, &flag) in mask.data.iter().enumerate() {
//         if let Some(selector) = keep {
//             if !selector.get(i).copied().unwrap_or(false) {
//                 continue;
//             }
//         }
//         total += 1;
//         vegetation += flag as usize;
//     }
//     Coverage {
//         total_pixels: total,
//         vegetation_pixels: vegetation,
//         coverage_percentage: if total == 0 {
//             0.0
//         } else {
//             (vegetation as f64 / total as f64) * 100.0
//         },
//     }
// }

/// Vegetation and total ground area in m²; both `None` without a GSD. Pure function.
pub fn area_metrics(coverage: &Coverage, gsd_m: Option<f64>) -> (Option<f64>, Option<f64>) {
    match gsd_m {
        Some(gsd) if gsd > 0.0 => {
            let pixel_area = gsd * gsd;
            (
                Some(coverage.vegetation_pixels as f64 * pixel_area),
                Some(coverage.total_pixels as f64 * pixel_area),
            )
        }
        _ => (None, None),
    }
}

/// Low/medium/high classification from user thresholds. Pure Function.
pub fn classify_coverage(
    coverage_percentage: f64,
    low_threshold: f64,
    medium_threshold: f64,
) -> Result<String> {
    if !(0.0..=100.0).contains(&low_threshold) || !(0.0..=100.0).contains(&medium_threshold) {
        return Err(PluginError::Value(
            "coverage thresholds must be between 0 and 100".into(),
        ));
    }
    if low_threshold > medium_threshold {
        return Err(PluginError::Value(format!(
            "low_threshold ({low_threshold}) must not exceed medium_threshold ({medium_threshold})"
        )));
    }
    let class = if coverage_percentage <= low_threshold {
        "low"
    } else if coverage_percentage <= medium_threshold {
        "medium"
    } else {
        "high"
    };
    Ok(class.to_string())
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
    fn coverage_counts_and_percentage() {
        let mask = mask_from(4, 1, vec![1, 1, 0, 0]);
        let coverage = calculate_coverage(&mask, None);
        assert_eq!(coverage.total_pixels, 4);
        assert_eq!(coverage.vegetation_pixels, 2);
        assert!((coverage.coverage_percentage - 50.0).abs() < 1e-9);
    }

    #[test]
    fn coverage_respects_selector() {
        let mask = mask_from(4, 1, vec![1, 1, 0, 0]);
        let coverage = calculate_coverage(&mask, Some(&[true, false, true, false]));
        assert_eq!(coverage.total_pixels, 2);
        assert_eq!(coverage.vegetation_pixels, 1);
    }

    #[test]
    fn area_metrics_need_gsd() {
        let coverage = Coverage {
            total_pixels: 100,
            vegetation_pixels: 40,
            coverage_percentage: 40.0,
        };
        assert_eq!(area_metrics(&coverage, None), (None, None));
        let (veg, total) = area_metrics(&coverage, Some(0.5));
        assert_eq!((veg, total), (Some(10.0), Some(25.0)));
    }

    #[test]
    fn classification_boundaries_are_inclusive() {
        assert_eq!(classify_coverage(20.0, 20.0, 50.0).unwrap(), "low");
        assert_eq!(classify_coverage(20.1, 20.0, 50.0).unwrap(), "medium");
        assert_eq!(classify_coverage(50.0, 20.0, 50.0).unwrap(), "medium");
        assert_eq!(classify_coverage(50.1, 20.0, 50.0).unwrap(), "high");
        assert!(classify_coverage(10.0, 60.0, 40.0).is_err());
    }
}
