#![allow(dead_code)]

//! Pure analysis functions for Module 10 - Simple Temporal Change Analysis.
//!
//! This module covers submodules 10.1 - 10.7; 10.8 lives in
//! [`super::visualization`].
//!
//! | Submodule | Function |
//! |---|---|
//! | 10.1 Multi-Date Image Management | [`group_by_acquisition_date`] |
//! | 10.2 Before-and-After Comparison | [`compare_before_after`], [`latest_before_after`] |
//! | 10.3 Vegetation Coverage Change | [`calculate_vegetation_change`] |
//! | 10.4 Land-Cover Change | [`identify_land_cover_changes`] |
//! | 10.5 Tree Count Change | [`calculate_tree_count_changes`] |
//! | 10.6 Change Magnitude Analysis | [`classify_change_magnitude`], [`calculate_change_magnitudes`] |
//! | 10.7 Change Area Detection | [`detect_significant_change_areas`] |
//!
//! Every function is pure: inputs are never mutated and the result is always a
//! new value. Fallible calculations return [`Result`] instead of panicking, so
//! the caller decides how a failed analysis is reported.
//!
//! 10.4 - 10.7 work on location-keyed maps (`location_id -> value`) and always
//! return entries sorted by `location_id`, matching the `sorted(...)` ordering
//! of the Python prototypes. The prototypes raise `TypeError` when a container
//! or value has the wrong type; in Rust those cases are already impossible
//! (`u32` counts, `f64` values, Serde at the command boundary), so only checks
//! that real data can still violate - mismatched locations, negative values,
//! non-finite numbers - return a [`TemporalChangeError`].
//!
//! [`ImageObservation`] is the boundary type between Module 1.4 (image records
//! plus EXIF metadata) and this module. Use
//! [`observations_from_images`](super::observations::observations_from_images)
//! to build it from persisted [`crate::models::Image`] rows; keeping the
//! analysis functions independent of the database makes them unit-testable.

use std::collections::BTreeMap;

use chrono::{NaiveDate, NaiveDateTime};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

/// Errors produced by the temporal change analysis.
#[derive(Debug, Error, PartialEq)]
pub enum TemporalChangeError {
    #[error("tidak ada citra yang dapat dianalisis")]
    EmptyObservations,

    #[error("jumlah periode pengamatan kurang dari dua (ditemukan {found})")]
    InsufficientPeriods { found: usize },

    #[error("waktu 'after' ({after}) harus lebih akhir daripada 'before' ({before})")]
    NonChronologicalPeriods {
        before: NaiveDateTime,
        after: NaiveDateTime,
    },

    #[error("nilai {value} bukan angka yang valid")]
    NonFiniteValue { value: f64 },

    #[error("nilai tutupan vegetasi {value}% di luar rentang 0-100%")]
    CoverageOutOfRange { value: f64 },

    #[error(
        "lokasi pada kedua periode harus sama (hanya di before: {only_before:?}, hanya di after: {only_after:?})"
    )]
    MismatchedLocations {
        only_before: Vec<String>,
        only_after: Vec<String>,
    },

    #[error(
        "nilai pengamatan untuk lokasi '{location_id}' tidak boleh negatif (ditemukan {value})"
    )]
    NegativeObservationValue { location_id: String, value: f64 },

    #[error("magnitude tidak boleh negatif (ditemukan {value})")]
    NegativeMagnitude { value: f64 },

    #[error("threshold tidak boleh negatif (ditemukan {value})")]
    NegativeThreshold { value: f64 },
}

/// One aerial observation: a stored image plus the metadata Module 10 needs.
///
/// All fields except `image_id`, `file_path`, and `captured_at` are optional
/// because Module 1.4 persists images with incomplete EXIF data. An observation
/// without GPS is still usable for date-based comparison, it just cannot take
/// part in spatial analysis (10.7).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageObservation {
    pub image_id: Uuid,
    /// Absolute path to the app-owned copy of the image.
    pub file_path: String,
    /// EXIF `DateTimeOriginal` as local camera time.
    pub captured_at: NaiveDateTime,
    /// GPS latitude in decimal degrees (south is negative).
    pub latitude: Option<f64>,
    /// GPS longitude in decimal degrees (west is negative).
    pub longitude: Option<f64>,
    /// GPS altitude in metres (EXIF).
    pub altitude_m: Option<f64>,
    /// Image width in pixels.
    pub width: Option<u32>,
    /// Image height in pixels.
    pub height: Option<u32>,
}

impl ImageObservation {
    /// Builds an observation with the metadata Module 10.1 requires.
    ///
    /// Location and dimensions default to `None`; fill them with
    /// [`with_metadata`](Self::with_metadata) or a struct update when the
    /// source image has them.
    pub fn new(image_id: Uuid, file_path: impl Into<String>, captured_at: NaiveDateTime) -> Self {
        Self {
            image_id,
            file_path: file_path.into(),
            captured_at,
            latitude: None,
            longitude: None,
            altitude_m: None,
            width: None,
            height: None,
        }
    }

    /// Attaches the optional EXIF metadata carried by the stored image.
    pub fn with_metadata(
        self,
        latitude: Option<f64>,
        longitude: Option<f64>,
        altitude_m: Option<f64>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Self {
        Self {
            latitude,
            longitude,
            altitude_m,
            width,
            height,
            ..self
        }
    }

    /// Acquisition date (the calendar day the image was captured).
    pub fn acquisition_date(&self) -> NaiveDate {
        self.captured_at.date()
    }

    /// `true` when both GPS coordinates are present, i.e. the observation can
    /// be placed on a map.
    pub fn has_location(&self) -> bool {
        self.latitude.is_some() && self.longitude.is_some()
    }
}

/// All observations acquired on one calendar day (one observation period).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationPeriod {
    pub date: NaiveDate,
    /// Ordered by capture time, so the first entry is the earliest image.
    pub observations: Vec<ImageObservation>,
}

/// 10.1 Multi-Date Image Management.
///
/// Groups observations by acquisition date. The returned periods are always in
/// chronological order, and observations inside a period are ordered by capture
/// time, which makes downstream before/after selection deterministic.
///
/// # Example
///
/// ```ignore
/// let periods = group_by_acquisition_date(&observations);
/// // 2026-01-10 -> [image_01.tif]
/// // 2026-02-10 -> [image_02.tif, image_03.tif]
/// ```
pub fn group_by_acquisition_date(observations: &[ImageObservation]) -> Vec<ObservationPeriod> {
    let mut ordered = observations.to_vec();
    ordered.sort_by(|left, right| {
        (left.captured_at, left.image_id).cmp(&(right.captured_at, right.image_id))
    });

    ordered
        .into_iter()
        .fold(
            BTreeMap::<NaiveDate, Vec<ImageObservation>>::new(),
            |mut grouped, observation| {
                grouped
                    .entry(observation.acquisition_date())
                    .or_default()
                    .push(observation);
                grouped
            },
        )
        .into_iter()
        .map(|(date, observations)| ObservationPeriod { date, observations })
        .collect()
}

/// Result of a 10.2 before-and-after comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeforeAfterComparison {
    pub before: ImageObservation,
    pub after: ImageObservation,
    /// Number of days between the two acquisition dates (never negative).
    pub interval_days: i64,
}

/// 10.2 Before-and-After Comparison.
///
/// Validates that `before` was captured strictly earlier than `after`, then
/// returns the pair together with the observation interval.
pub fn compare_before_after(
    before: ImageObservation,
    after: ImageObservation,
) -> Result<BeforeAfterComparison, TemporalChangeError> {
    if after.captured_at <= before.captured_at {
        return Err(TemporalChangeError::NonChronologicalPeriods {
            before: before.captured_at,
            after: after.captured_at,
        });
    }

    Ok(BeforeAfterComparison {
        interval_days: (after.acquisition_date() - before.acquisition_date()).num_days(),
        before,
        after,
    })
}

/// 10.2 Before-and-After Comparison using the two most recent periods.
///
/// Convenience wrapper around [`compare_before_after`] for the common case
/// where the user only picks a session and the module compares the latest two
/// acquisition dates. Within each period the latest captured image is used.
pub fn latest_before_after(
    observations: &[ImageObservation],
) -> Result<BeforeAfterComparison, TemporalChangeError> {
    if observations.is_empty() {
        return Err(TemporalChangeError::EmptyObservations);
    }

    let periods = group_by_acquisition_date(observations);
    if periods.len() < 2 {
        return Err(TemporalChangeError::InsufficientPeriods {
            found: periods.len(),
        });
    }

    let latest_of = |period: &ObservationPeriod| {
        period
            .observations
            .iter()
            .max_by(|left, right| left.captured_at.cmp(&right.captured_at))
            .cloned()
    };

    let mut recent = periods.iter().rev();
    let after = recent.next().and_then(latest_of);
    let before = recent.next().and_then(latest_of);

    match (before, after) {
        (Some(before), Some(after)) => compare_before_after(before, after),
        _ => Err(TemporalChangeError::EmptyObservations),
    }
}

/// 10.3 Vegetation Coverage Change.
///
/// `change` is expressed in percentage points, i.e. a direct subtraction of
/// the two coverage percentages (60% -> 72% is `+12.0`, not `+20%`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct VegetationCoverageChange {
    pub before: f64,
    pub after: f64,
    pub change: f64,
}

/// 10.3 Vegetation Coverage Change.
///
/// Both coverages must be finite numbers within `0.0..=100.0`, otherwise a
/// [`TemporalChangeError`] is returned instead of a silently wrong result.
pub fn calculate_vegetation_change(
    before_coverage: f64,
    after_coverage: f64,
) -> Result<VegetationCoverageChange, TemporalChangeError> {
    let before = validate_coverage(before_coverage)?;
    let after = validate_coverage(after_coverage)?;

    Ok(VegetationCoverageChange {
        before,
        after,
        change: after - before,
    })
}

impl VegetationCoverageChange {
    /// Maps the 10.3 result onto the Module 10.8 -> Module 11.5 contract, which
    /// expects the keys `before`, `after`, and `delta`.
    pub fn to_contract_json(self) -> serde_json::Value {
        serde_json::json!({
            "before": self.before,
            "after": self.after,
            "delta": self.change,
        })
    }
}

/// One land-cover class transition between two observation periods.
///
/// `location_id` is an opaque sample-location identifier: a plot id from
/// Module 3, a grid cell, or an area label. Module 10 compares the classes but
/// never interprets the identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandCoverChange {
    pub location_id: String,
    pub before: String,
    pub after: String,
    pub changed: bool,
}

/// 10.4 Land-Cover Change.
///
/// Compares the land-cover class of every location in both periods. Both maps
/// must cover exactly the same locations; otherwise the comparison would be
/// meaningless and [`TemporalChangeError::MismatchedLocations`] is returned.
///
/// Iterating a [`BTreeMap`] is sorted by key, so the output is ordered by
/// `location_id` just like the `sorted(before_classes)` of the prototype.
pub fn identify_land_cover_changes(
    before_classes: &BTreeMap<String, String>,
    after_classes: &BTreeMap<String, String>,
) -> Result<Vec<LandCoverChange>, TemporalChangeError> {
    ensure_same_locations(before_classes, after_classes)?;

    Ok(before_classes
        .iter()
        .filter_map(|(location_id, before)| {
            after_classes.get(location_id).map(|after| LandCoverChange {
                location_id: location_id.clone(),
                before: before.clone(),
                after: after.clone(),
                changed: before != after,
            })
        })
        .collect())
}

/// 10.4 helper: only the locations whose class actually changed.
pub fn changed_areas(results: &[LandCoverChange]) -> Vec<&LandCoverChange> {
    results.iter().filter(|change| change.changed).collect()
}

/// Direction of a tree-count change, using the labels Module 11 expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TreeCountDirection {
    #[serde(rename = "bertambah")]
    Increased,
    #[serde(rename = "berkurang")]
    Decreased,
    #[serde(rename = "tetap")]
    Unchanged,
}

impl TreeCountDirection {
    /// Indonesian label used by the UI and the 10.8 summary.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Increased => "bertambah",
            Self::Decreased => "berkurang",
            Self::Unchanged => "tetap",
        }
    }
}

/// 10.5 Tree Count Change for one location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeCountChange {
    pub location_id: String,
    pub before: u32,
    pub after: u32,
    /// `after - before`; negative when trees disappeared.
    pub change: i64,
    pub direction: TreeCountDirection,
}

/// 10.5 Tree Count Change.
///
/// Compares the number of detected trees per location between two periods.
///
/// The prototype also rejects negative and non-integer counts. Those values are
/// unrepresentable in `u32`, and Serde rejects them before this function is
/// reached, so the only runtime check left is that both periods cover the same
/// locations.
pub fn calculate_tree_count_changes(
    before_counts: &BTreeMap<String, u32>,
    after_counts: &BTreeMap<String, u32>,
) -> Result<Vec<TreeCountChange>, TemporalChangeError> {
    ensure_same_locations(before_counts, after_counts)?;

    Ok(before_counts
        .iter()
        .filter_map(|(location_id, before)| {
            after_counts.get(location_id).map(|after| {
                let change = i64::from(*after) - i64::from(*before);
                TreeCountChange {
                    location_id: location_id.clone(),
                    before: *before,
                    after: *after,
                    change,
                    direction: match change {
                        change if change > 0 => TreeCountDirection::Increased,
                        change if change < 0 => TreeCountDirection::Decreased,
                        _ => TreeCountDirection::Unchanged,
                    },
                }
            })
        })
        .collect())
}

/// Magnitude of a change, bucketed with the prototype's thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MagnitudeLevel {
    #[serde(rename = "rendah")]
    Low,
    #[serde(rename = "sedang")]
    Medium,
    #[serde(rename = "tinggi")]
    High,
}

/// Smallest magnitude classified as [`MagnitudeLevel::Medium`].
pub const MEDIUM_MAGNITUDE: f64 = 5.0;
/// Smallest magnitude classified as [`MagnitudeLevel::High`].
pub const HIGH_MAGNITUDE: f64 = 15.0;

/// Direction of a magnitude change, using the labels Module 11 expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MagnitudeDirection {
    #[serde(rename = "meningkat")]
    Increased,
    #[serde(rename = "menurun")]
    Decreased,
    #[serde(rename = "tetap")]
    Unchanged,
}

impl MagnitudeDirection {
    /// Indonesian label used by the UI and the 10.8 summary.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Increased => "meningkat",
            Self::Decreased => "menurun",
            Self::Unchanged => "tetap",
        }
    }
}

/// 10.6 Change Magnitude for one location.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeMagnitudeEntry {
    pub location_id: String,
    pub before: f64,
    pub after: f64,
    /// `after - before`, rounded to two decimals.
    pub change: f64,
    /// `abs(after - before)`, rounded to two decimals.
    pub magnitude: f64,
    pub direction: MagnitudeDirection,
    pub level: MagnitudeLevel,
}

/// 10.6 helper: `rendah` / `sedang` / `tinggi` for one magnitude.
pub fn classify_change_magnitude(magnitude: f64) -> Result<MagnitudeLevel, TemporalChangeError> {
    let magnitude = ensure_finite(magnitude)?;
    if magnitude < 0.0 {
        return Err(TemporalChangeError::NegativeMagnitude { value: magnitude });
    }

    Ok(match magnitude {
        value if value >= HIGH_MAGNITUDE => MagnitudeLevel::High,
        value if value >= MEDIUM_MAGNITUDE => MagnitudeLevel::Medium,
        _ => MagnitudeLevel::Low,
    })
}

/// 10.6 Change Magnitude Analysis.
///
/// Works on any non-negative metric (coverage percent, area, tree count) and
/// returns, per location, the signed change, its magnitude, the direction, and
/// the magnitude level. Change and magnitude are rounded to two decimals like
/// the prototype; the level is classified from the raw difference so the
/// rounding can never move an observation across a threshold.
pub fn calculate_change_magnitudes(
    before_values: &BTreeMap<String, f64>,
    after_values: &BTreeMap<String, f64>,
) -> Result<Vec<ChangeMagnitudeEntry>, TemporalChangeError> {
    ensure_same_locations(before_values, after_values)?;

    before_values
        .iter()
        .filter_map(|(location_id, before)| {
            after_values
                .get(location_id)
                .map(|after| (location_id, *before, *after))
        })
        .map(|(location_id, before, after)| {
            let before = ensure_observation_value(location_id, before)?;
            let after = ensure_observation_value(location_id, after)?;

            let raw_change = after - before;
            let raw_magnitude = raw_change.abs();

            Ok(ChangeMagnitudeEntry {
                location_id: location_id.clone(),
                before,
                after,
                change: round_to_2(raw_change),
                magnitude: round_to_2(raw_magnitude),
                direction: match raw_change {
                    change if change > 0.0 => MagnitudeDirection::Increased,
                    change if change < 0.0 => MagnitudeDirection::Decreased,
                    _ => MagnitudeDirection::Unchanged,
                },
                level: classify_change_magnitude(raw_magnitude)?,
            })
        })
        .collect()
}

/// Default `threshold` of 10.7, matching the prototype's keyword default.
pub const DEFAULT_SIGNIFICANCE_THRESHOLD: f64 = 10.0;

/// 10.7 Change Area Detection result for one location.
///
/// The fields are flattened, so the JSON looks exactly like the prototype's
/// `{**item, "significant": ...}` dictionaries.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignificantChangeArea {
    #[serde(flatten)]
    pub change: ChangeMagnitudeEntry,
    /// `true` when the location's magnitude reaches the threshold.
    pub significant: bool,
}

/// 10.7 Change Area Detection.
///
/// Runs the 10.6 analysis and flags every location whose magnitude is at least
/// `threshold` (inclusive, like the prototype's `>=`). Non-negative,
/// finite thresholds only; anything else is an error rather than a silent
/// "everything changed".
pub fn detect_significant_change_areas(
    before_values: &BTreeMap<String, f64>,
    after_values: &BTreeMap<String, f64>,
    threshold: f64,
) -> Result<Vec<SignificantChangeArea>, TemporalChangeError> {
    let threshold = ensure_finite(threshold)?;
    if threshold < 0.0 {
        return Err(TemporalChangeError::NegativeThreshold { value: threshold });
    }

    Ok(calculate_change_magnitudes(before_values, after_values)?
        .into_iter()
        .map(|change| SignificantChangeArea {
            significant: change.magnitude >= threshold,
            change,
        })
        .collect())
}

/// 10.7 helper: only the locations flagged as significant.
pub fn significant_areas(results: &[SignificantChangeArea]) -> Vec<&SignificantChangeArea> {
    results.iter().filter(|area| area.significant).collect()
}

fn ensure_finite(value: f64) -> Result<f64, TemporalChangeError> {
    match value {
        v if !v.is_finite() => Err(TemporalChangeError::NonFiniteValue { value: v }),
        v => Ok(v),
    }
}

fn validate_coverage(value: f64) -> Result<f64, TemporalChangeError> {
    let value = ensure_finite(value)?;
    match value {
        v if !(0.0..=100.0).contains(&v) => {
            Err(TemporalChangeError::CoverageOutOfRange { value: v })
        }
        v => Ok(v),
    }
}

/// Both periods must describe the same locations, otherwise per-location
/// comparison would silently drop or invent data.
fn ensure_same_locations<T>(
    before: &BTreeMap<String, T>,
    after: &BTreeMap<String, T>,
) -> Result<(), TemporalChangeError> {
    let only_before: Vec<String> = before
        .keys()
        .filter(|location_id| !after.contains_key(*location_id))
        .cloned()
        .collect();
    let only_after: Vec<String> = after
        .keys()
        .filter(|location_id| !before.contains_key(*location_id))
        .cloned()
        .collect();

    if only_before.is_empty() && only_after.is_empty() {
        Ok(())
    } else {
        Err(TemporalChangeError::MismatchedLocations {
            only_before,
            only_after,
        })
    }
}

fn ensure_observation_value(location_id: &str, value: f64) -> Result<f64, TemporalChangeError> {
    let value = ensure_finite(value)?;
    if value < 0.0 {
        return Err(TemporalChangeError::NegativeObservationValue {
            location_id: location_id.to_string(),
            value,
        });
    }
    Ok(value)
}

/// Rounds to two decimals, matching the prototype's `round(value, 2)`.
fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round_ties_even() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("tanggal uji harus valid")
    }

    fn at(day: u32, hour: u32) -> NaiveDateTime {
        date(2026, 1, day)
            .and_hms_opt(hour, 0, 0)
            .expect("jam uji harus valid")
    }

    fn observation(path: &str, day: u32, hour: u32) -> ImageObservation {
        ImageObservation::new(Uuid::new_v4(), path, at(day, hour))
    }

    #[test]
    fn groups_observations_by_date_in_chronological_order() {
        let observations = vec![
            observation("data/image_02.tif", 20, 9),
            observation("data/image_01.tif", 10, 7),
            observation("data/image_03.tif", 20, 11),
        ];

        let periods = group_by_acquisition_date(&observations);

        let dates: Vec<_> = periods.iter().map(|period| period.date).collect();
        assert_eq!(dates, vec![date(2026, 1, 10), date(2026, 1, 20)]);
        let paths: Vec<_> = periods[1]
            .observations
            .iter()
            .map(|observation| observation.file_path.as_str())
            .collect();
        assert_eq!(paths, vec!["data/image_02.tif", "data/image_03.tif"]);
    }

    #[test]
    fn keeps_real_metadata_on_each_observation() {
        let observation = observation("data/image_01.tif", 10, 7).with_metadata(
            Some(-6.973_743),
            Some(107.630_417),
            Some(768.5),
            Some(5472),
            Some(3648),
        );

        assert_eq!(observation.acquisition_date(), date(2026, 1, 10));
        assert_eq!(observation.latitude, Some(-6.973_743));
        assert_eq!(observation.longitude, Some(107.630_417));
        assert_eq!(observation.altitude_m, Some(768.5));
        assert_eq!(observation.width, Some(5472));
        assert_eq!(observation.height, Some(3648));
        assert!(observation.has_location());
    }

    #[test]
    fn observation_without_gps_is_not_locatable() {
        let observation = observation("data/image_01.tif", 10, 7);

        assert!(!observation.has_location());
        assert_eq!(observation.width, None);
    }

    #[test]
    fn compares_before_and_after_with_interval() {
        let result = compare_before_after(
            observation("data/image_01.tif", 10, 7),
            observation("data/image_02.tif", 20, 8),
        )
        .expect("rentang tanggal valid");

        assert_eq!(result.interval_days, 10);
        assert_eq!(result.before.file_path, "data/image_01.tif");
        assert_eq!(result.after.file_path, "data/image_02.tif");
    }

    #[test]
    fn rejects_reversed_before_and_after() {
        let result = compare_before_after(
            observation("data/image_02.tif", 20, 8),
            observation("data/image_01.tif", 10, 7),
        );

        assert_eq!(
            result,
            Err(TemporalChangeError::NonChronologicalPeriods {
                before: at(20, 8),
                after: at(10, 7),
            })
        );
    }

    #[test]
    fn rejects_same_capture_time() {
        let result = compare_before_after(
            observation("data/image_01.tif", 10, 7),
            observation("data/image_02.tif", 10, 7),
        );

        assert!(matches!(
            result,
            Err(TemporalChangeError::NonChronologicalPeriods { .. })
        ));
    }

    #[test]
    fn picks_the_two_most_recent_periods_with_latest_image_each() {
        let observations = vec![
            observation("data/image_01.tif", 10, 7),
            observation("data/image_02.tif", 15, 7),
            observation("data/image_03.tif", 20, 8),
            observation("data/image_04.tif", 20, 12),
        ];

        let result = latest_before_after(&observations).expect("ada dua periode");

        assert_eq!(result.before.file_path, "data/image_02.tif");
        assert_eq!(result.after.file_path, "data/image_04.tif");
        assert_eq!(result.interval_days, 5);
    }

    #[test]
    fn needs_two_periods_for_before_and_after() {
        let observations = vec![
            observation("data/image_01.tif", 10, 7),
            observation("data/image_02.tif", 10, 11),
        ];

        assert_eq!(
            latest_before_after(&observations),
            Err(TemporalChangeError::InsufficientPeriods { found: 1 })
        );
    }

    #[test]
    fn empty_observations_are_rejected() {
        assert_eq!(
            latest_before_after(&[]),
            Err(TemporalChangeError::EmptyObservations)
        );
    }

    #[test]
    fn calculates_vegetation_change_in_percentage_points() {
        let result = calculate_vegetation_change(60.0, 72.0).expect("nilai valid");

        assert_eq!(result.before, 60.0);
        assert_eq!(result.after, 72.0);
        assert_eq!(result.change, 12.0);
    }

    #[test]
    fn negative_vegetation_change_is_supported() {
        let result = calculate_vegetation_change(72.0, 60.0).expect("nilai valid");

        assert_eq!(result.change, -12.0);
    }

    #[test]
    fn rejects_out_of_range_coverage() {
        assert_eq!(
            calculate_vegetation_change(60.0, 120.0),
            Err(TemporalChangeError::CoverageOutOfRange { value: 120.0 })
        );
        assert_eq!(
            calculate_vegetation_change(-1.0, 72.0),
            Err(TemporalChangeError::CoverageOutOfRange { value: -1.0 })
        );
    }

    #[test]
    fn rejects_non_finite_coverage() {
        // `NaN != NaN`, so match on the variant instead of comparing values.
        assert!(matches!(
            calculate_vegetation_change(f64::NAN, 72.0),
            Err(TemporalChangeError::NonFiniteValue { .. })
        ));
        assert_eq!(
            calculate_vegetation_change(60.0, f64::INFINITY),
            Err(TemporalChangeError::NonFiniteValue {
                value: f64::INFINITY
            })
        );
    }

    #[test]
    fn vegetation_change_matches_contract_json() {
        let payload = calculate_vegetation_change(45.0, 52.0)
            .expect("nilai valid")
            .to_contract_json();

        assert_eq!(
            payload,
            serde_json::json!({ "before": 45.0, "after": 52.0, "delta": 7.0 })
        );
    }

    fn classes(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(location_id, class)| (location_id.to_string(), class.to_string()))
            .collect()
    }

    fn counts(entries: &[(&str, u32)]) -> BTreeMap<String, u32> {
        entries
            .iter()
            .map(|(location_id, count)| (location_id.to_string(), *count))
            .collect()
    }

    fn values(entries: &[(&str, f64)]) -> BTreeMap<String, f64> {
        entries
            .iter()
            .map(|(location_id, value)| (location_id.to_string(), *value))
            .collect()
    }

    #[test]
    fn identifies_land_cover_changes_per_location() {
        let before = classes(&[
            ("area-A", "vegetation"),
            ("area-B", "water"),
            ("area-C", "bare_soil"),
        ]);
        let after = classes(&[
            ("area-A", "built_up"),
            ("area-B", "water"),
            ("area-C", "vegetation"),
        ]);

        let results = identify_land_cover_changes(&before, &after).expect("lokasi sama");

        assert_eq!(
            results,
            vec![
                LandCoverChange {
                    location_id: "area-A".to_string(),
                    before: "vegetation".to_string(),
                    after: "built_up".to_string(),
                    changed: true,
                },
                LandCoverChange {
                    location_id: "area-B".to_string(),
                    before: "water".to_string(),
                    after: "water".to_string(),
                    changed: false,
                },
                LandCoverChange {
                    location_id: "area-C".to_string(),
                    before: "bare_soil".to_string(),
                    after: "vegetation".to_string(),
                    changed: true,
                },
            ]
        );

        let changed: Vec<&str> = changed_areas(&results)
            .iter()
            .map(|change| change.location_id.as_str())
            .collect();
        assert_eq!(changed, vec!["area-A", "area-C"]);
    }

    #[test]
    fn land_cover_comparison_rejects_different_locations() {
        let error = identify_land_cover_changes(
            &classes(&[("area-A", "vegetation")]),
            &classes(&[("area-B", "water")]),
        )
        .expect_err("lokasi berbeda");

        assert_eq!(
            error,
            TemporalChangeError::MismatchedLocations {
                only_before: vec!["area-A".to_string()],
                only_after: vec!["area-B".to_string()],
            }
        );
    }

    #[test]
    fn land_cover_comparison_does_not_mutate_inputs() {
        let before = classes(&[("area-A", "vegetation")]);
        let after = classes(&[("area-A", "built_up")]);
        let before_original = before.clone();
        let after_original = after.clone();

        identify_land_cover_changes(&before, &after).expect("lokasi sama");

        assert_eq!(before, before_original);
        assert_eq!(after, after_original);
    }

    #[test]
    fn land_cover_comparison_reports_no_changes() {
        let results = identify_land_cover_changes(
            &classes(&[("area-A", "water")]),
            &classes(&[("area-A", "water")]),
        )
        .expect("lokasi sama");

        assert!(changed_areas(&results).is_empty());
    }

    #[test]
    fn land_cover_change_serializes_to_camel_case() {
        let results = identify_land_cover_changes(
            &classes(&[("area-A", "vegetation")]),
            &classes(&[("area-A", "built_up")]),
        )
        .expect("lokasi sama");

        assert_eq!(
            serde_json::to_value(&results).expect("terserialisasi"),
            serde_json::json!([{
                "locationId": "area-A",
                "before": "vegetation",
                "after": "built_up",
                "changed": true
            }])
        );
    }

    #[test]
    fn calculates_tree_count_change_per_location() {
        let before = counts(&[("area-A", 120), ("area-B", 80), ("area-C", 50)]);
        let after = counts(&[("area-A", 105), ("area-B", 95), ("area-C", 50)]);

        let results = calculate_tree_count_changes(&before, &after).expect("lokasi sama");

        assert_eq!(
            results,
            vec![
                TreeCountChange {
                    location_id: "area-A".to_string(),
                    before: 120,
                    after: 105,
                    change: -15,
                    direction: TreeCountDirection::Decreased,
                },
                TreeCountChange {
                    location_id: "area-B".to_string(),
                    before: 80,
                    after: 95,
                    change: 15,
                    direction: TreeCountDirection::Increased,
                },
                TreeCountChange {
                    location_id: "area-C".to_string(),
                    before: 50,
                    after: 50,
                    change: 0,
                    direction: TreeCountDirection::Unchanged,
                },
            ]
        );
    }

    #[test]
    fn tree_count_comparison_rejects_different_locations() {
        let error =
            calculate_tree_count_changes(&counts(&[("area-A", 100)]), &counts(&[("area-B", 90)]))
                .expect_err("lokasi berbeda");

        assert!(matches!(
            error,
            TemporalChangeError::MismatchedLocations { .. }
        ));
    }

    #[test]
    fn tree_count_comparison_does_not_mutate_inputs() {
        let before = counts(&[("area-A", 120)]);
        let after = counts(&[("area-A", 105)]);
        let before_original = before.clone();
        let after_original = after.clone();

        calculate_tree_count_changes(&before, &after).expect("lokasi sama");

        assert_eq!(before, before_original);
        assert_eq!(after, after_original);
    }

    #[test]
    fn tree_counts_cannot_be_negative_or_fractional() {
        // The prototype raises ValueError / TypeError. In Rust the count type
        // is `u32`, so Serde rejects both payloads at the command boundary.
        assert!(serde_json::from_str::<BTreeMap<String, u32>>(r#"{"area-A": -5}"#).is_err());
        assert!(serde_json::from_str::<BTreeMap<String, u32>>(r#"{"area-A": 10.5}"#).is_err());
        assert!(serde_json::from_str::<BTreeMap<String, u32>>(r#"{"area-A": 10}"#).is_ok());
    }

    #[test]
    fn classifies_change_magnitude_with_prototype_thresholds() {
        assert_eq!(
            classify_change_magnitude(0.0).expect("nilai valid"),
            MagnitudeLevel::Low
        );
        assert_eq!(
            classify_change_magnitude(4.99).expect("nilai valid"),
            MagnitudeLevel::Low
        );
        assert_eq!(
            classify_change_magnitude(MEDIUM_MAGNITUDE).expect("nilai valid"),
            MagnitudeLevel::Medium
        );
        assert_eq!(
            classify_change_magnitude(14.99).expect("nilai valid"),
            MagnitudeLevel::Medium
        );
        assert_eq!(
            classify_change_magnitude(HIGH_MAGNITUDE).expect("nilai valid"),
            MagnitudeLevel::High
        );
        assert_eq!(
            classify_change_magnitude(100.0).expect("nilai valid"),
            MagnitudeLevel::High
        );
    }

    #[test]
    fn rejects_negative_magnitude() {
        assert_eq!(
            classify_change_magnitude(-1.0),
            Err(TemporalChangeError::NegativeMagnitude { value: -1.0 })
        );
    }

    #[test]
    fn rejects_non_finite_magnitude() {
        assert!(matches!(
            classify_change_magnitude(f64::NAN),
            Err(TemporalChangeError::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn calculates_change_magnitude_per_location() {
        let before = values(&[("area-A", 60.0), ("area-B", 50.0), ("area-C", 75.0)]);
        let after = values(&[("area-A", 72.0), ("area-B", 52.0), ("area-C", 55.0)]);

        let results = calculate_change_magnitudes(&before, &after).expect("lokasi sama");

        assert_eq!(
            results,
            vec![
                ChangeMagnitudeEntry {
                    location_id: "area-A".to_string(),
                    before: 60.0,
                    after: 72.0,
                    change: 12.0,
                    magnitude: 12.0,
                    direction: MagnitudeDirection::Increased,
                    level: MagnitudeLevel::Medium,
                },
                ChangeMagnitudeEntry {
                    location_id: "area-B".to_string(),
                    before: 50.0,
                    after: 52.0,
                    change: 2.0,
                    magnitude: 2.0,
                    direction: MagnitudeDirection::Increased,
                    level: MagnitudeLevel::Low,
                },
                ChangeMagnitudeEntry {
                    location_id: "area-C".to_string(),
                    before: 75.0,
                    after: 55.0,
                    change: -20.0,
                    magnitude: 20.0,
                    direction: MagnitudeDirection::Decreased,
                    level: MagnitudeLevel::High,
                },
            ]
        );
    }

    #[test]
    fn rounds_change_and_magnitude_to_two_decimals() {
        let results = calculate_change_magnitudes(
            &values(&[("area-A", 10.0)]),
            &values(&[("area-A", 10.126)]),
        )
        .expect("lokasi sama");

        assert_eq!(results[0].change, 0.13);
        assert_eq!(results[0].magnitude, 0.13);
    }

    #[test]
    fn magnitude_analysis_rejects_negative_observation_values() {
        let error = calculate_change_magnitudes(
            &values(&[("area-A", -10.0)]),
            &values(&[("area-A", 20.0)]),
        )
        .expect_err("nilai negatif");

        assert_eq!(
            error,
            TemporalChangeError::NegativeObservationValue {
                location_id: "area-A".to_string(),
                value: -10.0,
            }
        );
    }

    #[test]
    fn magnitude_analysis_rejects_non_finite_values() {
        assert!(matches!(
            calculate_change_magnitudes(
                &values(&[("area-A", 60.0)]),
                &values(&[("area-A", f64::NAN)])
            ),
            Err(TemporalChangeError::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn magnitude_analysis_rejects_different_locations() {
        assert!(matches!(
            calculate_change_magnitudes(&values(&[("area-A", 60.0)]), &values(&[("area-B", 70.0)])),
            Err(TemporalChangeError::MismatchedLocations { .. })
        ));
    }

    #[test]
    fn detects_significant_change_areas() {
        let before = values(&[("area-A", 60.0), ("area-B", 50.0), ("area-C", 75.0)]);
        let after = values(&[("area-A", 72.0), ("area-B", 52.0), ("area-C", 55.0)]);

        let results =
            detect_significant_change_areas(&before, &after, DEFAULT_SIGNIFICANCE_THRESHOLD)
                .expect("lokasi sama");

        let significant: Vec<&str> = significant_areas(&results)
            .iter()
            .map(|area| area.change.location_id.as_str())
            .collect();
        assert_eq!(significant, vec!["area-A", "area-C"]);
        assert!(!results[1].significant);
    }

    #[test]
    fn significance_threshold_is_inclusive() {
        let results = detect_significant_change_areas(
            &values(&[("area-A", 50.0)]),
            &values(&[("area-A", 60.0)]),
            10.0,
        )
        .expect("lokasi sama");

        assert!(results[0].significant);
    }

    #[test]
    fn rejects_negative_threshold() {
        assert_eq!(
            detect_significant_change_areas(&values(&[]), &values(&[]), -5.0),
            Err(TemporalChangeError::NegativeThreshold { value: -5.0 })
        );
    }

    #[test]
    fn reports_no_significant_changes() {
        let results = detect_significant_change_areas(
            &values(&[("area-A", 50.0)]),
            &values(&[("area-A", 52.0)]),
            DEFAULT_SIGNIFICANCE_THRESHOLD,
        )
        .expect("lokasi sama");

        assert!(significant_areas(&results).is_empty());
    }

    #[test]
    fn significant_areas_serialize_flattened() {
        let results = detect_significant_change_areas(
            &values(&[("area-A", 50.0)]),
            &values(&[("area-A", 60.0)]),
            DEFAULT_SIGNIFICANCE_THRESHOLD,
        )
        .expect("lokasi sama");

        assert_eq!(
            serde_json::to_value(&results).expect("terserialisasi"),
            serde_json::json!([{
                "locationId": "area-A",
                "before": 50.0,
                "after": 60.0,
                "change": 10.0,
                "magnitude": 10.0,
                "direction": "meningkat",
                "level": "sedang",
                "significant": true
            }])
        );
    }
}
