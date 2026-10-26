//! Blur Detection (Module 1.5): Laplacian-variance sharpness scoring,
//! classified relative to the flight session an image belongs to.
//!
//! [`compute_blur_score`] is pure: a 3x3 Laplacian convolution over a
//! grayscale buffer, followed by the variance of the response. Scoring a
//! file end-to-end ([`blur_score_for_copy`]) downscales first (never
//! upscales — see below) so the score is comparable across source
//! resolutions, and never fails its caller — a corrupt or undecodable file
//! simply yields `None`, the same fallback philosophy
//! [`super::import_io::metadata_from_copy`] uses for EXIF extraction
//! failures.
//!
//! # Why only downscale, never upscale
//!
//! Interpolating a small source up to [`BLUR_SCORE_LONG_EDGE_PX`] smooths
//! away exactly the high-frequency edges the Laplacian measures, so an
//! upscaled image scores as falsely blurry regardless of its real
//! sharpness. A DNG's embedded preview ([`super::dng_preview`]) is often
//! smaller than the scoring cap, and small JPEGs hit this too. A source
//! already below the cap is therefore left unscored (`None`) rather than
//! upscaled into a misleading number.
//!
//! # Why session-relative, not a single global threshold
//!
//! Laplacian variance alone can't tell a sharp, low-texture aerial photo
//! (farmland, open water) from a genuinely blurry one — both have few sharp
//! edges. But images from the same flight session share similar terrain and
//! altitude, so comparing a score against the mean/stddev of its own
//! session's other scores ([`classify_blur`]) is a self-calibrating
//! baseline. [`HARD_FLOOR_BLUR_SCORE`] remains underneath as an absolute
//! safety net for a session where every image is bad.
//!
//! # Why classification is a whole-session pass, not per-image at insert time
//!
//! `blur_score` is still computed once per image, at import
//! ([`blur_score_for_copy`], called from
//! [`super::import_io::ImportScope::single`]/
//! [`super::import_io::ImportScope::pair`]). But `is_blurry` is *not* set
//! from whatever scores happened to exist in the database at that moment —
//! that would make the flag depend on import order and on how many separate
//! import runs it took to fill the session: the same file could land as
//! blurry or not depending on whether it happened to be the 1st or the 11th
//! image scored. Instead, [`super::flight_session::reclassify_session_blur`]
//! re-derives `is_blurry` for every scored image in the session in one pass
//! ([`classify_against_session`] is the classifier it's given), run once at
//! the end of each import ([`super::import_io::import_planned`]). The result
//! depends only on the session's current score set, not on how it was
//! assembled.

use std::path::Path;

use image::GrayImage;

use crate::models::image::ImageFormat;

/// Long edge (px) images are downscaled to before scoring, so raw variance
/// is comparable across source resolutions. Sources smaller than this are
/// not scored at all: upscaling smooths away the edges the score measures.
pub(crate) const BLUR_SCORE_LONG_EDGE_PX: u32 = 1280;

/// Absolute safety net: a score below this is always classified as blurry,
/// regardless of how it compares to its session. Catches a session where
/// every image is bad (nothing to be relatively sharper than). Tuned
/// against real sample flight sessions, not a generic value — aerial
/// low-texture scenes score lower than textured ones at the same actual
/// sharpness, so this floor must stay well below a typical sharp score.
pub(crate) const HARD_FLOOR_BLUR_SCORE: f64 = 20.0;

/// How many standard deviations below its session's mean a score must fall
/// to be classified as a relative outlier (blurry). More negative is
/// stricter (fewer images flagged).
pub(crate) const BLUR_Z_THRESHOLD: f64 = -1.5;

/// Minimum number of *other* scored images in the session (excluding the one
/// being classified — see [`classify_blur`]) before relative (z-score)
/// classification applies. Below this, only the hard floor applies: a
/// mean/stddev computed from too few points is not a meaningful baseline.
pub(crate) const MIN_SESSION_SAMPLE_FOR_RELATIVE: usize = 5;

/// Laplacian-variance sharpness score. Higher means sharper (more and/or
/// stronger edges); lower means blurrier.
///
/// Edge pixels are handled by clamping the 3x3 window to the image bounds
/// (nearest-neighbour extension), so every pixel — including the border —
/// contributes one response value.
///
/// # Purity
///
/// Pure, total (never fails).
pub(crate) fn compute_blur_score(gray: &GrayImage) -> f64 {
    let (width, height) = gray.dimensions();
    if width == 0 || height == 0 {
        return 0.0;
    }
    let pixel = |x: i64, y: i64| -> f64 {
        let x = x.clamp(0, width as i64 - 1) as u32;
        let y = y.clamp(0, height as i64 - 1) as u32;
        f64::from(gray.get_pixel(x, y).0[0])
    };

    let mut responses = Vec::with_capacity((width as usize) * (height as usize));
    for y in 0..height as i64 {
        for x in 0..width as i64 {
            // Standard 3x3 Laplacian kernel: [[0,1,0],[1,-4,1],[0,1,0]].
            let response = pixel(x, y - 1) + pixel(x, y + 1) + pixel(x - 1, y) + pixel(x + 1, y)
                - 4.0 * pixel(x, y);
            responses.push(response);
        }
    }
    variance(&responses)
}

/// Population variance of `values`. `0.0` for an empty or single-element
/// slice.
///
/// # Purity
///
/// Pure, total.
fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
}

/// Reads, decodes, and scores one imported copy, downscaling first when the
/// source is larger than [`BLUR_SCORE_LONG_EDGE_PX`].
///
/// Never fails its caller: any read or decode error is logged and yields
/// `None` so import always proceeds. DNG is scored from its embedded
/// preview ([`super::dng_preview::read_dng_preview`]) since no raw decoder
/// exists in this codebase; JPEG decodes directly.
///
/// A source whose long edge is already below the cap is left unscored
/// (`None`) rather than upscaled — see the module-level "why only
/// downscale" note.
///
/// # Purity
///
/// Impure — reads a file from disk.
pub(crate) fn blur_score_for_copy(path: &Path, format: ImageFormat) -> Option<f64> {
    let bytes = match format {
        ImageFormat::Jpeg => std::fs::read(path).ok(),
        ImageFormat::Dng => super::dng_preview::read_dng_preview(path).ok(),
    };
    let bytes = match bytes {
        Some(bytes) => bytes,
        None => {
            eprintln!("warning: could not read image bytes for blur scoring");
            return None;
        }
    };
    let decoded = match image::load_from_memory(&bytes) {
        Ok(decoded) => decoded,
        Err(_) => {
            eprintln!("warning: could not decode image for blur scoring");
            return None;
        }
    };
    if !meets_scoring_size(decoded.width(), decoded.height()) {
        return None;
    }
    let resized = decoded.resize(
        BLUR_SCORE_LONG_EDGE_PX,
        BLUR_SCORE_LONG_EDGE_PX,
        image::imageops::FilterType::Triangle,
    );
    Some(compute_blur_score(&resized.to_luma8()))
}

/// Whether a decoded image's long edge is already at or above
/// [`BLUR_SCORE_LONG_EDGE_PX`], i.e. scoring it only ever downscales.
///
/// # Purity
///
/// Pure, total.
fn meets_scoring_size(width: u32, height: u32) -> bool {
    width.max(height) >= BLUR_SCORE_LONG_EDGE_PX
}

/// Mean/stddev/count of a set of blur scores, used as a session's relative
/// baseline.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionBlurStats {
    pub mean: f64,
    pub stddev: f64,
    pub sample_count: usize,
}

/// Mean and population standard deviation over whatever scores are handed
/// in.
///
/// # Purity
///
/// Pure, total (never fails).
pub(crate) fn compute_session_blur_stats(scores: &[f64]) -> SessionBlurStats {
    let sample_count = scores.len();
    if sample_count == 0 {
        return SessionBlurStats {
            mean: 0.0,
            stddev: 0.0,
            sample_count,
        };
    }
    let mean = scores.iter().sum::<f64>() / sample_count as f64;
    SessionBlurStats {
        mean,
        stddev: variance(scores).sqrt(),
        sample_count,
    }
}

/// Classifies one raw score as blurry, given its session's baseline stats.
///
/// `stats` must be built from the *other* scored images in the session,
/// excluding `raw_score` itself — see [`classify_against_session`]. Folding
/// the candidate into its own mean/stddev caps how far its z-score can ever
/// read (a population stddev including the point bounds `|z|` at
/// `(n-1)/sqrt(n)`), which quietly under-flags real outliers in small
/// sessions.
///
/// A score below `hard_floor` is always blurry. Otherwise, with fewer than
/// [`MIN_SESSION_SAMPLE_FOR_RELATIVE`] other samples or a zero stddev, there
/// is no meaningful baseline yet, so nothing beyond the hard floor is
/// flagged. Past that minimum, a z-score below `z_threshold` standard
/// deviations under the session mean is also classified as blurry.
///
/// # Purity
///
/// Pure, total.
pub(crate) fn classify_blur(
    raw_score: f64,
    stats: &SessionBlurStats,
    hard_floor: f64,
    z_threshold: f64,
) -> bool {
    if raw_score < hard_floor {
        return true;
    }
    if stats.sample_count < MIN_SESSION_SAMPLE_FOR_RELATIVE || stats.stddev == 0.0 {
        return false;
    }
    (raw_score - stats.mean) / stats.stddev < z_threshold
}

/// Classifies `score` against `others` (the rest of its session) with the
/// module's thresholds. Thin wrapper around [`classify_blur`] for callers
/// (the session-wide reclassification pass in
/// [`super::flight_session::reclassify_session_blur`]) that only have a raw
/// score and a baseline, not a pre-built [`SessionBlurStats`].
///
/// # Purity
///
/// Pure, total.
pub(crate) fn classify_against_session(score: f64, others: &[f64]) -> bool {
    classify_blur(
        score,
        &compute_session_blur_stats(others),
        HARD_FLOOR_BLUR_SCORE,
        BLUR_Z_THRESHOLD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(size: u32) -> GrayImage {
        GrayImage::from_fn(size, size, |x, y| {
            if (x / 2 + y / 2) % 2 == 0 {
                image::Luma([0])
            } else {
                image::Luma([255])
            }
        })
    }

    fn uniform(size: u32, value: u8) -> GrayImage {
        GrayImage::from_pixel(size, size, image::Luma([value]))
    }

    #[test]
    fn sharp_checkerboard_scores_higher_than_uniform_fill() {
        let sharp = compute_blur_score(&checkerboard(32));
        let blurred = compute_blur_score(&uniform(32, 128));
        assert!(
            sharp > blurred,
            "sharp={sharp} should exceed blurred={blurred}"
        );
        assert_eq!(blurred, 0.0, "a flat image has zero Laplacian response");
    }

    #[test]
    fn compute_blur_score_handles_degenerate_sizes() {
        assert_eq!(compute_blur_score(&GrayImage::new(0, 0)), 0.0);
        assert_eq!(compute_blur_score(&uniform(1, 200)), 0.0);
    }

    #[test]
    fn compute_session_blur_stats_empty_is_zeroed() {
        let stats = compute_session_blur_stats(&[]);
        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn compute_session_blur_stats_single_value_has_zero_stddev() {
        let stats = compute_session_blur_stats(&[42.0]);
        assert_eq!(stats.sample_count, 1);
        assert_eq!(stats.mean, 42.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn compute_session_blur_stats_known_values() {
        let stats = compute_session_blur_stats(&[10.0, 20.0, 30.0]);
        assert_eq!(stats.sample_count, 3);
        assert_eq!(stats.mean, 20.0);
        assert!((stats.stddev - 8.164_965_8).abs() < 1e-6);
    }

    #[test]
    fn classify_blur_hard_floor_short_circuits_regardless_of_stats() {
        let stats = SessionBlurStats {
            mean: 500.0,
            stddev: 50.0,
            sample_count: 50,
        };
        assert!(classify_blur(5.0, &stats, 20.0, -1.5));
    }

    #[test]
    fn classify_blur_below_min_sample_falls_back_to_hard_floor_only() {
        let stats = SessionBlurStats {
            mean: 100.0,
            stddev: 10.0,
            sample_count: MIN_SESSION_SAMPLE_FOR_RELATIVE - 1,
        };
        // Far below the mean, but not enough session data for a relative
        // judgment and above the hard floor.
        assert!(!classify_blur(50.0, &stats, 20.0, -1.5));
    }

    #[test]
    fn classify_blur_zero_stddev_falls_back_to_hard_floor_only() {
        let stats = SessionBlurStats {
            mean: 50.0,
            stddev: 0.0,
            sample_count: 10,
        };
        assert!(!classify_blur(10.0, &stats, 5.0, -1.5));
    }

    #[test]
    fn classify_blur_z_score_outlier_is_blurry() {
        let stats = SessionBlurStats {
            mean: 100.0,
            stddev: 10.0,
            sample_count: 10,
        };
        assert!(classify_blur(70.0, &stats, 20.0, -1.5)); // z = -3.0
        assert!(!classify_blur(95.0, &stats, 20.0, -1.5)); // z = -0.5
    }

    #[test]
    fn classify_against_session_excludes_the_candidate_from_its_own_baseline() {
        // Including the candidate in its own mean/stddev caps |z| at
        // (n-1)/sqrt(n) — here that would read z ≈ -1.27 (not blurry),
        // under-flagging a real outlier. Excluding it (the correct
        // behaviour) reads z ≈ -1.70, past the -1.5 threshold.
        let others = [100.0, 120.0, 80.0, 110.0, 90.0];
        assert!(classify_against_session(76.0, &others));
    }

    #[test]
    fn meets_scoring_size_rejects_a_source_below_the_cap() {
        assert!(!meets_scoring_size(
            BLUR_SCORE_LONG_EDGE_PX - 1,
            BLUR_SCORE_LONG_EDGE_PX - 1
        ));
        // A DNG's embedded preview, or a small JPEG: short on both edges.
        assert!(!meets_scoring_size(960, 540));
    }

    #[test]
    fn meets_scoring_size_accepts_a_source_at_or_above_the_cap() {
        // Exactly at the cap on the long edge is fine — scoring it is a
        // no-op resize, not an upscale.
        assert!(meets_scoring_size(BLUR_SCORE_LONG_EDGE_PX, 480));
        assert!(meets_scoring_size(4000, 3000));
    }

    /// A sharp (2px checkerboard) JPEG of the given size, on disk.
    fn textured_jpeg(dir: &Path, name: &str, width: u32, height: u32) -> std::path::PathBuf {
        let path = dir.join(name);
        image::RgbImage::from_fn(width, height, |x, y| {
            let v = if (x / 2 + y / 2) % 2 == 0 { 0 } else { 255 };
            image::Rgb([v, v, v])
        })
        .save(&path)
        .expect("write jpeg");
        path
    }

    // Regression coverage for `blur_score_for_copy` itself (not just the
    // `meets_scoring_size` predicate it calls): pins the end-to-end
    // decode-resize-score pipeline so an upscale-before-scoring bug would
    // fail here directly, rather than only surfacing indirectly through an
    // `import_io` integration test.
    #[test]
    fn sources_below_the_long_edge_are_not_scored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let small = textured_jpeg(dir.path(), "small.jpg", 640, 480);
        assert_eq!(blur_score_for_copy(&small, ImageFormat::Jpeg), None);
    }

    #[test]
    fn sources_at_or_above_the_long_edge_are_scored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exact = textured_jpeg(dir.path(), "exact.jpg", 1280, 960);
        let large = textured_jpeg(dir.path(), "large.jpg", 2560, 1920);
        assert!(blur_score_for_copy(&exact, ImageFormat::Jpeg).is_some());
        assert!(blur_score_for_copy(&large, ImageFormat::Jpeg).is_some());
    }
}
