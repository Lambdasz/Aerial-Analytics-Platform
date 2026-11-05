#![allow(dead_code)]

//! Adapter from Module 1.4 persisted records to Module 10 observations.
//!
//! Module 1.4 stores one [`Image`] row per imported file, with the EXIF fields
//! Module 10 needs kept as nullable columns. This module turns those rows into
//! [`ImageObservation`] values and reports - instead of silently dropping - the
//! images that cannot take part in a temporal comparison:
//!
//! | Reason | Meaning |
//! |---|---|
//! | [`ObservationSkipReason::MissingCaptureTime`] | No EXIF `DateTimeOriginal`, so the image cannot be assigned to an observation period. |
//! | [`ObservationSkipReason::NotAnalysisReady`] | Module 1 marked the row as unusable for plugins (e.g. a DNG with no same-stem JPEG). |
//!
//! The adapter is pure: it reads the records it is given and never touches the
//! database or the filesystem. That keeps the mapping testable without a
//! database and lets the command layer decide when to query.

use serde::Serialize;
use uuid::Uuid;

use crate::models::Image;

use super::analysis::ImageObservation;

/// Why an image was left out of the temporal analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSkipReason {
    MissingCaptureTime,
    NotAnalysisReady,
}

/// An image that could not become an observation, kept so the UI can explain
/// why a session has fewer usable images than expected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedImage {
    pub image_id: Uuid,
    pub file_path: String,
    pub reason: ObservationSkipReason,
}

/// The result of mapping a session's stored images.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationCollection {
    /// Usable observations, ordered by capture time then image id.
    pub observations: Vec<ImageObservation>,
    /// Images that had to be skipped, ordered by file path.
    pub skipped: Vec<SkippedImage>,
}

/// Converts one stored image into an observation.
///
/// `analysis_ready: Some(false)` is rejected before capture time is checked,
/// because such a row cannot be analysed even if its metadata is complete.
/// `analysis_ready: None` (rows migrated from before copy-on-import) is
/// accepted, matching Module 1's documented fallback to `format`.
pub fn observation_from_image(image: &Image) -> Result<ImageObservation, ObservationSkipReason> {
    if image.analysis_ready == Some(false) {
        return Err(ObservationSkipReason::NotAnalysisReady);
    }

    let captured_at = image
        .captured_at
        .ok_or(ObservationSkipReason::MissingCaptureTime)?;

    Ok(
        ImageObservation::new(image.id, image.file_path.clone(), captured_at).with_metadata(
            image.location_lat,
            image.location_lon,
            image.gps_altitude_m,
            image.width,
            image.height,
        ),
    )
}

/// Converts every stored image of a session into an [`ObservationCollection`].
///
/// Ordering is deterministic so grouping by date and picking a representative
/// image produce the same result on every run.
pub fn observations_from_images(images: &[Image]) -> ObservationCollection {
    let (mut observations, mut skipped) = images.iter().fold(
        (Vec::new(), Vec::new()),
        |(mut observations, mut skipped), image| {
            match observation_from_image(image) {
                Ok(observation) => observations.push(observation),
                Err(reason) => skipped.push(SkippedImage {
                    image_id: image.id,
                    file_path: image.file_path.clone(),
                    reason,
                }),
            }
            (observations, skipped)
        },
    );

    observations.sort_by(|left, right| {
        (left.captured_at, left.image_id).cmp(&(right.captured_at, right.image_id))
    });
    skipped.sort_by(|left, right| left.file_path.cmp(&right.file_path));

    ObservationCollection {
        observations,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};
    use uuid::Uuid;

    use super::*;

    fn at(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 3, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .expect("tanggal uji harus valid")
    }

    /// Minimal stored image; fields Module 10 ignores are set to `None`.
    fn image(id: Uuid, path: &str, captured_at: Option<NaiveDateTime>) -> Image {
        Image {
            id,
            session_id: Uuid::nil(),
            file_path: path.to_string(),
            location_lat: Some(-6.973_743),
            location_lon: Some(107.630_417),
            captured_at,
            gps_altitude_m: Some(768.5),
            width: Some(5472),
            height: Some(3648),
            format: Some("jpeg".to_string()),
            make: Some("DJI".to_string()),
            camera_model: Some("Mavic 3".to_string()),
            source_path: None,
            content_hash: None,
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: Some(true),
            blur_score: None,
            is_blurry: None,
            heading_deg: None,
        }
    }

    #[test]
    fn maps_stored_image_to_observation_with_real_metadata() {
        let id = Uuid::new_v4();
        let observation = observation_from_image(&image(id, "C:/library/a.jpg", Some(at(10, 7))))
            .expect("citra punya waktu pengambilan");

        assert_eq!(observation.image_id, id);
        assert_eq!(observation.file_path, "C:/library/a.jpg");
        assert_eq!(observation.captured_at, at(10, 7));
        assert_eq!(observation.acquisition_date(), at(10, 7).date());
        assert_eq!(observation.latitude, Some(-6.973_743));
        assert_eq!(observation.longitude, Some(107.630_417));
        assert_eq!(observation.altitude_m, Some(768.5));
        assert_eq!(observation.width, Some(5472));
        assert_eq!(observation.height, Some(3648));
        assert!(observation.has_location());
    }

    #[test]
    fn skips_image_without_capture_time() {
        let skipped_id = Uuid::new_v4();
        let usable_id = Uuid::new_v4();
        let collection = observations_from_images(&[
            image(skipped_id, "C:/library/b.jpg", None),
            image(usable_id, "C:/library/a.jpg", Some(at(10, 7))),
        ]);

        assert_eq!(collection.observations.len(), 1);
        assert_eq!(collection.observations[0].image_id, usable_id);
        assert_eq!(
            collection.skipped,
            vec![SkippedImage {
                image_id: skipped_id,
                file_path: "C:/library/b.jpg".to_string(),
                reason: ObservationSkipReason::MissingCaptureTime,
            }]
        );
    }

    #[test]
    fn skips_image_module_one_marks_as_not_analysis_ready() {
        let mut not_ready = image(Uuid::new_v4(), "C:/library/c.dng", Some(at(10, 7)));
        not_ready.analysis_ready = Some(false);

        let collection = observations_from_images(&[not_ready]);

        assert!(collection.observations.is_empty());
        assert_eq!(
            collection.skipped[0].reason,
            ObservationSkipReason::NotAnalysisReady
        );
    }

    #[test]
    fn accepts_migrated_rows_with_unknown_analysis_readiness() {
        let mut migrated = image(Uuid::new_v4(), "C:/library/old.jpg", Some(at(10, 7)));
        migrated.analysis_ready = None;

        let collection = observations_from_images(&[migrated]);

        assert_eq!(collection.observations.len(), 1);
        assert!(collection.skipped.is_empty());
    }

    #[test]
    fn orders_observations_by_capture_time() {
        let early = Uuid::new_v4();
        let late = Uuid::new_v4();
        let collection = observations_from_images(&[
            image(late, "C:/library/z.jpg", Some(at(10, 12))),
            image(early, "C:/library/a.jpg", Some(at(10, 7))),
        ]);

        let ids: Vec<_> = collection
            .observations
            .iter()
            .map(|observation| observation.image_id)
            .collect();
        assert_eq!(ids, vec![early, late]);
    }

    #[test]
    fn keeps_images_with_incomplete_gps_but_reports_them_as_not_locatable() {
        let mut partial = image(Uuid::new_v4(), "C:/library/partial.jpg", Some(at(10, 7)));
        partial.location_lat = Some(-6.9);
        partial.location_lon = None;
        partial.gps_altitude_m = None;
        partial.width = None;
        partial.height = None;

        let collection = observations_from_images(&[partial]);

        let observation = &collection.observations[0];
        assert!(!observation.has_location());
        assert_eq!(observation.altitude_m, None);
        assert_eq!(observation.width, None);
    }
}
