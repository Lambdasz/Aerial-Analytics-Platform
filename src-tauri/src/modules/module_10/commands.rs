#![allow(dead_code)]

//! Tauri commands for Simple Temporal Change Analysis (Module 10).
//!
//! The commands here are the bridge between Module 1.4 data and Module 10's
//! pure analysis:
//!
//! | Command | Submodule | Input | Data source |
//! |---|---|---|---|
//! | [`get_observation_periods`] | 10.1 | `session_id` | Module 1.4 [`Image`] rows |
//! | [`get_vegetation_change`] | 10.2 + 10.3 | `session_id` | Module 1.4 images + their pixels |
//!
//! The heavier analysis (decoding images) runs **after** the database lock is
//! released, so a long comparison never blocks Module 1's session commands.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::models::Image;
use crate::modules::module_01::error::SessionError;
use crate::modules::module_01::flight_session;
use crate::plugin_manager::error::CommandError;
use crate::AppState;

use super::analysis::{
    calculate_change_magnitudes, calculate_tree_count_changes, calculate_vegetation_change,
    detect_significant_change_areas, group_by_acquisition_date, identify_land_cover_changes,
    latest_before_after, ChangeMagnitudeEntry, ImageObservation, LandCoverChange,
    ObservationPeriod, SignificantChangeArea, TemporalChangeError, TreeCountChange,
    VegetationCoverageChange, DEFAULT_SIGNIFICANCE_THRESHOLD,
};
use super::coverage::{coverage_from_image_path, CoverageError, VegetationCoverage};
use super::observations::{observations_from_images, ObservationCollection, SkippedImage};

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    SessionError::LockPoisoned(e.to_string()).into()
}

/// Maps a domain error onto the structured error the frontend matches on.
///
/// All 10.4 - 10.7 failures share one code because they are validation failures
/// of the same analysis pipeline; the message names the offending location.
impl From<TemporalChangeError> for CommandError {
    fn from(error: TemporalChangeError) -> Self {
        Self {
            code: "TEMPORAL_ANALYSIS_ERROR".to_string(),
            message: error.to_string(),
        }
    }
}

/// 10.1 Multi-Date Image Management: a session's images grouped per date.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionObservationPeriods {
    pub session_id: Uuid,
    /// Chronological observation periods, each holding that day's images with
    /// the metadata Module 10 needs (capture time, GPS, altitude, dimensions).
    pub periods: Vec<ObservationPeriod>,
    /// Images Module 1.4 stores but Module 10 cannot date or analyse, reported
    /// with the reason instead of disappearing silently.
    pub skipped: Vec<SkippedImage>,
}

/// Pure part of [`get_observation_periods`], kept separate so it can be tested
/// without a Tauri [`AppState`].
pub fn summarize_observations(session_id: Uuid, images: &[Image]) -> SessionObservationPeriods {
    let ObservationCollection {
        observations,
        skipped,
    } = observations_from_images(images);

    SessionObservationPeriods {
        session_id,
        periods: group_by_acquisition_date(&observations),
        skipped,
    }
}

/// Returns the session's images grouped by acquisition date.
///
/// This is the Module 10.1 entry point and the one the page should call first:
/// every later comparison (10.2 - 10.8) selects its before/after period from
/// the periods returned here.
#[tauri::command]
pub fn get_observation_periods(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<SessionObservationPeriods, CommandError> {
    let images = {
        let conn = state.db.lock().map_err(lock_err)?;
        flight_session::get_images_by_session(&conn, session_id)?
    };

    Ok(summarize_observations(session_id, &images))
}

/// 10.3 Vegetation coverage change with the images that produced it, so the UI
/// can show "before and after imagery" (10.8) from the same payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VegetationChangeReport {
    pub before: ImageObservation,
    pub after: ImageObservation,
    /// Days between the two capture dates; always positive.
    pub interval_days: i64,
    pub before_coverage: VegetationCoverage,
    pub after_coverage: VegetationCoverage,
    /// Percentage-point change (`after - before`), plus the 10.8 contract form.
    pub change: VegetationCoverageChange,
}

/// Error union for the 10.2 + 10.3 pipeline, so the command can return one
/// structured [`CommandError`] instead of a stringly-typed message.
#[derive(Debug, Error)]
pub enum VegetationAnalysisError {
    #[error(transparent)]
    Temporal(#[from] TemporalChangeError),

    #[error(transparent)]
    Coverage(#[from] CoverageError),
}

impl VegetationAnalysisError {
    /// Stable code the frontend can match on.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Temporal(_) => "TEMPORAL_ANALYSIS_ERROR",
            Self::Coverage(_) => "VEGETATION_COVERAGE_ERROR",
        }
    }

    fn into_command_error(self) -> CommandError {
        CommandError {
            code: self.code().to_string(),
            message: self.to_string(),
        }
    }
}

/// Pure-ish 10.2 + 10.3 pipeline: pick the two most recent observation periods
/// from `observations`, measure vegetation coverage on each representative
/// image, and return the difference.
///
/// Only file reads happen here; no database access, so it is callable from
/// tests and from other commands.
pub fn analyze_vegetation_change(
    observations: &[ImageObservation],
) -> Result<VegetationChangeReport, VegetationAnalysisError> {
    let comparison = latest_before_after(observations)?;

    let before_coverage = coverage_from_image_path(Path::new(&comparison.before.file_path))?;
    let after_coverage = coverage_from_image_path(Path::new(&comparison.after.file_path))?;
    let change = calculate_vegetation_change(
        before_coverage.coverage_percent,
        after_coverage.coverage_percent,
    )?;

    Ok(VegetationChangeReport {
        before: comparison.before,
        after: comparison.after,
        interval_days: comparison.interval_days,
        before_coverage,
        after_coverage,
        change,
    })
}

/// Compares vegetation coverage between the two most recent observation dates
/// of a session, using the stored image files themselves.
///
/// The `change` field is in percentage points and `change.to_contract_json()`
/// produces the payload Module 10.8 forwards to Module 11.5.
#[tauri::command]
pub fn get_vegetation_change(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<VegetationChangeReport, CommandError> {
    let images = {
        let conn = state.db.lock().map_err(lock_err)?;
        flight_session::get_images_by_session(&conn, session_id)?
    };

    let observations = observations_from_images(&images).observations;

    analyze_vegetation_change(&observations).map_err(VegetationAnalysisError::into_command_error)
}

// ---------------------------------------------------------------------------
// 10.4 - 10.7: per-location comparisons
//
// Module 1.4 stores images, not per-location analysis results. The location
// maps these commands accept are produced by Module 3 (plots / grid cells) and
// Module 4/5 (classification and tree detection) and are passed in as-is, so
// Module 10 never invents land-cover classes or tree counts.
// ---------------------------------------------------------------------------

/// 10.4 Land-Cover Change: compare per-location classes of two periods.
///
/// Both maps must contain the same location ids; otherwise the command fails
/// with `TEMPORAL_ANALYSIS_ERROR` instead of returning a partial comparison.
#[tauri::command]
pub fn get_land_cover_changes(
    before_classes: BTreeMap<String, String>,
    after_classes: BTreeMap<String, String>,
) -> Result<Vec<LandCoverChange>, CommandError> {
    identify_land_cover_changes(&before_classes, &after_classes).map_err(CommandError::from)
}

/// 10.5 Tree Count Change: compare per-location tree counts of two periods.
#[tauri::command]
pub fn get_tree_count_changes(
    before_counts: BTreeMap<String, u32>,
    after_counts: BTreeMap<String, u32>,
) -> Result<Vec<TreeCountChange>, CommandError> {
    calculate_tree_count_changes(&before_counts, &after_counts).map_err(CommandError::from)
}

/// 10.6 Change Magnitude Analysis: per-location change, magnitude, and level.
#[tauri::command]
pub fn get_change_magnitudes(
    before_values: BTreeMap<String, f64>,
    after_values: BTreeMap<String, f64>,
) -> Result<Vec<ChangeMagnitudeEntry>, CommandError> {
    calculate_change_magnitudes(&before_values, &after_values).map_err(CommandError::from)
}

/// 10.7 Change Area Detection: flag locations whose magnitude reaches
/// `threshold`. Omitting the threshold keeps the prototype's default of 10.
#[tauri::command]
pub fn get_significant_change_areas(
    before_values: BTreeMap<String, f64>,
    after_values: BTreeMap<String, f64>,
    threshold: Option<f64>,
) -> Result<Vec<SignificantChangeArea>, CommandError> {
    detect_significant_change_areas(
        &before_values,
        &after_values,
        threshold.unwrap_or(DEFAULT_SIGNIFICANCE_THRESHOLD),
    )
    .map_err(CommandError::from)
}

/// Runs a comparison between two sessions' analysis results (vegetation
/// coverage, land-cover, tree count) and produces a
/// [`TemporalChangeResult`](super::functions::TemporalChangeResult)
/// and change-area payload.
///
/// **Status**: `todo!()` stub — added so Module 10 has a minimal
/// contract before the deadline, matching the Module 9 (`plot_api`) pattern.
#[tauri::command]
pub fn compare_sessions(
    _baseline_session_id: String,
    _target_session_id: String,
) -> Result<super::functions::TemporalChangeResult, String> {
    todo!("M10: run comparison between two sessions' analysis results")
}

/// Returns the latest computed temporal summary for a project,
/// for Module 11 (dashboard / reporting) to consume.
///
/// **Status**: `todo!()` stub — added so Module 10 has a minimal
/// contract before the deadline, matching the Module 9 (`plot_api`) pattern.
#[tauri::command]
pub fn get_change_summary(
    _project_id: String,
) -> Result<super::functions::TemporalChangeResult, String> {
    todo!("M10: return latest temporal summary for a project")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use chrono::{NaiveDate, NaiveDateTime};
    use image::{Rgb, RgbImage};

    use super::*;
    use crate::modules::module_10::analysis::{MagnitudeLevel, TreeCountDirection};

    fn at(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 2, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .expect("tanggal uji harus valid")
    }

    /// Writes a `4 x 1` PNG with `green` green pixels and the rest red, so the
    /// coverage percentage is exactly `green * 25`.
    fn coverage_image(dir: &Path, name: &str, green: u32) -> PathBuf {
        let mut image = RgbImage::new(4, 1);
        for x in 0..4 {
            let color = if x < green {
                Rgb([0, 200, 0])
            } else {
                Rgb([200, 0, 0])
            };
            image.put_pixel(x, 0, color);
        }

        let path = dir.join(name);
        image.save(&path).expect("citra uji tersimpan");
        path
    }

    #[test]
    fn compares_coverage_of_the_two_most_recent_periods() {
        let dir = tempfile::tempdir().expect("direktori sementara dibuat");
        let before_path = coverage_image(dir.path(), "before.png", 1);
        let after_path = coverage_image(dir.path(), "after.png", 3);

        let observations = vec![
            ImageObservation::new(Uuid::new_v4(), before_path.display().to_string(), at(10, 7)),
            ImageObservation::new(Uuid::new_v4(), after_path.display().to_string(), at(20, 7)),
        ];

        let report = analyze_vegetation_change(&observations).expect("dua periode valid");

        assert_eq!(report.interval_days, 10);
        assert_eq!(report.before_coverage.coverage_percent, 25.0);
        assert_eq!(report.after_coverage.coverage_percent, 75.0);
        assert_eq!(report.change.change, 50.0);
        assert_eq!(
            report.change.to_contract_json(),
            serde_json::json!({ "before": 25.0, "after": 75.0, "delta": 50.0 })
        );
    }

    #[test]
    fn reports_a_single_period_with_a_stable_error_code() {
        let dir = tempfile::tempdir().expect("direktori sementara dibuat");
        let path = coverage_image(dir.path(), "only.png", 2);
        let observations = vec![ImageObservation::new(
            Uuid::new_v4(),
            path.display().to_string(),
            at(10, 7),
        )];

        let error = analyze_vegetation_change(&observations).expect_err("butuh dua periode");

        assert_eq!(error.code(), "TEMPORAL_ANALYSIS_ERROR");
    }

    #[test]
    fn reports_a_missing_image_file_with_a_stable_error_code() {
        let observations = vec![
            ImageObservation::new(Uuid::new_v4(), "C:/tidak/ada/before.png", at(10, 7)),
            ImageObservation::new(Uuid::new_v4(), "C:/tidak/ada/after.png", at(20, 7)),
        ];

        let error = analyze_vegetation_change(&observations).expect_err("berkas tidak ada");

        assert_eq!(error.code(), "VEGETATION_COVERAGE_ERROR");
    }

    #[test]
    fn summarizes_periods_and_skips_undated_images() {
        // `summarize_observations` needs stored rows; the adapter itself is
        // covered by `observations` tests. Here we only verify that the
        // command-level summary keeps the session id and both result buckets.
        let summary = summarize_observations(Uuid::nil(), &[]);

        assert_eq!(summary.session_id, Uuid::nil());
        assert!(summary.periods.is_empty());
        assert!(summary.skipped.is_empty());
    }

    fn classes(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(location_id, class)| (location_id.to_string(), class.to_string()))
            .collect()
    }

    fn values(entries: &[(&str, f64)]) -> BTreeMap<String, f64> {
        entries
            .iter()
            .map(|(location_id, value)| (location_id.to_string(), *value))
            .collect()
    }

    #[test]
    fn land_cover_command_returns_sorted_changes() {
        let results = get_land_cover_changes(
            classes(&[("area-B", "water"), ("area-A", "vegetation")]),
            classes(&[("area-B", "water"), ("area-A", "built_up")]),
        )
        .expect("lokasi sama");

        let ids: Vec<&str> = results
            .iter()
            .map(|change| change.location_id.as_str())
            .collect();
        assert_eq!(ids, vec!["area-A", "area-B"]);
        assert!(results[0].changed);
        assert!(!results[1].changed);
    }

    #[test]
    fn land_cover_command_reports_mismatched_locations() {
        let error = get_land_cover_changes(
            classes(&[("area-A", "vegetation")]),
            classes(&[("area-B", "water")]),
        )
        .expect_err("lokasi berbeda");

        assert_eq!(error.code, "TEMPORAL_ANALYSIS_ERROR");
        assert!(error.message.contains("area-A"));
    }

    #[test]
    fn tree_count_command_reports_direction() {
        let before = [("area-A".to_string(), 120u32)].into_iter().collect();
        let after = [("area-A".to_string(), 105u32)].into_iter().collect();

        let results = get_tree_count_changes(before, after).expect("lokasi sama");

        assert_eq!(results[0].change, -15);
        assert_eq!(results[0].direction, TreeCountDirection::Decreased);
    }

    #[test]
    fn magnitude_commands_share_the_same_pipeline() {
        let before = values(&[("area-A", 60.0), ("area-B", 50.0)]);
        let after = values(&[("area-A", 72.0), ("area-B", 52.0)]);

        let magnitudes = get_change_magnitudes(before.clone(), after.clone()).expect("lokasi sama");

        assert_eq!(magnitudes[0].level, MagnitudeLevel::Medium);
        assert_eq!(magnitudes[1].level, MagnitudeLevel::Low);
    }

    #[test]
    fn significance_command_uses_default_threshold() {
        let before = values(&[("area-A", 60.0), ("area-B", 50.0)]);
        let after = values(&[("area-A", 72.0), ("area-B", 52.0)]);

        let results =
            get_significant_change_areas(before.clone(), after.clone(), None).expect("lokasi sama");
        let significant = results.iter().filter(|area| area.significant).count();
        assert_eq!(significant, 1);

        let stricter =
            get_significant_change_areas(before, after, Some(15.0)).expect("lokasi sama");
        assert!(stricter.iter().all(|area| !area.significant));
    }

    #[test]
    fn significance_command_rejects_negative_threshold() {
        let error = get_significant_change_areas(values(&[]), values(&[]), Some(-5.0))
            .expect_err("threshold negatif");

        assert_eq!(error.code, "TEMPORAL_ANALYSIS_ERROR");
        assert!(error.message.contains("threshold"));
    }
}
