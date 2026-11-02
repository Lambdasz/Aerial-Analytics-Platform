//! Vegetation indices and canopy mask thresholding algorithms (Module 5 §2).
//!
//! Pure mathematical transformations mapping normalized RGB pixels to canopy indicators.

use crate::types::{BinaryMask, PluginError, Result, RgbArray, EPSILON};

/// Per-pixel vegetation index value on normalized channels. Pure.
pub fn pixel_index(method: &str, p: [f64; 3]) -> Result<f64> {
    let [r, g, b] = p;
    match method {
        "ExG" => Ok(2.0 * g - r - b),
        "ExR" => Ok(1.4 * r - g),
        "VARI" => Ok((g - r) / (g + r - b + EPSILON)),
        _ => Err(PluginError::Value(format!("Unknown method: {method}"))),
    }
}

/// Default detection threshold per index, mirroring the Module 4 suggestion. Pure.
pub fn default_threshold(method: &str) -> f64 {
    match method {
        "ExR" => 0.0,
        "VARI" => 0.03,
        _ => 0.10,
    }
}

/// Maps every pixel to its index value. Pure.
pub fn vegetation_index(rgb: &RgbArray, method: &str) -> Result<Vec<f64>> {
    rgb.pixels.iter().map(|&p| pixel_index(method, p)).collect()
}

/// Thresholds index values into a 0/1 mask. Pure.
pub fn apply_threshold(index: &[f64], threshold_value: f64) -> Result<Vec<u8>> {
    if !(0.0..=1.0).contains(&threshold_value) {
        return Err(PluginError::Value(format!(
            "threshold_value must be between 0.0 and 1.0, got {threshold_value}"
        )));
    }
    Ok(index
        .iter()
        .map(|&v| u8::from(v > threshold_value))
        .collect())
}

/// Derives a mask straight from an RGB image. Pure.
pub fn mask_from_rgb(rgb: &RgbArray, method: &str, threshold_value: f64) -> Result<BinaryMask> {
    let index = vegetation_index(rgb, method)?;
    let data = apply_threshold(&index, threshold_value)?;
    Ok(BinaryMask {
        width: rgb.width,
        height: rgb.height,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(pixels: Vec<[f64; 3]>) -> RgbArray {
        RgbArray {
            width: pixels.len(),
            height: 1,
            pixels,
        }
    }

    #[test]
    fn exg_flags_green_but_not_gray() {
        let rgb = img(vec![[0.04, 0.78, 0.04], [0.4, 0.4, 0.4], [0.0, 0.0, 0.0]]);
        let mask = mask_from_rgb(&rgb, "ExG", 0.1).unwrap();
        assert_eq!(mask.data, vec![1, 0, 0]);
    }

    #[test]
    fn unknown_method_and_out_of_range_threshold_fail() {
        assert!(pixel_index("XYZ", [0.1, 0.2, 0.3]).is_err());
        assert!(apply_threshold(&[0.5], 1.5).is_err());
    }
}
