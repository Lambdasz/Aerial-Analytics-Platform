#![allow(dead_code)]

//! Vegetation coverage measurement for Module 10.3.
//!
//! Module 10.3 needs a coverage percentage for the "before" and "after" image.
//! Until Module 4/5 (RGB vegetation detection / coverage analytics) exposes a
//! plugin result, Module 10 computes a deliberately simple placeholder directly
//! from the stored image: a pixel counts as vegetation when the excess green
//! index `ExG = 2G - R - B` is clearly positive.
//!
//! The measurement is intentionally split in two:
//! - [`coverage_from_rgb`] is pure and unit-tested with hand-built pixels.
//! - [`coverage_from_image_path`] is the thin I/O wrapper that decodes a file
//!   and downscales it before measuring.
//!
//! Replacing the heuristic later only means swapping the classifier inside
//! [`coverage_from_rgb`]; the rest of Module 10 keeps the same types.

use std::path::Path;

use image::DynamicImage;
use serde::Serialize;
use thiserror::Error;

/// Longest edge (in pixels) analysed. Aerial images are far larger than the
/// statistics need, so they are downscaled first to keep the command fast.
pub const MAX_ANALYSIS_EDGE: u32 = 1024;

/// Minimum `2G - R - B` (0-255 scale) for a pixel to count as vegetation.
/// Zero alone would also accept grey pixels with a green tint, so a small
/// margin is used.
pub const EXCESS_GREEN_THRESHOLD: f32 = 20.0;

/// Errors produced while measuring vegetation coverage.
#[derive(Debug, Error)]
pub enum CoverageError {
    #[error("gagal membaca citra '{path}': {source}")]
    ReadImage {
        path: String,
        #[source]
        source: image::ImageError,
    },

    #[error("citra tidak memiliki piksel RGB")]
    EmptyImage,
}

/// Result of measuring vegetation coverage on one image.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VegetationCoverage {
    /// Number of pixels (after downscaling) that were classified.
    pub sampled_pixels: u64,
    /// Number of sampled pixels classified as vegetation.
    pub vegetation_pixels: u64,
    /// `vegetation_pixels / sampled_pixels * 100`, in `0.0..=100.0`.
    pub coverage_percent: f64,
}

/// Counts vegetation pixels in an RGB8 buffer (`r, g, b, r, g, b, ...`).
///
/// Pure: the buffer is only read, never modified.
pub fn coverage_from_rgb(rgb: &[u8]) -> Result<VegetationCoverage, CoverageError> {
    let (pixels, _trailing) = rgb.as_chunks::<3>();
    let sampled_pixels = pixels.len() as u64;
    if sampled_pixels == 0 {
        return Err(CoverageError::EmptyImage);
    }

    let vegetation_pixels = pixels
        .iter()
        .filter(|pixel| is_vegetation(pixel[0], pixel[1], pixel[2]))
        .count() as u64;

    Ok(VegetationCoverage {
        sampled_pixels,
        vegetation_pixels,
        coverage_percent: vegetation_pixels as f64 / sampled_pixels as f64 * 100.0,
    })
}

/// Measures vegetation coverage of an image file.
///
/// The image is downscaled to at most [`MAX_ANALYSIS_EDGE`] on its longest
/// edge, then measured with [`coverage_from_rgb`]. Reading the file is the only
/// impure step; callers hold no database lock while this runs.
pub fn coverage_from_image_path(path: &Path) -> Result<VegetationCoverage, CoverageError> {
    let image = image::open(path).map_err(|source| CoverageError::ReadImage {
        path: path.display().to_string(),
        source,
    })?;

    let image = downscale(image);
    let rgb = image.to_rgb8();

    coverage_from_rgb(rgb.as_raw())
}

fn downscale(image: DynamicImage) -> DynamicImage {
    if image.width() > MAX_ANALYSIS_EDGE || image.height() > MAX_ANALYSIS_EDGE {
        image.thumbnail(MAX_ANALYSIS_EDGE, MAX_ANALYSIS_EDGE)
    } else {
        image
    }
}

/// Excess green index: green must dominate both red and blue by a margin.
fn is_vegetation(red: u8, green: u8, blue: u8) -> bool {
    let (red, green, blue) = (f32::from(red), f32::from(green), f32::from(blue));
    2.0 * green - red - blue > EXCESS_GREEN_THRESHOLD
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use image::{Rgb, RgbImage};

    use super::*;

    #[test]
    fn counts_green_pixels_only() {
        // green, red, white, black -> 1 of 4 pixels is vegetation.
        let rgb = [0, 200, 0, 200, 0, 0, 255, 255, 255, 0, 0, 0];

        let coverage = coverage_from_rgb(&rgb).expect("ada piksel");

        assert_eq!(coverage.sampled_pixels, 4);
        assert_eq!(coverage.vegetation_pixels, 1);
        assert_eq!(coverage.coverage_percent, 25.0);
    }

    #[test]
    fn barely_green_grey_is_not_vegetation() {
        // Difference is 8, below the threshold, so a grey-green road is skipped.
        let rgb = [100, 104, 100];

        let coverage = coverage_from_rgb(&rgb).expect("ada piksel");

        assert_eq!(coverage.vegetation_pixels, 0);
        assert_eq!(coverage.coverage_percent, 0.0);
    }

    #[test]
    fn rejects_buffer_without_pixels() {
        assert!(matches!(
            coverage_from_rgb(&[]),
            Err(CoverageError::EmptyImage)
        ));
        assert!(matches!(
            coverage_from_rgb(&[1, 2]),
            Err(CoverageError::EmptyImage)
        ));
    }

    #[test]
    fn reads_coverage_from_a_real_image_file() {
        let mut image = RgbImage::new(2, 1);
        image.put_pixel(0, 0, Rgb([0, 200, 0]));
        image.put_pixel(1, 0, Rgb([200, 0, 0]));

        let dir = tempfile::tempdir().expect("direktori sementara dibuat");
        let path: PathBuf = dir.path().join("plot.png");
        image.save(&path).expect("citra uji tersimpan");

        let coverage = coverage_from_image_path(&path).expect("citra terbaca");

        assert_eq!(coverage.sampled_pixels, 2);
        assert_eq!(coverage.vegetation_pixels, 1);
        assert_eq!(coverage.coverage_percent, 50.0);
    }

    #[test]
    fn reports_unreadable_path() {
        let result = coverage_from_image_path(Path::new("C:/tidak/ada/citra.png"));

        assert!(matches!(result, Err(CoverageError::ReadImage { .. })));
    }
}
