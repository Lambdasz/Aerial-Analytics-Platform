//! Blur Detection (Module 1.5): Laplacian-variance sharpness scoring,
//! classified relative to the flight session an image belongs to.
//!
//! [`compute_blur_score`] is pure: a 3x3 Laplacian convolution over a
//! grayscale buffer, followed by the variance of the response. Scoring a
//! file end-to-end ([`blur_score_for_copy`]) resizes first so the score is
//! comparable across source resolutions, and never fails its caller — a
//! corrupt or undecodable file simply yields `None`, the same fallback
//! philosophy [`super::import_io::metadata_from_copy`] uses for EXIF
//! extraction failures.
//!
//! # Why session-relative, not a single global threshold
//!
//! Laplacian variance alone can't tell a sharp, low-texture aerial photo
//! (farmland, open water) from a genuinely blurry one — both have few sharp
//! edges. But images from the same flight session share similar terrain and
//! altitude, so comparing a score against the mean/stddev of its own
//! session's other scores ([`classify_blur`]) is a self-calibrating
//! baseline. [`HARD_FLOOR_BLUR_SCORE`] remains underneath as an absolute
//! safety net for a session where every image is bad. See
//! [`super::import_io::ImportScope::single`] and
//! [`super::import_io::ImportScope::pair`] for where this runs, at import
//! time, and why classification is frozen at import rather than
//! recomputed for the whole session each time new images land.

use std::path::Path;

use image::GrayImage;

use crate::models::image::ImageFormat;

/// Long edge (px) images are resized to before scoring, so raw variance is
/// comparable across source resolutions. Matches
/// [`crate::commands::image_preview`]'s preview cap.
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

/// Minimum number of session samples (including the image being classified)
/// before relative (z-score) classification applies. Below this, only the
/// hard floor applies: a mean/stddev computed from too few points is not a
/// meaningful baseline.
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

/// Reads, decodes, resizes, and scores one imported copy.
///
/// Never fails its caller: any read or decode error is logged and yields
/// `None` so import always proceeds. DNG is scored from its embedded
/// preview ([`super::dng_preview::read_dng_preview`]) since no raw decoder
/// exists in this codebase; JPEG decodes directly.
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
    let resized = decoded.resize(
        BLUR_SCORE_LONG_EDGE_PX,
        BLUR_SCORE_LONG_EDGE_PX,
        image::imageops::FilterType::Triangle,
    );
    Some(compute_blur_score(&resized.to_luma8()))
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
/// A score below `hard_floor` is always blurry. Otherwise, with fewer than
/// [`MIN_SESSION_SAMPLE_FOR_RELATIVE`] samples or a zero stddev, there is no
/// meaningful baseline yet, so nothing beyond the hard floor is flagged.
/// Past that minimum, a z-score below `z_threshold` standard deviations
/// under the session mean is also classified as blurry.
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
}
