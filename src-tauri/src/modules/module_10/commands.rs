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
    classify_change_magnitude, compare_before_after, detect_significant_change_areas,
    group_by_acquisition_date, identify_land_cover_changes, latest_before_after,
    ChangeMagnitudeEntry, ImageObservation, LandCoverChange, ObservationPeriod,
    SignificantChangeArea, TemporalChangeError, TreeCountChange, VegetationCoverageChange,
    DEFAULT_SIGNIFICANCE_THRESHOLD,
};
use super::comparisons::{
    get_comparison, insert_comparison, list_comparisons, magnitude_level_name, ComparisonRecord,
    NewComparison,
};
use super::coverage::{coverage_from_image_path, CoverageError, VegetationCoverage};
use super::observations::{observations_from_images, ObservationCollection, SkippedImage};
use super::spatial_change::{
    bounds_for_observation, build_report, mask_from_image, SpatialChangeError, SpatialChangeReport,
};
use super::visualization::{describe_analytical_results, AnalysisSummary};

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

/// Result of comparing two sessions: the representative images, their measured
/// vegetation coverage, and the stored comparison row.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionComparison {
    pub comparison_id: String,
    pub before: ImageObservation,
    pub after: ImageObservation,
    pub interval_days: i64,
    pub before_coverage: VegetationCoverage,
    pub after_coverage: VegetationCoverage,
    pub change: VegetationCoverageChange,
    pub magnitude: f64,
    /// `rendah` | `sedang` | `tinggi`.
    pub magnitude_level: String,
    /// 10.8 analytical summary lines for the panel.
    pub summary: AnalysisSummary,
}

/// Runs a before/after comparison between two sessions (10.2 + 10.3).
///
/// Each session contributes its latest captured image. Coverage is measured on
/// those pixels, the difference is classified, and the result is stored in
/// `temporal_comparison` so the page and the dashboard can list it later.
#[tauri::command]
pub fn compare_sessions(
    state: tauri::State<AppState>,
    before_session_id: Uuid,
    after_session_id: Uuid,
) -> Result<SessionComparison, CommandError> {
    let (project_id, before_images, after_images) = {
        let conn = state.db.lock().map_err(lock_err)?;
        let before_session = flight_session::get_session(&conn, before_session_id)?;
        let after_session = flight_session::get_session(&conn, after_session_id)?;
        let before_images = flight_session::get_images_by_session(&conn, before_session_id)?;
        let after_images = flight_session::get_images_by_session(&conn, after_session_id)?;
        // Sessions from different projects cannot be meaningfully compared.
        if before_session.project_id != after_session.project_id {
            return Err(CommandError {
                code: "TEMPORAL_SESSION_MISMATCH".to_string(),
                message: "both sessions must belong to the same project".to_string(),
            });
        }
        (
            before_session.project_id.to_string(),
            before_images,
            after_images,
        )
    };

    let latest_observation = |images: &[Image]| -> Result<ImageObservation, CommandError> {
        observations_from_images(images)
            .observations
            .into_iter()
            .max_by(|left, right| left.captured_at.cmp(&right.captured_at))
            .ok_or_else(|| CommandError {
                code: "TEMPORAL_NO_DATED_IMAGES".to_string(),
                message: "a session has no image with a capture date".to_string(),
            })
    };

    let before = latest_observation(&before_images)?;
    let after = latest_observation(&after_images)?;
    let comparison = compare_before_after(before, after).map_err(CommandError::from)?;

    let before_coverage = coverage_from_image_path(Path::new(&comparison.before.file_path))
        .map_err(|error| CommandError {
            code: "VEGETATION_COVERAGE_ERROR".to_string(),
            message: error.to_string(),
        })?;
    let after_coverage =
        coverage_from_image_path(Path::new(&comparison.after.file_path)).map_err(|error| {
            CommandError {
                code: "VEGETATION_COVERAGE_ERROR".to_string(),
                message: error.to_string(),
            }
        })?;
    let change = calculate_vegetation_change(
        before_coverage.coverage_percent,
        after_coverage.coverage_percent,
    )
    .map_err(CommandError::from)?;
    let magnitude = change.change.abs();
    let level = classify_change_magnitude(magnitude).map_err(CommandError::from)?;

    let stored = {
        let conn = state.db.lock().map_err(lock_err)?;
        insert_comparison(
            &conn,
            &NewComparison {
                project_id: Some(project_id),
                before_session_id: before_session_id.to_string(),
                after_session_id: after_session_id.to_string(),
                before_image_id: comparison.before.image_id.to_string(),
                after_image_id: comparison.after.image_id.to_string(),
                before_date: comparison.before.acquisition_date().to_string(),
                after_date: comparison.after.acquisition_date().to_string(),
                interval_days: comparison.interval_days,
                metric: "vegetation_coverage_percent".to_string(),
                before_value: change.before,
                after_value: change.after,
                delta: change.change,
                magnitude,
                magnitude_level: magnitude_level_name(level).to_string(),
            },
        )
        .map_err(|error| CommandError {
            code: "TEMPORAL_STORAGE_ERROR".to_string(),
            message: error.to_string(),
        })?
    };

    Ok(SessionComparison {
        comparison_id: stored.id,
        before: comparison.before,
        after: comparison.after,
        interval_days: comparison.interval_days,
        before_coverage,
        after_coverage,
        change,
        magnitude,
        magnitude_level: magnitude_level_name(level).to_string(),
        summary: describe_analytical_results(&[], &[], Some(change)),
    })
}

/// Writes one stored comparison to `map_storage/exports/` as JSON (10.8 export).
///
/// Returns the written path so the page can tell the user where the file is.
#[tauri::command]
pub fn export_comparison_json(
    state: tauri::State<AppState>,
    comparison_id: String,
) -> Result<String, CommandError> {
    let record = {
        let conn = state.db.lock().map_err(lock_err)?;
        get_comparison(&conn, &comparison_id)
            .map_err(|error| CommandError {
                code: "TEMPORAL_STORAGE_ERROR".to_string(),
                message: error.to_string(),
            })?
            .ok_or_else(|| CommandError {
                code: "TEMPORAL_COMPARISON_NOT_FOUND".to_string(),
                message: format!("comparison '{comparison_id}' not found"),
            })?
    };

    let change = VegetationCoverageChange {
        before: record.before_value,
        after: record.after_value,
        change: record.delta,
    };
    let summary = describe_analytical_results(&[], &[], Some(change));
    let payload = serde_json::json!({
        "comparison_id": record.id,
        "metric": record.metric,
        "project_id": record.project_id,
        "before_session_id": record.before_session_id,
        "after_session_id": record.after_session_id,
        "before_image_id": record.before_image_id,
        "after_image_id": record.after_image_id,
        "before_date": record.before_date,
        "after_date": record.after_date,
        "interval_days": record.interval_days,
        "before": record.before_value,
        "after": record.after_value,
        "delta": record.delta,
        "magnitude": record.magnitude,
        "magnitude_level": record.magnitude_level,
        "created_at": record.created_at,
        "summary": summary,
        "contract": change.to_contract_json(),
    });

    let exports = crate::modules::module_03::state::get_global_state()
        .storage_dir
        .join("exports");
    std::fs::create_dir_all(&exports).map_err(|error| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("cannot create '{}': {error}", exports.display()),
    })?;
    let path = exports.join(format!("temporal_comparison_{}.json", record.id));
    let text = serde_json::to_string_pretty(&payload).map_err(|error| CommandError {
        code: "SERIALIZATION_ERROR".to_string(),
        message: error.to_string(),
    })?;
    std::fs::write(&path, text).map_err(|error| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("cannot write '{}': {error}", path.display()),
    })?;
    Ok(path.to_string_lossy().into_owned())
}

/// Latest stored comparisons for a project, newest first (dashboard input).
#[tauri::command]
pub fn get_change_summary(
    state: tauri::State<AppState>,
    project_id: Uuid,
) -> Result<Vec<ComparisonRecord>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    list_comparisons(&conn, Some(&project_id.to_string()), 50).map_err(|error| CommandError {
        code: "TEMPORAL_STORAGE_ERROR".to_string(),
        message: error.to_string(),
    })
}

/// Spatial change detection (10.7) between two sessions (10.2 before/after).
///
/// Both session images need GPS, altitude and pixel dimensions. Vegetation
/// masks are compared over the shared footprint; changed regions above
/// `min_area_m2` (default 10 m²) are published as a Map Explorer layer.
#[tauri::command]
pub fn detect_change_areas(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    before_session_id: Uuid,
    after_session_id: Uuid,
    min_area_m2: Option<f64>,
) -> Result<SpatialChangeReport, CommandError> {
    use crate::modules::module_03::sp_result::types::{
        FieldDef, LayerDisplayMode, LayerPayload, SpatialGeometry, SpatialProperties, SpatialResult,
    };
    use crate::modules::module_03::state::get_global_state;
    use tauri::Emitter;

    crate::modules::module_03::ortho_source::instance_lock::require_primary_instance()?;

    let latest_image = |session_id: Uuid| -> Result<Image, CommandError> {
        let conn = state.db.lock().map_err(lock_err)?;
        let images = flight_session::get_images_by_session(&conn, session_id)?;
        observations_from_images(&images)
            .observations
            .into_iter()
            .max_by(|left, right| left.captured_at.cmp(&right.captured_at))
            .map(|observation| (observation.image_id, observation.captured_at))
            .and_then(|(image_id, _)| images.into_iter().find(|image| image.id == image_id))
            .ok_or_else(|| CommandError {
                code: "TEMPORAL_NO_DATED_IMAGES".to_string(),
                message: "a session has no image with a capture date".to_string(),
            })
    };

    let observations_for = |image: &Image| -> Result<ImageObservation, CommandError> {
        observations_from_images(std::slice::from_ref(image))
            .observations
            .into_iter()
            .next()
            .ok_or_else(|| CommandError {
                code: "TEMPORAL_NO_DATED_IMAGES".to_string(),
                message: "image has no capture date".to_string(),
            })
    };

    let before_image = latest_image(before_session_id)?;
    let after_image = latest_image(after_session_id)?;
    let before = observations_for(&before_image)?;
    let after = observations_for(&after_image)?;
    let comparison = compare_before_after(before, after).map_err(CommandError::from)?;

    let bounds_of = |observation: &ImageObservation| -> Result<[f64; 4], CommandError> {
        bounds_for_observation(observation).ok_or_else(|| CommandError {
            code: "TEMPORAL_MISSING_GEOREFERENCE".to_string(),
            message: format!(
                "'{}' needs GPS, altitude and pixel size for spatial change detection",
                observation.file_path
            ),
        })
    };
    let before_bounds = bounds_of(&comparison.before)?;
    let after_bounds = bounds_of(&comparison.after)?;

    let before_mask = mask_from_image(Path::new(&comparison.before.file_path), before_bounds)
        .map_err(spatial_error)?;
    let after_mask = mask_from_image(Path::new(&comparison.after.file_path), after_bounds)
        .map_err(spatial_error)?;

    // Provisional report to get the changed fraction, then classify it.
    let provisional = build_report(
        &before_mask,
        &after_mask,
        min_area_m2.unwrap_or(10.0),
        "rendah".to_string(),
        String::new(),
        String::new(),
    )
    .map_err(spatial_error)?;
    let level = classify_change_magnitude(provisional.changed_fraction_percent)
        .map_err(CommandError::from)?;

    let short = |id: Uuid| id.to_string().replace('-', "")[..8].to_string();
    let layer_id = format!(
        "temporal_{}_{}",
        short(before_session_id),
        short(after_session_id)
    );
    let layer_name = format!(
        "Perubahan {} → {}",
        comparison.before.acquisition_date(),
        comparison.after.acquisition_date()
    );
    let report = build_report(
        &before_mask,
        &after_mask,
        min_area_m2.unwrap_or(10.0),
        magnitude_level_name(level).to_string(),
        layer_id.clone(),
        layer_name.clone(),
    )
    .map_err(spatial_error)?;

    // Publish the polygons so Module 3 can show them.
    let features: Vec<SpatialResult> = report
        .features
        .iter()
        .map(|feature| {
            let [min_lon, min_lat, max_lon, max_lat] = feature.bbox;
            let mut properties = serde_json::Map::new();
            properties.insert("change_type".into(), serde_json::json!(feature.change_type));
            properties.insert("area_m2".into(), serde_json::json!(feature.area_m2.round()));
            properties.insert("pixels".into(), serde_json::json!(feature.pixels));
            SpatialResult {
                source: layer_id.clone(),
                geometry: SpatialGeometry::Polygon {
                    coordinates: vec![vec![
                        vec![min_lon, min_lat],
                        vec![max_lon, min_lat],
                        vec![max_lon, max_lat],
                        vec![min_lon, max_lat],
                        vec![min_lon, min_lat],
                    ]],
                },
                properties: SpatialProperties::Generic(properties),
                image_url: None,
                edited_fields: None,
                extra: serde_json::Map::new(),
            }
        })
        .collect();

    if !features.is_empty() {
        let fields = vec![
            FieldDef {
                key: "change_type".into(),
                label: "Change".into(),
                unit: None,
                visible: true,
                custom: false,
                kind: Some("text".into()),
            },
            FieldDef {
                key: "area_m2".into(),
                label: "Area".into(),
                unit: Some("m²".into()),
                visible: true,
                custom: false,
                kind: Some("number".into()),
            },
        ];
        let payload = LayerPayload {
            layer_id: layer_id.clone(),
            layer_name,
            display_preference: LayerDisplayMode::Polygon {
                color: "#b42318".to_string(),
                opacity: 0.45,
                image_url: None,
                fields: Some(fields),
            },
            spatial_results: features,
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            concurrency_token: None,
            extra: serde_json::Map::new(),
        };
        if let Ok(saved) = get_global_state().save_spatial_result(payload) {
            let _ = app.emit("spatial_result:saved", &saved);
        }
    }

    Ok(report)
}

fn spatial_error(error: SpatialChangeError) -> CommandError {
    CommandError {
        code: "TEMPORAL_SPATIAL_ERROR".to_string(),
        message: error.to_string(),
    }
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
