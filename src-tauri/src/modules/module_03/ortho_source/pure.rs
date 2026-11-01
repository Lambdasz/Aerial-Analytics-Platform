//! Fungsi murni untuk validasi citra, klasifikasi format, dan perbandingan rasio aspek.

use super::types::ImageCategory;
use crate::modules::module_03::error::MapControllerError;

/// Mengklasifikasikan kategori citra berdasarkan ekstensi file secara murni.
/// Menolak berkas TIFF dengan pesan jelas sesuai spesifikasi baru.
pub fn classify_image_extension(ext: &str) -> Result<ImageCategory, MapControllerError> {
    match ext.to_lowercase().as_str() {
        "png" | "jpg" | "jpeg" => Ok(ImageCategory::Ortho),
        "tif" | "tiff" => Err(MapControllerError::InvalidGeometry(
            "Format berkas TIFF tidak lagi didukung. Harap gunakan format JPG atau PNG."
                .to_string(),
        )),
        _ => Err(MapControllerError::InvalidGeometry(format!(
            "Ekstensi '{}' tidak didukung. Hanya mendukung citra JPG (.jpg, .jpeg) dan PNG (.png).",
            ext
        ))),
    }
}

/// Menghitung perbedaan relatif rasio aspek antara bounding box geografis dan rasio piksel citra.
/// Mengembalikan nilai selisih absolut rasio (|R_geo / R_px - 1|).
pub fn calculate_aspect_ratio_difference(bounds: [[f64; 2]; 2], dimensions: (u32, u32)) -> f64 {
    let (width, height) = dimensions;
    if width == 0 || height == 0 {
        return 0.0;
    }

    let pixel_ratio = width as f64 / height as f64;

    let south = bounds[0][0];
    let west = bounds[0][1];
    let north = bounds[1][0];
    let east = bounds[1][1];

    let lat_mid_rad = ((south + north) / 2.0).to_radians();
    let delta_lon_deg = (east - west).abs();
    let delta_lat_deg = (north - south).abs();

    if delta_lat_deg <= f64::EPSILON {
        return 0.0;
    }

    let geo_ratio = (delta_lon_deg * lat_mid_rad.cos()) / delta_lat_deg;

    ((geo_ratio / pixel_ratio) - 1.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_image_extension() {
        assert_eq!(
            classify_image_extension("jpg").unwrap(),
            ImageCategory::Ortho
        );
        assert_eq!(
            classify_image_extension("jpeg").unwrap(),
            ImageCategory::Ortho
        );
        assert_eq!(
            classify_image_extension("png").unwrap(),
            ImageCategory::Ortho
        );
        assert!(classify_image_extension("tif").is_err());
        assert!(classify_image_extension("tiff").is_err());
        assert!(classify_image_extension("pdf").is_err());
    }

    #[test]
    fn test_calculate_aspect_ratio_difference_exact() {
        // Kotak 1:1 di khatulistiwa (lat_mid = 0)
        let bounds = [[0.0, 100.0], [1.0, 101.0]];
        let dimensions = (1000, 1000);
        let diff = calculate_aspect_ratio_difference(bounds, dimensions);
        assert!(diff < 0.01);
    }
}
