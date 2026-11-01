#![allow(dead_code)]

//! Module 10.8 - Change Visualization.
//!
//! The Python prototype draws a Matplotlib figure and prints the analysis, which
//! is not something a Rust backend should do. In the Tauri app the frontend
//! draws the figure, so Module 10.8's Rust job is to hand the page a stable
//! payload:
//!
//! - [`ChangeVisualization`] - the before/after image paths plus the 10.4 - 10.6
//!   results, everything the viewer needs in one value.
//! - [`AnalysisSummary`] - the same text lines the prototype printed, kept as
//!   pure data so they can be rendered or tested without a terminal.
//!
//! Both builders are pure: no file is opened and no image is decoded. A missing
//! image file is a frontend concern (`<img>` fallback), so the payload carries
//! the path as recorded by Module 1.4 instead of guessing whether it exists.

use serde::Serialize;

use super::analysis::{LandCoverChange, TreeCountChange, VegetationCoverageChange};

/// Everything the 10.8 before/after viewer displays.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeVisualization {
    /// Stored path of the "before" image (Module 1.4 record).
    pub before_image_path: String,
    /// Stored path of the "after" image (Module 1.4 record).
    pub after_image_path: String,
    pub land_cover: Vec<LandCoverChange>,
    pub tree_count: Vec<TreeCountChange>,
    /// 10.3 result, when the caller computed vegetation coverage.
    pub vegetation: Option<VegetationCoverageChange>,
}

/// Builds the 10.8 payload from the comparison images and the analysis results.
pub fn build_change_visualization(
    before_image_path: impl Into<String>,
    after_image_path: impl Into<String>,
    land_cover: Vec<LandCoverChange>,
    tree_count: Vec<TreeCountChange>,
    vegetation: Option<VegetationCoverageChange>,
) -> ChangeVisualization {
    ChangeVisualization {
        before_image_path: before_image_path.into(),
        after_image_path: after_image_path.into(),
        land_cover,
        tree_count,
        vegetation,
    }
}

/// Text lines of the 10.8 analytical panel.
///
/// Mirrors the prototype's `display_analytical_results`, but as structured data
/// instead of `print`, so the caller decides where it goes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSummary {
    /// Only locations whose land-cover class changed: `area-A: vegetation -> built_up`.
    pub land_cover_changes: Vec<String>,
    /// Every location: `area-A: -15 pohon (berkurang)`.
    pub tree_count_changes: Vec<String>,
    /// `Perubahan: 12.0 percentage points`, when coverage was compared.
    pub vegetation_change: Option<String>,
}

/// 10.8 analytical results, in the same order and wording as the prototype.
pub fn describe_analytical_results(
    land_cover: &[LandCoverChange],
    tree_count: &[TreeCountChange],
    vegetation: Option<VegetationCoverageChange>,
) -> AnalysisSummary {
    AnalysisSummary {
        land_cover_changes: land_cover
            .iter()
            .filter(|change| change.changed)
            .map(|change| {
                format!(
                    "{}: {} -> {}",
                    change.location_id, change.before, change.after
                )
            })
            .collect(),
        tree_count_changes: tree_count
            .iter()
            .map(|change| {
                format!(
                    "{}: {:+} pohon ({})",
                    change.location_id,
                    change.change,
                    change.direction.label()
                )
            })
            .collect(),
        vegetation_change: vegetation.map(|change| {
            // `{:?}` keeps the prototype's `12.0` style instead of printing `12`.
            format!("Perubahan: {:?} percentage points", change.change)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_10::analysis::{TreeCountChange, TreeCountDirection};

    fn land_cover(location_id: &str, before: &str, after: &str) -> LandCoverChange {
        LandCoverChange {
            location_id: location_id.to_string(),
            before: before.to_string(),
            after: after.to_string(),
            changed: before != after,
        }
    }

    fn tree_count(location_id: &str, before: u32, after: u32) -> TreeCountChange {
        let change = i64::from(after) - i64::from(before);
        TreeCountChange {
            location_id: location_id.to_string(),
            before,
            after,
            change,
            direction: match change {
                change if change > 0 => TreeCountDirection::Increased,
                change if change < 0 => TreeCountDirection::Decreased,
                _ => TreeCountDirection::Unchanged,
            },
        }
    }

    fn vegetation(before: f64, after: f64) -> VegetationCoverageChange {
        VegetationCoverageChange {
            before,
            after,
            change: after - before,
        }
    }

    #[test]
    fn builds_visualization_payload_with_real_image_paths() {
        let payload = build_change_visualization(
            "C:/library/image_01.tif",
            "C:/library/image_02.tif",
            vec![land_cover("area-A", "vegetation", "built_up")],
            vec![tree_count("area-A", 120, 105)],
            Some(vegetation(60.0, 72.0)),
        );

        assert_eq!(payload.before_image_path, "C:/library/image_01.tif");
        assert_eq!(payload.after_image_path, "C:/library/image_02.tif");
        assert_eq!(payload.land_cover.len(), 1);
        assert_eq!(payload.tree_count.len(), 1);
        assert_eq!(payload.vegetation, Some(vegetation(60.0, 72.0)));
    }

    #[test]
    fn summarizes_only_changed_land_cover() {
        let summary = describe_analytical_results(
            &[
                land_cover("area-A", "vegetation", "built_up"),
                land_cover("area-B", "water", "water"),
            ],
            &[],
            None,
        );

        assert_eq!(
            summary.land_cover_changes,
            vec!["area-A: vegetation -> built_up".to_string()]
        );
        assert!(summary.tree_count_changes.is_empty());
        assert_eq!(summary.vegetation_change, None);
    }

    #[test]
    fn summarizes_every_tree_count_change() {
        let summary = describe_analytical_results(
            &[],
            &[tree_count("area-A", 120, 105), tree_count("area-B", 80, 95)],
            None,
        );

        assert_eq!(
            summary.tree_count_changes,
            vec![
                "area-A: -15 pohon (berkurang)".to_string(),
                "area-B: +15 pohon (bertambah)".to_string(),
            ]
        );
    }

    #[test]
    fn formats_vegetation_change_like_the_prototype() {
        let summary = describe_analytical_results(&[], &[], Some(vegetation(60.0, 72.0)));

        assert_eq!(
            summary.vegetation_change,
            Some("Perubahan: 12.0 percentage points".to_string())
        );
    }

    #[test]
    fn empty_results_produce_an_empty_summary() {
        assert_eq!(
            describe_analytical_results(&[], &[], None),
            AnalysisSummary::default()
        );
    }

    #[test]
    fn visualization_payload_serializes_to_camel_case() {
        let payload = build_change_visualization(
            "before.png",
            "after.png",
            vec![],
            vec![],
            Some(vegetation(25.0, 75.0)),
        );

        assert_eq!(
            serde_json::to_value(&payload).expect("terserialisasi"),
            serde_json::json!({
                "beforeImagePath": "before.png",
                "afterImagePath": "after.png",
                "landCover": [],
                "treeCount": [],
                "vegetation": { "before": 25.0, "after": 75.0, "change": 50.0 }
            })
        );
    }
}
