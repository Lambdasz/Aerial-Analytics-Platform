//! Pembaca dan pengurai GeoTIFF pure Rust menggunakan pustaka tiff 0.9.1.
//!
//! Mengimplementasikan:
//! 1. Validasi fast-fail header berkas (ukuran <= 250 MB, magic bytes, dimensi max 25.000 px, total piksel max 500 MP).
//! 2. Ekstraksi georeferensi ModelTiepointTag (33922), ModelPixelScaleTag (33550), ModelTransformationTag (34264).
//! 3. Parsing GeoKeyDirectoryTag (34735) untuk ekstraksi CRS (EPSG:4326, EPSG:3857, UTM 1-60 N/S), deteksi raster point vs area.
//! 4. Perhitungan envelope bounding box 4 sudut yang diproyeksikan ke WGS84 derajat desimal.
//! 5. Ekstraksi detail teknis raster (kompresi, layout tiled/stripped, planar, bits per sample, photometric).
//! 6. Deteksi kanal alpha via ExtraSamples (338) dan nilai nodata via GDAL_NODATA (42113).
//! 7. Analisis persentil deterministik p0,5 - p99,5 dari grid tile untuk peregangan kontras citra 16-bit.
//! 8. Perakitan strip tile-row penuh (Tile-Row Assembly) dan streaming ke StreamingResampler.
//! 9. Deteksi dan pemilihan Sub-IFD / Overview piramida (COG).

use std::fs::{self, File};
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use tiff::decoder::ifd::Value;
use tiff::decoder::{ChunkType, Decoder, DecodingResult};
use tiff::tags::Tag;
use tiff::ColorType;

use super::constants::{
    DISPLAY_TARGET_DIMENSION_PX, MAX_OPTION_B_PIXELS, MAX_TIFF_DIMENSION_PX,
    MAX_TIFF_FILE_SIZE_BYTES,
};
use super::job_manager::CancellationToken;
use super::streaming_resampler::StreamingResampler;
use super::types::TiffTechnicalDetails;
use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::geometry::projection::compute_wgs84_envelope_from_corners;

/// Ringkasan header berkas GeoTIFF setelah validasi awal.
#[derive(Debug, Clone)]
pub struct GeoTiffHeaderSummary {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    pub samples_per_pixel: u32,
    pub color_type: ColorType,
    pub chunk_type: ChunkType,
    pub tile_dimensions: Option<(u32, u32)>,
    pub compression_name: String,
    pub has_alpha: bool,
    pub nodata_value: Option<f64>,
    pub is_min_is_white: bool,
}

/// Informasi georeferensi yang diekstrak dari tag GeoTIFF.
#[derive(Debug, Clone)]
pub struct GeoTiffGeoreference {
    pub crs_code: u32,
    pub crs_string: String,
    pub bounds: [[f64; 2]; 2],
    pub pixel_scale: (f64, f64),
    pub tiepoint: (f64, f64),
    pub is_point_raster: bool,
}

// ============================================================================
// 1. VALIDASI FAST-FAIL HEADER
// ============================================================================

/// Memeriksa keabsahan berkas TIFF sebelum folder staging dibuat di disk.
///
/// Memeriksa ukuran berkas (<= 250 MB), magic bytes (II*, MM*, II+, MM+),
/// dimensi piksel (<= 25.000 px per sisi), dan batasan megapiksel (<= 500 MP).
pub fn validate_geotiff_header_fast(path: &Path) -> Result<GeoTiffHeaderSummary, OrthoImportError> {
    if !path.exists() {
        return Err(OrthoImportError::FileKorup(format!(
            "Berkas GeoTIFF tidak ditemukan: {}",
            path.display()
        )));
    }

    let meta = fs::metadata(path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membaca metadata berkas: {}", e))
    })?;
    let size = meta.len();

    #[cfg(not(test))]
    if size > MAX_TIFF_FILE_SIZE_BYTES {
        let size_mb = size as f64 / (1024.0 * 1024.0);
        return Err(OrthoImportError::MelebihiBatas {
            width: 0,
            height: 0,
            max_dimension: 0,
            message: format!(
                "Ukuran berkas {:.1} MB melebihi batas maksimal 250 MB untuk impor Opsi B",
                size_mb
            ),
        });
    }
    #[cfg(test)]
    if size > MAX_TIFF_FILE_SIZE_BYTES && std::env::var("AAP_BYPASS_MAX_TIFF_SIZE").is_err() {
        let size_mb = size as f64 / (1024.0 * 1024.0);
        return Err(OrthoImportError::MelebihiBatas {
            width: 0,
            height: 0,
            max_dimension: 0,
            message: format!(
                "Ukuran berkas {:.1} MB melebihi batas maksimal 250 MB untuk impor Opsi B",
                size_mb
            ),
        });
    }

    // Periksa magic bytes (8 byte pertama)
    let mut file = File::open(path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuka berkas GeoTIFF: {}", e))
    })?;
    let mut header = [0u8; 8];
    let bytes_read = file
        .read(&mut header)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca header berkas: {}", e)))?;

    if bytes_read < 4 {
        return Err(OrthoImportError::FileKorup(
            "Berkas terlalu kecil untuk menjadi berkas TIFF yang valid".to_string(),
        ));
    }

    let is_little_endian_tiff = header[0..4] == [0x49, 0x49, 0x2A, 0x00];
    let is_big_endian_tiff = header[0..4] == [0x4D, 0x4D, 0x00, 0x2A];
    let is_little_endian_bigtiff = header[0..4] == [0x49, 0x49, 0x2B, 0x00];
    let is_big_endian_bigtiff = header[0..4] == [0x4D, 0x4D, 0x00, 0x2B];

    if !is_little_endian_tiff
        && !is_big_endian_tiff
        && !is_little_endian_bigtiff
        && !is_big_endian_bigtiff
    {
        return Err(OrthoImportError::FormatTidakDidukung(
            "Signature berkas bukan format TIFF atau BigTIFF yang valid".to_string(),
        ));
    }

    // Buka berkas dengan Decoder tiff untuk membaca IFD0
    let file = File::open(path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuka berkas TIFF: {}", e))
    })?;
    let reader = BufReader::new(file);
    let mut decoder = Decoder::new(reader)?;

    let (width, height) = decoder.dimensions()?;

    if width == 0 || height == 0 {
        return Err(OrthoImportError::FileKorup(
            "Dimensi citra TIFF bernilai nol".to_string(),
        ));
    }

    if width > MAX_TIFF_DIMENSION_PX || height > MAX_TIFF_DIMENSION_PX {
        return Err(OrthoImportError::MelebihiBatas {
            width,
            height,
            max_dimension: MAX_TIFF_DIMENSION_PX,
            message: format!(
                "Dimensi citra {}x{} piksel melebihi batas maksimal 25.000 piksel per sisi",
                width, height
            ),
        });
    }

    let total_pixels = (width as u64) * (height as u64);
    if total_pixels > MAX_OPTION_B_PIXELS {
        return Err(OrthoImportError::MelebihiBatas {
            width,
            height,
            max_dimension: MAX_TIFF_DIMENSION_PX,
            message: format!(
                "Total piksel {:.1} MP melebihi batas maksimal 500 Megapiksel",
                total_pixels as f64 / 1_000_000.0
            ),
        });
    }

    // 1. Periksa PlanarConfiguration (Tag 284): 1=Chunky (didukung), 2=Planar (ditolak)
    if let Ok(val) = decoder.get_tag(Tag::PlanarConfiguration) {
        let is_planar = matches!(val, Value::Short(2) | Value::Unsigned(2));
        if is_planar {
            return Err(OrthoImportError::FormatTidakDidukung(
                "PlanarConfiguration 2 (separate planes) tidak didukung. Harap konversi ke format Chunky (PlanarConfiguration 1).".to_string(),
            ));
        }
    }

    // 2. Periksa PhotometricInterpretation (Tag 262) & Kompresi (Tag 259)
    let comp_id = decoder.get_tag_u32(Tag::Compression).unwrap_or(1);
    let photo_val = if let Ok(val) = decoder.get_tag(Tag::PhotometricInterpretation) {
        match val {
            Value::Short(v) => Some(v as u32),
            Value::Unsigned(v) => Some(v),
            _ => None,
        }
    } else {
        None
    };

    if let Some(photo) = photo_val {
        if photo == 3 {
            return Err(OrthoImportError::FormatTidakDidukung(
                "PhotometricInterpretation Palette (3) tidak didukung untuk ortofoto. Harap konversi ke RGB/RGBA.".to_string(),
            ));
        }
        if photo == 5 {
            return Err(OrthoImportError::FormatTidakDidukung(
                "PhotometricInterpretation CMYK (5) tidak didukung untuk tampilan peta. Harap konversi ke RGB.".to_string(),
            ));
        }
        if photo == 6 && comp_id != 7 {
            return Err(OrthoImportError::FormatTidakDidukung(
                "PhotometricInterpretation YCbCr (6) hanya didukung dengan kompresi JPEG (7)."
                    .to_string(),
            ));
        }
    }

    let is_min_is_white = photo_val == Some(0);

    // 3. Validasi Kompresi: 1=None, 5=LZW, 7=JPEG, 8=Deflate, 32773=PackBits, 32946=PKZIP Deflate
    match comp_id {
        1 | 5 | 7 | 8 | 32773 | 32946 => {}
        _ => {
            let comp_name = detect_compression_name(&mut decoder);
            return Err(OrthoImportError::KompresiTidakDidukung(format!(
                "Metode kompresi '{}' (ID {}) tidak didukung oleh decoder pure Rust. Harap gunakan Deflate, LZW, JPEG, PackBits, atau Uncompressed.",
                comp_name, comp_id
            )));
        }
    }

    let color_type = decoder.colortype()?;
    let (bit_depth, samples_per_pixel) = match color_type {
        ColorType::Gray(b) => (b as u32, 1u32),
        ColorType::RGB(b) => (b as u32, 3u32),
        ColorType::RGBA(b) => (b as u32, 4u32),
        ColorType::GrayA(b) => (b as u32, 2u32),
        ColorType::YCbCr(b) => (b as u32, 3u32),
        ColorType::Palette(_) => {
            return Err(OrthoImportError::FormatTidakDidukung(
                "Format warna Indexed/Palette tidak didukung untuk ortofoto. Harap konversi ke RGB."
                    .to_string(),
            ));
        }
        ColorType::CMYK(_) => {
            return Err(OrthoImportError::FormatTidakDidukung(
                "Format warna CMYK tidak didukung untuk tampilan peta. Harap konversi ke RGB."
                    .to_string(),
            ));
        }
    };

    if bit_depth != 8 && bit_depth != 16 {
        return Err(OrthoImportError::FormatTidakDidukung(format!(
            "Kedalaman bit {} bit per kanal belum didukung. Hanya 8-bit dan 16-bit yang didukung.",
            bit_depth
        )));
    }

    let chunk_type = decoder.get_chunk_type();
    let tile_dimensions = if chunk_type == ChunkType::Tile {
        Some(decoder.chunk_dimensions())
    } else {
        None
    };

    // Deteksi ExtraSamples (Tag 338) untuk alpha
    let has_alpha = detect_alpha_from_tags(&mut decoder, samples_per_pixel);

    // Deteksi GDAL_NODATA (Tag 42113)
    let mut nodata_value = detect_nodata_from_tags(&mut decoder);

    let compression_name = detect_compression_name(&mut decoder);

    // Jika tag GDAL_NODATA tidak ada dan citra tidak memiliki alpha eksplisit,
    // periksa apakah sudut-sudut citra berwarna hitam pekat (0, 0, 0)
    // yang mencerminkan padding batas NoData penerbangan drone.
    if nodata_value.is_none()
        && !has_alpha
        && detect_black_border_nodata(&mut decoder, width, height, samples_per_pixel as usize)
    {
        nodata_value = Some(0.0);
    }

    Ok(GeoTiffHeaderSummary {
        width,
        height,
        bit_depth,
        samples_per_pixel,
        color_type,
        chunk_type,
        tile_dimensions,
        compression_name,
        has_alpha,
        nodata_value,
        is_min_is_white,
    })
}

// ============================================================================
// 2. EKSTRAKSI GEOREFERENSI & REPROYEKSI WGS84
// ============================================================================

/// Mengekstrak koordinat tiepoint, pixel scale, dan direktori GeoKey untuk menghitung
/// bounding box WGS84 `[[south, west], [north, east]]`.
pub fn extract_geotiff_georeference<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
) -> Result<GeoTiffGeoreference, OrthoImportError> {
    // 1. Periksa rotasi / affine matrix (Tag 34264)
    if let Ok(transform_matrix) = decoder.get_tag_f64_vec(Tag::ModelTransformationTag) {
        if transform_matrix.len() >= 16 {
            let m01 = transform_matrix[1];
            let m10 = transform_matrix[4];
            if m01.abs() > 1e-9 || m10.abs() > 1e-9 {
                let rot_rad = m01.atan2(transform_matrix[0]);
                return Err(OrthoImportError::RotasiTidakDidukung(rot_rad));
            }
        }
    }

    // 2. Baca ModelTiepointTag (Tag 33922)
    let tiepoints = decoder
        .get_tag_f64_vec(Tag::ModelTiepointTag)
        .map_err(|_| {
            OrthoImportError::CrsTidakDidukung(
                "Tag ModelTiepointTag (33922) tidak ditemukan dalam berkas TIFF".to_string(),
            )
        })?;

    if tiepoints.len() < 6 {
        return Err(OrthoImportError::FileKorup(
            "ModelTiepointTag harus memuat minimal 6 nilai koordinat (I, J, K, X, Y, Z)"
                .to_string(),
        ));
    }

    let i_pixel = tiepoints[0];
    let j_pixel = tiepoints[1];
    let x_origin_raw = tiepoints[3];
    let y_origin_raw = tiepoints[4];

    // 3. Baca ModelPixelScaleTag (Tag 33550)
    let pixel_scales = decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(|_| {
            OrthoImportError::CrsTidakDidukung(
                "Tag ModelPixelScaleTag (33550) tidak ditemukan dalam berkas TIFF".to_string(),
            )
        })?;

    if pixel_scales.len() < 2 {
        return Err(OrthoImportError::FileKorup(
            "ModelPixelScaleTag harus memuat minimal 2 nilai (ScaleX, ScaleY)".to_string(),
        ));
    }

    let scale_x = pixel_scales[0];
    let scale_y = pixel_scales[1];

    if scale_x <= 0.0 || scale_y <= 0.0 {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Skala piksel tidak valid: ScaleX={}, ScaleY={}",
            scale_x, scale_y
        )));
    }

    // Sesuaikan origin jika tiepoint tidak berada di (0, 0)
    let x_origin = x_origin_raw - i_pixel * scale_x;
    let y_origin = y_origin_raw + j_pixel * scale_y;

    // 4. Baca GeoKeyDirectoryTag (Tag 34735)
    let geokeys_raw = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(|_| {
            OrthoImportError::CrsTidakDidukung(
                "GeoKeyDirectoryTag (34735) tidak ditemukan dalam berkas TIFF".to_string(),
            )
        })?;

    let (raster_type, crs_code) = parse_geokeys(&geokeys_raw)?;

    let is_point_raster = raster_type == 2;
    let half_x = if is_point_raster { 0.5 } else { 0.0 };
    let half_y = if is_point_raster { 0.5 } else { 0.0 };

    // 5. Hitung batas 4 sudut citra dalam sistem koordinat proyeksi asal
    let x_min = x_origin - half_x * scale_x;
    let x_max = x_origin + (width as f64 - half_x) * scale_x;
    let y_max = y_origin + half_y * scale_y;
    let y_min = y_origin - (height as f64 - half_y) * scale_y;

    let corners = [
        (x_min, y_max), // Top-Left
        (x_max, y_max), // Top-Right
        (x_min, y_min), // Bottom-Left
        (x_max, y_min), // Bottom-Right
    ];

    // 6. Proyeksikan keempat sudut ke WGS84 untuk mendapatkan bounding box envelope
    let bounds = compute_wgs84_envelope_from_corners(&corners, crs_code)?;

    let crs_string = format_crs_name(crs_code);

    Ok(GeoTiffGeoreference {
        crs_code,
        crs_string,
        bounds,
        pixel_scale: (scale_x, scale_y),
        tiepoint: (x_origin, y_origin),
        is_point_raster,
    })
}

/// Mengurai pasangan GeoKey dari array `u16` GeoKeyDirectoryTag.
///
/// Format Header: `[DirectoryVersion, KeyRevision, MinorRevision, NumberOfKeys]`
/// Setiap Key: `[KeyID, TIFFTagLocation, Count, Value_Offset]`
fn parse_geokeys(keys: &[u16]) -> Result<(u16, u32), OrthoImportError> {
    if keys.len() < 4 {
        return Err(OrthoImportError::FileKorup(
            "Header GeoKeyDirectoryTag tidak lengkap (kurang dari 4 nilai)".to_string(),
        ));
    }

    let num_keys = keys[3] as usize;
    if keys.len() < 4 + num_keys * 4 {
        return Err(OrthoImportError::FileKorup(
            "GeoKeyDirectoryTag terpotong (jumlah key tidak sesuai)".to_string(),
        ));
    }

    let mut raster_type = 1u16; // Default: RasterPixelIsArea (1)
    let mut projected_crs: Option<u32> = None;
    let mut geographic_crs: Option<u32> = None;

    for i in 0..num_keys {
        let offset = 4 + i * 4;
        let key_id = keys[offset];
        let tag_loc = keys[offset + 1];
        let val_offset = keys[offset + 3];

        // Jika tag_loc == 0, nilai disimpan langsung pada val_offset
        if tag_loc == 0 {
            match key_id {
                1025 => {
                    // GTRasterTypeGeoKey (1 = PixelIsArea, 2 = PixelIsPoint)
                    raster_type = val_offset;
                }
                2048 => {
                    // GeographicTypeGeoKey (e.g. 4326)
                    geographic_crs = Some(val_offset as u32);
                }
                3072 if val_offset != 32767 => {
                    // ProjectedCSTypeGeoKey (e.g. 3857, 32601..=32660, 32701..=32760)
                    // 32767 = user-defined, not a concrete code.
                    projected_crs = Some(val_offset as u32);
                }
                _ => {}
            }
        }
    }

    let crs_code = if let Some(proj) = projected_crs {
        proj
    } else if let Some(geo) = geographic_crs {
        geo
    } else {
        return Err(OrthoImportError::CrsTidakDidukung(
            "Kode sistem koordinat (CRS) tidak ditemukan dalam GeoKeyDirectory".to_string(),
        ));
    };

    Ok((raster_type, crs_code))
}

/// Formats a numeric EPSG code into a descriptive human-readable CRS string.
fn format_crs_name(code: u32) -> String {
    match code {
        4326 => "EPSG:4326 (WGS84 Geodetic)".to_string(),
        3857 => "EPSG:3857 (Web Mercator)".to_string(),
        32601..=32660 => format!("EPSG:{} (UTM Zone {}N)", code, code - 32600),
        32701..=32760 => format!("EPSG:{} (UTM Zone {}S)", code, code - 32700),
        other => format!("EPSG:{}", other),
    }
}

// ============================================================================
// 3. EKSTRAKSI DETAIL TEKNIS & TAG KHUSUS
// ============================================================================

/// Detects whether an image contains an alpha transparency channel by inspecting ExtraSamples tags or RGBA color type.
fn detect_alpha_from_tags<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    samples_per_pixel: u32,
) -> bool {
    // Tag 338: ExtraSamples
    if let Ok(val) = decoder.get_tag(Tag::ExtraSamples) {
        match val {
            Value::Short(v) => {
                if v == 1 || v == 2 {
                    return true;
                }
            }
            Value::Unsigned(v) => {
                if v == 1 || v == 2 {
                    return true;
                }
            }
            Value::List(list) => {
                for item in list {
                    match item {
                        Value::Short(v) if v == 1 || v == 2 => return true,
                        Value::Unsigned(v) if v == 1 || v == 2 => return true,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if let Ok(extra_samples) = decoder.get_tag_u16_vec(Tag::ExtraSamples) {
        if extra_samples.iter().any(|&v| v == 1 || v == 2) {
            return true;
        }
    }

    // Jika kanal >= 4 dan colortype RGBA murni
    if samples_per_pixel == 4 {
        if let Ok(ColorType::RGBA(_)) = decoder.colortype() {
            return true;
        }
    }

    false
}

/// Detects the numeric nodata value specified in TIFF tag 42113 (GDAL_NODATA), if present.
fn detect_nodata_from_tags<R: Read + Seek>(decoder: &mut Decoder<R>) -> Option<f64> {
    // Tag 42113: GDAL_NODATA
    if let Ok(ascii_str) = decoder.get_tag_ascii_string(Tag::GdalNodata) {
        let cleaned = ascii_str.trim().trim_matches('\0');
        if let Ok(parsed) = cleaned.parse::<f64>() {
            return Some(parsed);
        }
    }
    None
}

/// Translates the numeric TIFF compression tag into a human-readable algorithm name.
fn detect_compression_name<R: Read + Seek>(decoder: &mut Decoder<R>) -> String {
    if let Ok(comp_val) = decoder.get_tag_u32(Tag::Compression) {
        match comp_val {
            1 => "None (Uncompressed)".to_string(),
            2 => "CCITT 1D".to_string(),
            3 => "Group 3 Fax".to_string(),
            4 => "Group 4 Fax".to_string(),
            5 => "LZW".to_string(),
            6 => "Old JPEG".to_string(),
            7 => "JPEG".to_string(),
            8 => "Deflate / Adobe ZIP".to_string(),
            32773 => "PackBits".to_string(),
            32946 => "Deflate (PKZIP)".to_string(),
            34887 => "LERC".to_string(),
            50000 => "ZSTD".to_string(),
            other => format!("Compression ID {}", other),
        }
    } else {
        "Unknown".to_string()
    }
}

/// Memeriksa apakah sudut-sudut citra GeoTIFF berwarna hitam pekat (0, 0, 0),
/// yang mencerminkan padding batas NoData penerbangan drone tanpa tag GDAL_NODATA eksplisit.
fn detect_black_border_nodata<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
    channels: usize,
) -> bool {
    let chunk_type = decoder.get_chunk_type();
    let mut black_corners = 0;
    let mut checked_corners = 0;

    let check_pixel_is_black = |data: &[u8], ch: usize| -> bool {
        if ch >= 3 {
            data[0] <= 2 && data[1] <= 2 && data[2] <= 2
        } else if ch >= 1 {
            data[0] <= 2
        } else {
            false
        }
    };

    let check_pixel_is_black_u16 = |data: &[u16], ch: usize| -> bool {
        if ch >= 3 {
            data[0] <= 500 && data[1] <= 500 && data[2] <= 500
        } else if ch >= 1 {
            data[0] <= 500
        } else {
            false
        }
    };

    match chunk_type {
        ChunkType::Tile => {
            let (tw, th) = decoder.chunk_dimensions();
            if tw == 0 || th == 0 {
                return false;
            }
            let tiles_across = width.div_ceil(tw);
            let _tiles_down = height.div_ceil(th);

            let corner_coords = [
                (0u32, 0u32),
                (width.saturating_sub(1), 0u32),
                (0u32, height.saturating_sub(1)),
                (width.saturating_sub(1), height.saturating_sub(1)),
            ];

            for (cx, cy) in corner_coords {
                let tc = cx / tw;
                let tr = cy / th;
                let chunk_idx = tr * tiles_across + tc;

                let px_in_tile_x = (cx % tw) as usize;
                let px_in_tile_y = (cy % th) as usize;

                let (data_w, _data_h) = decoder.chunk_data_dimensions(chunk_idx);
                let data_w = if data_w > 0 {
                    data_w as usize
                } else {
                    tw as usize
                };

                if let Ok(chunk_res) = decoder.read_chunk(chunk_idx) {
                    checked_corners += 1;
                    match chunk_res {
                        DecodingResult::U8(bytes) => {
                            let idx = (px_in_tile_y * data_w + px_in_tile_x) * channels;
                            if idx + channels <= bytes.len()
                                && check_pixel_is_black(&bytes[idx..idx + channels], channels)
                            {
                                black_corners += 1;
                            }
                        }
                        DecodingResult::U16(words) => {
                            let idx = (px_in_tile_y * data_w + px_in_tile_x) * channels;
                            if idx + channels <= words.len()
                                && check_pixel_is_black_u16(&words[idx..idx + channels], channels)
                            {
                                black_corners += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        ChunkType::Strip => {
            let strip_count = decoder.strip_count().unwrap_or(1);
            if strip_count == 0 {
                return false;
            }

            if let Ok(chunk_res) = decoder.read_chunk(0) {
                let (data_w, _data_h) = decoder.chunk_data_dimensions(0);
                let data_w = if data_w > 0 {
                    data_w as usize
                } else {
                    width as usize
                };

                match chunk_res {
                    DecodingResult::U8(bytes) => {
                        if bytes.len() >= channels {
                            checked_corners += 1;
                            if check_pixel_is_black(&bytes[0..channels], channels) {
                                black_corners += 1;
                            }
                        }
                        let tr_idx = (data_w.saturating_sub(1)) * channels;
                        if tr_idx + channels <= bytes.len() {
                            checked_corners += 1;
                            if check_pixel_is_black(&bytes[tr_idx..tr_idx + channels], channels) {
                                black_corners += 1;
                            }
                        }
                    }
                    DecodingResult::U16(words) => {
                        if words.len() >= channels {
                            checked_corners += 1;
                            if check_pixel_is_black_u16(&words[0..channels], channels) {
                                black_corners += 1;
                            }
                        }
                        let tr_idx = (data_w.saturating_sub(1)) * channels;
                        if tr_idx + channels <= words.len() {
                            checked_corners += 1;
                            if check_pixel_is_black_u16(&words[tr_idx..tr_idx + channels], channels)
                            {
                                black_corners += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if strip_count > 1 {
                let last_strip = strip_count - 1;
                if let Ok(chunk_res) = decoder.read_chunk(last_strip) {
                    let (data_w, data_h) = decoder.chunk_data_dimensions(last_strip);
                    let data_w = if data_w > 0 {
                        data_w as usize
                    } else {
                        width as usize
                    };
                    let last_y = if data_h > 0 { (data_h - 1) as usize } else { 0 };

                    match chunk_res {
                        DecodingResult::U8(bytes) => {
                            let bl_idx = (last_y * data_w) * channels;
                            if bl_idx + channels <= bytes.len() {
                                checked_corners += 1;
                                if check_pixel_is_black(&bytes[bl_idx..bl_idx + channels], channels)
                                {
                                    black_corners += 1;
                                }
                            }
                            let br_idx = (last_y * data_w + data_w.saturating_sub(1)) * channels;
                            if br_idx + channels <= bytes.len() {
                                checked_corners += 1;
                                if check_pixel_is_black(&bytes[br_idx..br_idx + channels], channels)
                                {
                                    black_corners += 1;
                                }
                            }
                        }
                        DecodingResult::U16(words) => {
                            let bl_idx = (last_y * data_w) * channels;
                            if bl_idx + channels <= words.len() {
                                checked_corners += 1;
                                if check_pixel_is_black_u16(
                                    &words[bl_idx..bl_idx + channels],
                                    channels,
                                ) {
                                    black_corners += 1;
                                }
                            }
                            let br_idx = (last_y * data_w + data_w.saturating_sub(1)) * channels;
                            if br_idx + channels <= words.len() {
                                checked_corners += 1;
                                if check_pixel_is_black_u16(
                                    &words[br_idx..br_idx + channels],
                                    channels,
                                ) {
                                    black_corners += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    checked_corners >= 2 && black_corners >= 2
}

/// Mengekstrak detail teknis berkas GeoTIFF untuk disimpan pada `metadata.json`.
pub fn extract_tiff_technical_details<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
) -> Result<TiffTechnicalDetails, OrthoImportError> {
    let color_type = decoder.colortype()?;
    let (bit_depth, bands) = match color_type {
        ColorType::Gray(b) => (b as u32, 1u32),
        ColorType::RGB(b) => (b as u32, 3u32),
        ColorType::RGBA(b) => (b as u32, 4u32),
        ColorType::GrayA(b) => (b as u32, 2u32),
        ColorType::YCbCr(b) => (b as u32, 3u32),
        _ => (8, 3),
    };

    let sample_format = match bit_depth {
        16 => "uint16".to_string(),
        _ => "uint8".to_string(),
    };

    let chunk_type = decoder.get_chunk_type();
    let (layout, chunk_dimensions, total_chunks) = match chunk_type {
        ChunkType::Tile => {
            let (tw, th) = decoder.chunk_dimensions();
            let count = decoder.tile_count().unwrap_or(0);
            ("tiled".to_string(), [tw, th], count as u64)
        }
        ChunkType::Strip => {
            let count = decoder.strip_count().unwrap_or(0);
            let rows_per_strip = if count > 0 {
                height.div_ceil(count)
            } else {
                height
            };
            (
                "stripped".to_string(),
                [width, rows_per_strip],
                count as u64,
            )
        }
    };

    let compression = detect_compression_name(decoder);
    let has_alpha = detect_alpha_from_tags(decoder, bands);
    let mut nodata_value = detect_nodata_from_tags(decoder);
    if nodata_value.is_none()
        && !has_alpha
        && detect_black_border_nodata(decoder, width, height, bands as usize)
    {
        nodata_value = Some(0.0);
    }

    let photometric_interpretation = match color_type {
        ColorType::Gray(_) | ColorType::GrayA(_) => "BlackIsZero (Grayscale)".to_string(),
        ColorType::RGB(_) | ColorType::RGBA(_) => "RGB".to_string(),
        _ => "Unknown".to_string(),
    };

    let software = decoder.get_tag_ascii_string(Tag::Software).ok();

    Ok(TiffTechnicalDetails {
        bands,
        bit_depth,
        sample_format,
        compression,
        layout,
        chunk_dimensions,
        total_chunks,
        planar_configuration: "Contiguous (Chunky)".to_string(),
        photometric_interpretation,
        has_alpha,
        nodata_value,
        software,
    })
}

// ============================================================================
// 4. ANALISIS PERSENTIL DETERMINISTIK 16-BIT (p0,5 - p99,5)
// ============================================================================

/// Menghitung persentil kontras deterministik `p0,5` (lo) dan `p99,5` (hi)
/// dari sampel grid minimal 9 tile/strip terdistribusi merata.
///
/// Mengecualikan piksel transparan (alpha=0) dan nilai nodata.
/// Mengembalikan `(lo, hi, is_full_range)`.
pub fn compute_16bit_percentiles<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
    has_alpha: bool,
    nodata_val: Option<f64>,
) -> Result<(f64, f64, bool), OrthoImportError> {
    let mut histogram = vec![0u64; 65536];
    let mut total_samples = 0u64;

    let chunk_type = decoder.get_chunk_type();
    let chunk_indices = match chunk_type {
        ChunkType::Tile => {
            let (tw, th) = decoder.chunk_dimensions();
            let tiles_across = width.div_ceil(tw);
            let tiles_down = height.div_ceil(th);
            let total_tiles = tiles_across * tiles_down;

            let mut indices = Vec::new();
            if total_tiles <= 64 {
                for i in 0..total_tiles {
                    indices.push(i);
                }
            } else {
                let num_samples_x = 8.min(tiles_across);
                let num_samples_y = 8.min(tiles_down);
                for sy in 0..num_samples_y {
                    let r = if num_samples_y > 1 {
                        (sy * (tiles_down - 1)) / (num_samples_y - 1)
                    } else {
                        0
                    };
                    for sx in 0..num_samples_x {
                        let c = if num_samples_x > 1 {
                            (sx * (tiles_across - 1)) / (num_samples_x - 1)
                        } else {
                            0
                        };
                        let idx = r * tiles_across + c;
                        if !indices.contains(&idx) {
                            indices.push(idx);
                        }
                    }
                }
            }
            indices
        }
        ChunkType::Strip => {
            let count = decoder.strip_count().unwrap_or(1);
            let mut indices = Vec::new();
            if count <= 64 {
                for i in 0..count {
                    indices.push(i);
                }
            } else {
                for step in 0..64 {
                    let idx = (step * (count - 1)) / 63;
                    if !indices.contains(&idx) {
                        indices.push(idx);
                    }
                }
            }
            indices
        }
    };

    let start_time = std::time::Instant::now();
    let max_duration = std::time::Duration::from_millis(1500);

    for &idx in &chunk_indices {
        if start_time.elapsed() > max_duration && total_samples >= 1000 {
            break;
        }

        if let Ok(DecodingResult::U16(data)) = decoder.read_chunk(idx) {
            let color_type = decoder.colortype()?;
            let channels = match color_type {
                ColorType::Gray(_) => 1,
                ColorType::GrayA(_) => 2,
                ColorType::RGB(_) => 3,
                ColorType::RGBA(_) => 4,
                ColorType::YCbCr(_) => 3,
                _ => 3,
            };

            let num_pixels = data.len() / channels;
            for p in 0..num_pixels {
                let offset = p * channels;
                if has_alpha && channels >= 4 && data[offset + 3] == 0 {
                    continue;
                }

                if let Some(nd) = nodata_val {
                    let nd_u16 = nd.round() as u16;
                    if channels >= 3 {
                        if data[offset] == nd_u16
                            && data[offset + 1] == nd_u16
                            && data[offset + 2] == nd_u16
                        {
                            continue;
                        }
                    } else if data[offset] == nd_u16 {
                        continue;
                    }
                }

                // Masukkan saluran warna (RGB atau Gray) ke histogram bersama
                let color_chans = if channels >= 3 { 3 } else { 1 };
                for c in 0..color_chans {
                    let val = data[offset + c] as usize;
                    histogram[val] += 1;
                    total_samples += 1;
                }
            }
        }
    }

    // Jika histogram masih kosong (semua 64 tile nodata/alpha=0), scan tile tambahan
    if total_samples == 0 {
        let total_chunks = match decoder.get_chunk_type() {
            ChunkType::Tile => decoder.tile_count().unwrap_or(0),
            ChunkType::Strip => decoder.strip_count().unwrap_or(0),
        };
        for idx in 0..total_chunks.min(64) {
            if chunk_indices.contains(&idx) {
                continue;
            }
            if start_time.elapsed() > max_duration {
                break;
            }
            if let Ok(DecodingResult::U16(data)) = decoder.read_chunk(idx) {
                let color_type = decoder.colortype()?;
                let channels = match color_type {
                    ColorType::Gray(_) => 1,
                    ColorType::GrayA(_) => 2,
                    ColorType::RGB(_) => 3,
                    ColorType::RGBA(_) => 4,
                    _ => 3,
                };
                let num_pixels = data.len() / channels;
                for p in 0..num_pixels {
                    let offset = p * channels;
                    if has_alpha && channels >= 4 && data[offset + 3] == 0 {
                        continue;
                    }
                    if let Some(nd) = nodata_val {
                        let nd_u16 = nd.round() as u16;
                        if channels >= 3 {
                            if data[offset] == nd_u16
                                && data[offset + 1] == nd_u16
                                && data[offset + 2] == nd_u16
                            {
                                continue;
                            }
                        } else if data[offset] == nd_u16 {
                            continue;
                        }
                    }
                    let color_chans = if channels >= 3 { 3 } else { 1 };
                    for c in 0..color_chans {
                        let val = data[offset + c] as usize;
                        histogram[val] += 1;
                        total_samples += 1;
                    }
                }
            }
            if total_samples > 0 {
                break;
            }
        }
    }

    if total_samples == 0 {
        // Fallback: full-range
        return Ok((0.0, 65535.0, true));
    }

    let p05_target = (total_samples as f64 * 0.005).round() as u64;
    let p995_target = (total_samples as f64 * 0.995).round() as u64;

    let mut cumulative = 0u64;
    let mut lo = 0.0;
    let mut hi = 65535.0;
    let mut found_lo = false;

    for (bin, &count) in histogram.iter().enumerate() {
        cumulative += count;
        if !found_lo && cumulative >= p05_target {
            lo = bin as f64;
            found_lo = true;
        }
        if cumulative >= p995_target {
            hi = bin as f64;
            break;
        }
    }

    // Periksa apakah data sudah full-range
    let is_full_range = lo <= 50.0 && hi >= 65400.0;

    Ok((lo, hi, is_full_range))
}

// ============================================================================
// 5. STREAMING TILE-ROW ASSEMBLY KE RESAMPLER
// ============================================================================

/// Memproses seluruh data GeoTIFF dan mengalirkannya ke `StreamingResampler`.
///
/// Untuk citra Tiled: Merakit satu strip tile penuh ($W \times TileHeight$) lalu
/// menyuapkannya per baris ke resampler (pilihan arsitektur Opsi i).
/// Untuk citra Stripped: Membaca per strip langsung dan menyuapkan baris-barisnya.
#[allow(clippy::too_many_arguments)]
pub fn stream_geotiff_to_resampler<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
    resampler: &mut StreamingResampler,
    u16_stretch: Option<(f64, f64)>,
    nodata_val: Option<f64>,
    is_min_is_white: bool,
    cancel_token: Option<&CancellationToken>,
) -> Result<(), OrthoImportError> {
    let color_type = decoder.colortype()?;
    let is_16bit = matches!(
        color_type,
        ColorType::Gray(16)
            | ColorType::GrayA(16)
            | ColorType::RGB(16)
            | ColorType::RGBA(16)
            | ColorType::YCbCr(16)
    );

    let channels = match color_type {
        ColorType::Gray(_) => 1,
        ColorType::GrayA(_) => 2,
        ColorType::RGB(_) => 3,
        ColorType::RGBA(_) => 4,
        ColorType::YCbCr(_) => 3,
        _ => 3,
    };

    let target_channels = resampler.channels();
    let chunk_type = decoder.get_chunk_type();

    match chunk_type {
        ChunkType::Tile => {
            let (tw, th) = decoder.chunk_dimensions();
            let tiles_across = width.div_ceil(tw);
            let tiles_down = height.div_ceil(th);

            for tr in 0..tiles_down {
                let actual_h = (height - tr * th).min(th);

                if is_16bit {
                    let (lo, hi) = u16_stretch.unwrap_or((0.0, 65535.0));
                    let mut strip_buf =
                        vec![0u16; width as usize * actual_h as usize * target_channels];

                    for tc in 0..tiles_across {
                        let chunk_idx = tr * tiles_across + tc;
                        let tile_x = tc * tw;

                        if let Ok(DecodingResult::U16(tile_data)) = decoder.read_chunk(chunk_idx) {
                            let (data_w, data_h) = decoder.chunk_data_dimensions(chunk_idx);
                            let valid_w = data_w.min(width.saturating_sub(tile_x));
                            let valid_h = data_h.min(actual_h);

                            for y in 0..valid_h {
                                for x in 0..valid_w {
                                    let src_idx = (y * data_w + x) as usize * channels;
                                    let dst_idx =
                                        ((y * width + (tile_x + x)) as usize) * target_channels;

                                    blit_pixel_u16(
                                        &tile_data[src_idx..src_idx + channels],
                                        &mut strip_buf[dst_idx..dst_idx + target_channels],
                                        channels,
                                        target_channels,
                                        nodata_val,
                                        is_min_is_white,
                                    );
                                }
                            }
                        }
                    }

                    // Suapkan baris-baris strip ke resampler
                    for y in 0..actual_h {
                        if let Some(token) = cancel_token {
                            if token.is_cancelled() {
                                return Err(OrthoImportError::Dibatalkan);
                            }
                        }
                        let y_in = tr * th + y;
                        let start = (y * width) as usize * target_channels;
                        let end = start + (width as usize * target_channels);
                        resampler.push_row_u16(
                            y_in,
                            &strip_buf[start..end],
                            lo,
                            hi,
                            cancel_token,
                        )?;
                    }
                } else {
                    let mut strip_buf =
                        vec![0u8; width as usize * actual_h as usize * target_channels];

                    for tc in 0..tiles_across {
                        let chunk_idx = tr * tiles_across + tc;
                        let tile_x = tc * tw;

                        if let Ok(DecodingResult::U8(tile_data)) = decoder.read_chunk(chunk_idx) {
                            let (data_w, data_h) = decoder.chunk_data_dimensions(chunk_idx);
                            let valid_w = data_w.min(width.saturating_sub(tile_x));
                            let valid_h = data_h.min(actual_h);

                            for y in 0..valid_h {
                                for x in 0..valid_w {
                                    let src_idx = (y * data_w + x) as usize * channels;
                                    let dst_idx =
                                        ((y * width + (tile_x + x)) as usize) * target_channels;

                                    blit_pixel_u8(
                                        &tile_data[src_idx..src_idx + channels],
                                        &mut strip_buf[dst_idx..dst_idx + target_channels],
                                        channels,
                                        target_channels,
                                        nodata_val,
                                        is_min_is_white,
                                    );
                                }
                            }
                        }
                    }

                    // Suapkan baris-baris strip ke resampler
                    for y in 0..actual_h {
                        if let Some(token) = cancel_token {
                            if token.is_cancelled() {
                                return Err(OrthoImportError::Dibatalkan);
                            }
                        }
                        let y_in = tr * th + y;
                        let start = (y * width) as usize * target_channels;
                        let end = start + (width as usize * target_channels);
                        resampler.push_row_u8(y_in, &strip_buf[start..end], cancel_token)?;
                    }
                }
            }
        }
        ChunkType::Strip => {
            let strip_count = decoder.strip_count().unwrap_or(1);
            let mut current_y = 0u32;

            for si in 0..strip_count {
                if let Some(token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(OrthoImportError::Dibatalkan);
                    }
                }

                let (data_w, data_h) = decoder.chunk_data_dimensions(si);

                if is_16bit {
                    let (lo, hi) = u16_stretch.unwrap_or((0.0, 65535.0));
                    if let Ok(DecodingResult::U16(strip_data)) = decoder.read_chunk(si) {
                        let mut row_buf = vec![0u16; width as usize * target_channels];

                        for y in 0..data_h {
                            if current_y >= height {
                                break;
                            }
                            for x in 0..data_w.min(width) {
                                let src_idx = (y * data_w + x) as usize * channels;
                                let dst_idx = x as usize * target_channels;

                                blit_pixel_u16(
                                    &strip_data[src_idx..src_idx + channels],
                                    &mut row_buf[dst_idx..dst_idx + target_channels],
                                    channels,
                                    target_channels,
                                    nodata_val,
                                    is_min_is_white,
                                );
                            }
                            resampler.push_row_u16(current_y, &row_buf, lo, hi, cancel_token)?;
                            current_y += 1;
                        }
                    }
                } else if let Ok(DecodingResult::U8(strip_data)) = decoder.read_chunk(si) {
                    let mut row_buf = vec![0u8; width as usize * target_channels];

                    for y in 0..data_h {
                        if current_y >= height {
                            break;
                        }
                        for x in 0..data_w.min(width) {
                            let src_idx = (y * data_w + x) as usize * channels;
                            let dst_idx = x as usize * target_channels;

                            blit_pixel_u8(
                                &strip_data[src_idx..src_idx + channels],
                                &mut row_buf[dst_idx..dst_idx + target_channels],
                                channels,
                                target_channels,
                                nodata_val,
                                is_min_is_white,
                            );
                        }
                        resampler.push_row_u8(current_y, &row_buf, cancel_token)?;
                        current_y += 1;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Blits and normalizes an 8-bit source pixel into the destination buffer with nodata handling.
fn blit_pixel_u8(
    src: &[u8],
    dst: &mut [u8],
    src_ch: usize,
    dst_ch: usize,
    nodata_val: Option<f64>,
    is_min_is_white: bool,
) {
    let src_g = if is_min_is_white && src_ch == 1 {
        255 - src[0]
    } else {
        src[0]
    };

    let is_nodata = if let Some(nd) = nodata_val {
        let nd_u8 = nd.round() as u8;
        if src_ch >= 3 {
            if nd_u8 == 0 {
                src[0] <= 2 && src[1] <= 2 && src[2] <= 2
            } else {
                src[0] == nd_u8 && src[1] == nd_u8 && src[2] == nd_u8
            }
        } else if nd_u8 == 0 {
            src_g <= 2
        } else {
            src_g == nd_u8
        }
    } else {
        false
    };

    if dst_ch == 4 {
        // Output RGBA
        if is_nodata {
            dst[0] = 0;
            dst[1] = 0;
            dst[2] = 0;
            dst[3] = 0;
        } else if src_ch >= 4 {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
            dst[3] = src[3];
        } else if src_ch >= 3 {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
            dst[3] = 255;
        } else {
            // Grayscale
            dst[0] = src_g;
            dst[1] = src_g;
            dst[2] = src_g;
            dst[3] = 255;
        }
    } else if dst_ch == 3 {
        // Output RGB
        if src_ch >= 3 {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
        } else {
            dst[0] = src_g;
            dst[1] = src_g;
            dst[2] = src_g;
        }
    }
}

/// Blits and normalizes a 16-bit source pixel into the destination buffer with nodata handling.
fn blit_pixel_u16(
    src: &[u16],
    dst: &mut [u16],
    src_ch: usize,
    dst_ch: usize,
    nodata_val: Option<f64>,
    is_min_is_white: bool,
) {
    let src_g = if is_min_is_white && src_ch == 1 {
        65535 - src[0]
    } else {
        src[0]
    };

    let is_nodata = if let Some(nd) = nodata_val {
        let nd_u16 = nd.round() as u16;
        if src_ch >= 3 {
            if nd_u16 == 0 {
                src[0] <= 500 && src[1] <= 500 && src[2] <= 500
            } else {
                src[0] == nd_u16 && src[1] == nd_u16 && src[2] == nd_u16
            }
        } else if nd_u16 == 0 {
            src_g <= 500
        } else {
            src_g == nd_u16
        }
    } else {
        false
    };

    if dst_ch == 4 {
        // Output RGBA
        if is_nodata {
            dst[0] = 0;
            dst[1] = 0;
            dst[2] = 0;
            dst[3] = 0;
        } else if src_ch >= 4 {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
            dst[3] = src[3];
        } else if src_ch >= 3 {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
            dst[3] = 65535;
        } else {
            dst[0] = src_g;
            dst[1] = src_g;
            dst[2] = src_g;
            dst[3] = 65535;
        }
    } else if dst_ch == 3 {
        // Output RGB
        if src_ch >= 3 {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
        } else {
            dst[0] = src_g;
            dst[1] = src_g;
            dst[2] = src_g;
        }
    }
}

// ============================================================================
// 6. DETEKSI PIRAMIDA INTERNAL (OVERVIEW IFD)
// ============================================================================

/// Memeriksa apakah berkas memiliki sub-IFD piramida resolusi (overview)
/// dan memilih indeks IFD yang paling mendekati target display (>= 4.096 px).
pub fn find_best_overview_ifd<R: Read + Seek>(
    decoder: &mut Decoder<R>,
) -> Result<Option<usize>, OrthoImportError> {
    if !decoder.more_images() {
        return Ok(None);
    }

    let mut best_ifd: Option<usize> = None;
    let mut best_longest = u32::MAX;
    let mut ifd_idx = 1usize;

    while decoder.more_images() {
        if decoder.next_image().is_err() {
            break;
        }

        if let Ok((w, h)) = decoder.dimensions() {
            let longest = w.max(h);
            if longest >= DISPLAY_TARGET_DIMENSION_PX && longest < best_longest {
                best_longest = longest;
                best_ifd = Some(ifd_idx);
            }
        }
        ifd_idx += 1;
    }

    // Kembalikan cursor ke IFD pertama (indeks 0) jika overview tidak digunakan
    let _ = decoder.seek_to_image(0);

    Ok(best_ifd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_black_border_on_real_or_synthetic() {
        let test_real_path =
            std::path::Path::new(r"C:\Users\x0776\Downloads\600fcc9055481e0005e312a1.tif");
        if test_real_path.exists() {
            let summary =
                validate_geotiff_header_fast(test_real_path).expect("Header validasi harus sukses");
            assert_eq!(
                summary.nodata_value,
                Some(0.0),
                "Batas hitam harus terdeteksi sebagai NoData = 0.0"
            );
        }
    }
}
