//! Georeferenced Raster Clipper & Tiled Slicing Engine (Module 3).
//!
//! Menyediakan kalkulasi pemotongan geospasial murni dari koordinat poligon WGS84
//! ke batas piksel raster orthomosaic, serta pembagian ubin (*tiled slicing*)
//! untuk menjamin penggunaan memori konstan O(1) dan mencegah OOM (*Out Of Memory*).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::store::get_images_dir;
use super::types::StoredImageMetadata;

/// Kesalahan dalam operasi pemotongan raster orthomosaic.
#[derive(Debug, thiserror::Error)]
pub enum ClipperError {
    #[error("Koordinat plot berada di luar jangkauan citra orthomosaic")]
    OutOfBounds,
    #[error("Geometri poligon tidak valid: {0}")]
    InvalidGeometry(String),
    #[error("Citra orthomosaic '{0}' tidak ditemukan di storage")]
    OrthoNotFound(String),
    #[error("Metadata citra orthomosaic korup atau tidak memiliki bounds")]
    MissingBounds,
    #[error("Gagal membaca atau memproses citra: {0}")]
    ImageError(String),
    #[error("Kesalahan I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// Bounding box pada sistem koordinat piksel raster `[0..width, 0..height]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelBBox {
    pub min_col: u32,
    pub min_row: u32,
    pub max_col: u32,
    pub max_row: u32,
}

impl PixelBBox {
    /// Lebar area dalam piksel.
    pub fn width(&self) -> u32 {
        self.max_col.saturating_sub(self.min_col)
    }

    /// Tinggi area dalam piksel.
    pub fn height(&self) -> u32 {
        self.max_row.saturating_sub(self.min_row)
    }

    /// Total jumlah piksel.
    pub fn area_pixels(&self) -> u64 {
        self.width() as u64 * self.height() as u64
    }
}

/// Jendela potongan sub-citra (slice) untuk pemrosesan sekuensial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceWindow {
    pub slice_index: usize,
    pub total_slices: usize,
    pub col_offset: u32,
    pub row_offset: u32,
    pub width: u32,
    pub height: u32,
}

// ============================================================================
// 1. FUNCTIONAL CORE (Pure Functions, Zero I/O)
// ============================================================================

/// Pure: Mengonversi koordinat poligon WGS84 `[[lon, lat], ...]` menjadi batas piksel raster.
///
/// **Pre-condition**:
/// - `bounds` berupa `[[south, west], [north, east]]` dengan `south < north` dan `west < east`.
/// - `img_w > 0` dan `img_h > 0`.
/// - `polygon_coords` memiliki minimal 3 koordinat.
///
/// **Post-condition**:
/// - Mengembalikan `PixelBBox` di mana `min_col < max_col` dan `min_row < max_row`
///   dengan batas yang di-clamp ke `[0, img_w]` dan `[0, img_h]`.
/// - Mengembalikan `Err(ClipperError::OutOfBounds)` jika poligon tidak beririsan sama sekali dengan raster.
pub fn polygon_to_pixel_bbox(
    polygon_coords: &[[f64; 2]],
    bounds: [[f64; 2]; 2],
    img_w: u32,
    img_h: u32,
    padding: u32,
) -> Result<PixelBBox, ClipperError> {
    if polygon_coords.len() < 3 {
        return Err(ClipperError::InvalidGeometry(
            "Poligon membutuhkan minimal 3 koordinat posisi".to_string(),
        ));
    }
    if img_w == 0 || img_h == 0 {
        return Err(ClipperError::InvalidGeometry(
            "Dimensi citra tidak valid (0 piksel)".to_string(),
        ));
    }

    let south = bounds[0][0];
    let west = bounds[0][1];
    let north = bounds[1][0];
    let east = bounds[1][1];

    if south >= north || west >= east {
        return Err(ClipperError::InvalidGeometry(
            "Batas koordinat raster tidak valid (south >= north atau west >= east)".to_string(),
        ));
    }

    let mut poly_min_lon = f64::INFINITY;
    let mut poly_max_lon = f64::NEG_INFINITY;
    let mut poly_min_lat = f64::INFINITY;
    let mut poly_max_lat = f64::NEG_INFINITY;

    for point in polygon_coords {
        let lon = point[0];
        let lat = point[1];
        if lon < poly_min_lon {
            poly_min_lon = lon;
        }
        if lon > poly_max_lon {
            poly_max_lon = lon;
        }
        if lat < poly_min_lat {
            poly_min_lat = lat;
        }
        if lat > poly_max_lat {
            poly_max_lat = lat;
        }
    }

    // Fast-fail: cek irisan spasial (interseksi AABB)
    if poly_max_lon < west || poly_min_lon > east || poly_max_lat < south || poly_min_lat > north {
        return Err(ClipperError::OutOfBounds);
    }

    // Batasi ke dalam jangkauan raster (clamping)
    let clamped_min_lon = poly_min_lon.max(west);
    let clamped_max_lon = poly_max_lon.min(east);
    let clamped_min_lat = poly_min_lat.max(south);
    let clamped_max_lat = poly_max_lat.min(north);

    let span_lon = east - west;
    let span_lat = north - south;

    // Proyeksi koordinat ke piksel
    // Kolom: barat (0) ke timur (img_w)
    // Baris: utara (0) ke selatan (img_h)
    let raw_col_min = ((clamped_min_lon - west) / span_lon) * img_w as f64;
    let raw_col_max = ((clamped_max_lon - west) / span_lon) * img_w as f64;
    let raw_row_min = ((north - clamped_max_lat) / span_lat) * img_h as f64;
    let raw_row_max = ((north - clamped_min_lat) / span_lat) * img_h as f64;

    const EPSILON: f64 = 1e-7;
    let pad = padding as f64;
    let col_min = ((raw_col_min + EPSILON) - pad).floor().max(0.0) as u32;
    let col_max = (((raw_col_max - EPSILON) + pad).ceil() as u32).min(img_w);
    let row_min = ((raw_row_min + EPSILON) - pad).floor().max(0.0) as u32;
    let row_max = (((raw_row_max - EPSILON) + pad).ceil() as u32).min(img_h);

    if col_min >= col_max || row_min >= row_max {
        return Err(ClipperError::OutOfBounds);
    }

    Ok(PixelBBox {
        min_col: col_min,
        min_row: row_min,
        max_col: col_max,
        max_row: row_max,
    })
}

/// Pure: Membagi bounding box piksel menjadi ubin-ubin slice berukuran tetap.
///
/// **Aturan Slicing**:
/// - Jika lebar $\le 2048$ DAN tinggi $\le 2048$, kembalikan **1 slice tunggal** (tanpa slicing).
/// - Jika dimensi melebihi ambang batas, potong menjadi grid ubin dengan ukuran `chunk_size`
///   (default 1024) dan overlap `overlap` (default 32).
pub fn calculate_slices(bbox: &PixelBBox, chunk_size: u32, overlap: u32) -> Vec<SliceWindow> {
    let w = bbox.width();
    let h = bbox.height();

    // Fast-path: jika area masih dalam toleransi aman single image (<= 2048x2048)
    if w <= 2048 && h <= 2048 {
        return vec![SliceWindow {
            slice_index: 0,
            total_slices: 1,
            col_offset: bbox.min_col,
            row_offset: bbox.min_row,
            width: w,
            height: h,
        }];
    }

    let chunk = if chunk_size == 0 { 1024 } else { chunk_size };
    let step = if chunk > overlap {
        chunk - overlap
    } else {
        chunk
    };

    let mut windows = Vec::new();
    let mut row = bbox.min_row;

    while row < bbox.max_row {
        let slice_h = chunk.min(bbox.max_row - row);
        let mut col = bbox.min_col;

        while col < bbox.max_col {
            let slice_w = chunk.min(bbox.max_col - col);

            windows.push(SliceWindow {
                slice_index: 0, // diisi nanti
                total_slices: 0,
                col_offset: col,
                row_offset: row,
                width: slice_w,
                height: slice_h,
            });

            if col + slice_w >= bbox.max_col {
                break;
            }
            col += step;
        }

        if row + slice_h >= bbox.max_row {
            break;
        }
        row += step;
    }

    let total = windows.len();
    for (i, win) in windows.iter_mut().enumerate() {
        win.slice_index = i;
        win.total_slices = total;
    }

    windows
}

// ============================================================================
// 2. IMPERATIVE SHELL (File I/O, Image Decoding, Slicing Execution)
// ============================================================================

/// Memotong wilayah sub-citra dari berkas citra sumber dan menyimpannya sebagai PNG.
pub fn crop_raster_region(
    source_img_path: &Path,
    window: &SliceWindow,
    output_path: &Path,
) -> Result<(), ClipperError> {
    let img = image::open(source_img_path)
        .map_err(|e| ClipperError::ImageError(format!("Gagal membuka citra sumber: {}", e)))?;

    let cropped = img.crop_imm(
        window.col_offset,
        window.row_offset,
        window.width,
        window.height,
    );

    cropped
        .save(output_path)
        .map_err(|e| ClipperError::ImageError(format!("Gagal menyimpan potongan citra: {}", e)))?;

    Ok(())
}

/// Menghasilkan potongan-potongan slice citra untuk satu Plot dari direktori orthomosaic.
///
/// **Strategi Berkas Sumber**:
/// - Memprioritaskan `original.tif` jika tersimpan di disk.
/// - Fallback otomatis ke `display.png` atau `display.jpg` jika berkas asli tidak ada.
pub fn clip_orthomosaic_for_plot(
    ortho_id: &str,
    polygon_coords: &[[f64; 2]],
    staging_dir: &Path,
    plot_id: &str,
) -> Result<Vec<PathBuf>, ClipperError> {
    let target_dir = get_images_dir().join(ortho_id);
    if !target_dir.is_dir() {
        return Err(ClipperError::OrthoNotFound(ortho_id.to_string()));
    }

    let meta_file = target_dir.join("meta.json");
    if !meta_file.is_file() {
        return Err(ClipperError::MissingBounds);
    }

    let meta_content = fs::read_to_string(&meta_file)?;
    let meta: StoredImageMetadata =
        serde_json::from_str(&meta_content).map_err(|_| ClipperError::MissingBounds)?;

    let bounds = meta.bounds.ok_or(ClipperError::MissingBounds)?;

    // Tentukan berkas sumber citra: original.tif > display.png > display.jpg
    let original_tif = target_dir.join("original.tif");
    let display_png = target_dir.join("display.png");
    let display_jpg = target_dir.join("display.jpg");

    let (src_path, img_w, img_h) = if original_tif.is_file() {
        let (w, h) = if let Some([w, h]) = meta.dimensions {
            (w, h)
        } else {
            image::image_dimensions(&original_tif)
                .map_err(|e| ClipperError::ImageError(e.to_string()))?
        };
        (original_tif, w, h)
    } else if display_png.is_file() {
        let (w, h) = if let Some([w, h]) = meta.display_dimensions {
            (w, h)
        } else {
            image::image_dimensions(&display_png)
                .map_err(|e| ClipperError::ImageError(e.to_string()))?
        };
        (display_png, w, h)
    } else if display_jpg.is_file() {
        let (w, h) = if let Some([w, h]) = meta.display_dimensions {
            (w, h)
        } else {
            image::image_dimensions(&display_jpg)
                .map_err(|e| ClipperError::ImageError(e.to_string()))?
        };
        (display_jpg, w, h)
    } else {
        return Err(ClipperError::ImageError(
            "Tidak ditemukan berkas citra orthomosaic (original.tif atau display.png)".to_string(),
        ));
    };

    let bbox = polygon_to_pixel_bbox(polygon_coords, bounds, img_w, img_h, 10)?;
    let slices = calculate_slices(&bbox, 1024, 32);

    fs::create_dir_all(staging_dir)?;

    let mut generated_paths = Vec::with_capacity(slices.len());
    for slice in &slices {
        let slice_filename = format!(
            "clip_{}_slice_{}_of_{}.png",
            plot_id,
            slice.slice_index + 1,
            slice.total_slices
        );
        let out_path = staging_dir.join(slice_filename);
        crop_raster_region(&src_path, slice, &out_path)?;
        generated_paths.push(out_path);
    }

    Ok(generated_paths)
}

// ============================================================================
// 3. UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polygon_to_pixel_bbox_inside() {
        // Raster: south = -7.5, west = 110.0, north = -7.0, east = 110.5, 1000x1000 px
        let bounds = [[-7.5, 110.0], [-7.0, 110.5]];
        let poly = vec![
            [110.1, -7.4],
            [110.3, -7.4],
            [110.3, -7.2],
            [110.1, -7.2],
            [110.1, -7.4],
        ];

        let bbox = polygon_to_pixel_bbox(&poly, bounds, 1000, 1000, 0).unwrap();
        // lon: 110.1 -> (110.1 - 110.0)/0.5 * 1000 = 200
        // lon: 110.3 -> (110.3 - 110.0)/0.5 * 1000 = 600
        // lat: -7.2 -> (-7.0 - (-7.2))/0.5 * 1000 = 400
        // lat: -7.4 -> (-7.0 - (-7.4))/0.5 * 1000 = 800
        assert_eq!(bbox.min_col, 200);
        assert_eq!(bbox.max_col, 600);
        assert_eq!(bbox.min_row, 400);
        assert_eq!(bbox.max_row, 800);
        assert_eq!(bbox.width(), 400);
        assert_eq!(bbox.height(), 400);
    }

    #[test]
    fn test_polygon_to_pixel_bbox_outside_returns_error() {
        let bounds = [[-7.5, 110.0], [-7.0, 110.5]];
        // Plot berada di barat raster (lon 108.0)
        let poly = vec![
            [108.1, -7.4],
            [108.3, -7.4],
            [108.3, -7.2],
            [108.1, -7.2],
            [108.1, -7.4],
        ];

        let res = polygon_to_pixel_bbox(&poly, bounds, 1000, 1000, 0);
        assert!(matches!(res, Err(ClipperError::OutOfBounds)));
    }

    #[test]
    fn test_calculate_slices_below_threshold_single_window() {
        let bbox = PixelBBox {
            min_col: 100,
            min_row: 100,
            max_col: 1600,
            max_row: 1500,
        };
        // 1500x1400 <= 2048x2048 -> single slice
        let slices = calculate_slices(&bbox, 1024, 32);
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].slice_index, 0);
        assert_eq!(slices[0].total_slices, 1);
        assert_eq!(slices[0].width, 1500);
        assert_eq!(slices[0].height, 1400);
    }

    #[test]
    fn test_calculate_slices_above_threshold_grid() {
        let bbox = PixelBBox {
            min_col: 0,
            min_row: 0,
            max_col: 3000,
            max_row: 2500,
        };
        // 3000x2500 > 2048x2048 -> dipecah dengan chunk_size 1024, overlap 32 (step 992)
        let slices = calculate_slices(&bbox, 1024, 32);
        assert!(slices.len() > 1);
        for slice in &slices {
            assert!(slice.width <= 1024);
            assert!(slice.height <= 1024);
            assert_eq!(slice.total_slices, slices.len());
        }
    }

    #[test]
    fn test_crop_raster_region_pure_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let src_path = temp.path().join("src.png");
        let out_path = temp.path().join("out.png");

        // Buat citra uji 100x100
        let img = image::RgbImage::new(100, 100);
        img.save(&src_path).unwrap();

        let window = SliceWindow {
            slice_index: 0,
            total_slices: 1,
            col_offset: 10,
            row_offset: 20,
            width: 30,
            height: 40,
        };

        crop_raster_region(&src_path, &window, &out_path).unwrap();
        assert!(out_path.exists());

        let cropped_meta = image::image_dimensions(&out_path).unwrap();
        assert_eq!(cropped_meta, (30, 40));
    }
}
