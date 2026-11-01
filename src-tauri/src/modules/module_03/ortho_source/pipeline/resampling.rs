//! Logika decoding dan resampling gambar untuk Module 03 Ortho Pipeline.
//!
//! Menyediakan deteksi progressive JPEG, IDCT-scaled decoding, streaming PNG decoding,
//! dan box-filter resampling ke dimensi target.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::ortho_source::constants::{
    MAX_IMAGE_DIMENSION_PX, MAX_PROGRESSIVE_JPEG_PIXELS,
};
use crate::modules::module_03::ortho_source::jpeg_wrapper::decode_jpeg_with_idct_scale;
use crate::modules::module_03::ortho_source::streaming_resampler::{
    ChannelCount, StreamingResampler,
};

/// Mendeteksi apakah berkas JPEG bertipe progressive (SOF2 marker) dan mengekstrak dimensi.
/// Menolak JPEG progressive > 40 MP secara instan tanpa mengalokasikan RAM buffer piksel.
pub fn detect_jpeg_progressive_and_dimensions(
    path: &Path,
) -> Result<(bool, u32, u32), OrthoImportError> {
    let mut file = File::open(path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas JPEG: {}", e)))?;
    let mut buf = Vec::new();
    Read::by_ref(&mut file)
        .take(131072) // Baca header sampai 128 KB
        .read_to_end(&mut buf)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca header JPEG: {}", e)))?;

    if buf.len() < 4 || buf[0] != 0xFF || buf[1] != 0xD8 {
        return Err(OrthoImportError::FormatTidakDidukung(
            "Berkas bukan berkas JPEG yang valid".to_string(),
        ));
    }

    let mut i = 2;
    while i + 8 < buf.len() {
        if buf[i] != 0xFF {
            i += 1;
            continue;
        }

        let marker = buf[i + 1];
        // Lewati padding 0xFF berturut-turut
        if marker == 0xFF || marker == 0x00 {
            i += 1;
            continue;
        }

        // SOI, RST, EOI tidak memiliki panjang segmen
        if (0xD0..=0xD9).contains(&marker) {
            i += 2;
            continue;
        }

        let segment_len = ((buf[i + 2] as usize) << 8) | (buf[i + 3] as usize);

        // Penolakan Kasus Tepi: Arithmetic Coding (SOF9 = 0xC9, SOF10 = 0xCA, SOF11 = 0xCB, SOF13..=SOF15)
        if marker == 0xC9 || marker == 0xCA || marker == 0xCB || (0xCD..=0xCF).contains(&marker) {
            return Err(OrthoImportError::FormatTidakDidukung(
                "JPEG dengan pengkodean arithmetic (SOF9/SOF10) tidak didukung. Harap simpan citra dengan Huffman coding standar."
                    .to_string(),
            ));
        }

        // Marker SOF (Start of Frame) umum: 0xC0 (Baseline), 0xC1 (Extended), 0xC2 (Progressive), 0xC3 (Lossless)
        if (0xC0..=0xC7).contains(&marker) && marker != 0xC4 && i + 9 < buf.len() {
            let precision = buf[i + 4];
            if precision == 12 {
                return Err(OrthoImportError::FormatTidakDidukung(
                    "JPEG 12-bit tidak didukung. Hanya JPEG 8-bit yang didukung.".to_string(),
                ));
            }

            let components = buf[i + 9];
            if components == 4 {
                return Err(OrthoImportError::FormatTidakDidukung(
                    "JPEG dengan 4 komponen warna (CMYK/YCCK) tidak didukung. Harap konversi citra ke ruang warna RGB."
                        .to_string(),
                ));
            }
        }

        // SOF2: Progressive DCT
        if marker == 0xC2 {
            let height = ((buf[i + 5] as u32) << 8) | (buf[i + 6] as u32);
            let width = ((buf[i + 7] as u32) << 8) | (buf[i + 8] as u32);
            let total_pixels = width as u64 * height as u64;

            if total_pixels > MAX_PROGRESSIVE_JPEG_PIXELS {
                return Err(OrthoImportError::MelebihiBatas {
                    width,
                    height,
                    max_dimension: 6324,
                    message: format!(
                        "JPEG progressive dengan resolusi {}x{} (total {:.1} MP) melebihi batas aman 40 MP. Harap simpan citra sebagai baseline JPEG atau PNG.",
                        width, height, total_pixels as f64 / 1_000_000.0
                    ),
                });
            }

            return Ok((true, width, height));
        }

        // SOF0 (Baseline) atau SOF1 (Extended sequential)
        if marker == 0xC0 || marker == 0xC1 {
            let height = ((buf[i + 5] as u32) << 8) | (buf[i + 6] as u32);
            let width = ((buf[i + 7] as u32) << 8) | (buf[i + 8] as u32);
            return Ok((false, width, height));
        }

        i += 2 + segment_len;
    }

    // Jika tidak ditemukan dalam 128 KB pertama, gunakan fallback image::image_dimensions
    let (w, h) = image::image_dimensions(path).map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca dimensi berkas JPEG: {}", e))
    })?;
    Ok((false, w, h))
}

/// Mendecode baseline JPEG dengan IDCT Downscaling ke batas target (4.096 px)
/// lalu disuapkan per baris ke StreamingResampler (Box filter / Area Averaging)
/// tanpa mengalokasikan intermediate resize buffer.
pub fn decode_and_scale_jpeg_idct(
    source_path: &Path,
    target_dim: u32,
) -> Result<image::RgbImage, OrthoImportError> {
    let decoded = decode_jpeg_with_idct_scale(source_path, target_dim)?;
    let (scaled_w, scaled_h) = (decoded.width, decoded.height);

    let cur_longest = scaled_w.max(scaled_h);
    if cur_longest > target_dim && target_dim > 0 {
        let mut resampler =
            StreamingResampler::new(scaled_w, scaled_h, target_dim, ChannelCount::Rgb)?;
        let stride = scaled_w as usize * 3;
        for y in 0..scaled_h {
            let row_start = y as usize * stride;
            let row_end = row_start + stride;
            resampler.push_row_u8(y, &decoded.pixels[row_start..row_end], None)?;
        }
        let resampled = resampler.finish()?;
        image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data).ok_or_else(
            || {
                OrthoImportError::InternalError(
                    "Ukuran buffer hasil resampling JPEG tidak sesuai".to_string(),
                )
            },
        )
    } else {
        image::RgbImage::from_raw(scaled_w, scaled_h, decoded.pixels).ok_or_else(|| {
            OrthoImportError::InternalError("Ukuran buffer piksel JPEG tidak sesuai".to_string())
        })
    }
}

/// Mendecode PNG dan memeriksa channel alpha menggunakan StreamingResampler berbasis baris.
/// Mencegah alokasi buffer frame 100 MP penuh untuk PNG non-interlaced (Reader::next_row).
/// Untuk PNG interlaced (Adam7), batas aman ditetapkan <= 25 MP karena butuh frame penuh.
pub fn decode_and_scale_png(
    source_path: &Path,
    target_dim: u32,
) -> Result<(image::DynamicImage, bool), OrthoImportError> {
    let file = File::open(source_path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas PNG: {}", e)))?;
    let buf_reader = BufReader::new(file);

    let decoder = png::Decoder::new(buf_reader);
    let mut reader = decoder
        .read_info()
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca info PNG: {}", e)))?;

    let info = reader.info();
    let (orig_w, orig_h) = (info.width, info.height);
    let color_type = info.color_type;
    let is_interlaced = info.interlaced;

    // Penolakan Kasus Tepi: PNG 16-bit
    if info.bit_depth == png::BitDepth::Sixteen {
        return Err(OrthoImportError::FormatTidakDidukung(
            "PNG 16-bit tidak didukung pada Opsi A. Harap simpan citra sebagai PNG 8-bit atau gunakan Opsi B untuk citra sensor kedalaman tinggi."
                .to_string(),
        ));
    }

    // Penolakan Kasus Tepi: PNG Palette / Indexed
    if color_type == png::ColorType::Indexed {
        return Err(OrthoImportError::FormatTidakDidukung(
            "PNG Palette (Indexed Color) tidak didukung. Harap konversi citra ke format warna RGB atau RGBA."
                .to_string(),
        ));
    }

    let has_alpha =
        color_type == png::ColorType::Rgba || color_type == png::ColorType::GrayscaleAlpha;

    let total_pixels = orig_w as u64 * orig_h as u64;

    // Batas piksel Opsi A umum (144 MP)
    if orig_w > MAX_IMAGE_DIMENSION_PX
        || orig_h > MAX_IMAGE_DIMENSION_PX
        || total_pixels > 144_000_000
    {
        return Err(OrthoImportError::MelebihiBatas {
            width: orig_w,
            height: orig_h,
            max_dimension: MAX_IMAGE_DIMENSION_PX,
            message: format!(
                "Dimensi PNG {}x{} melebihi batas aman 144 MP.",
                orig_w, orig_h
            ),
        });
    }

    // Batas khusus PNG Interlaced (Adam7) <= 25 MP:
    // Karena Adam7 memerlukan dekompresi buffer frame penuh di memori untuk mengumpulkan semua pass.
    if is_interlaced && total_pixels > 25_000_000 {
        return Err(OrthoImportError::MelebihiBatas {
            width: orig_w,
            height: orig_h,
            max_dimension: 5000,
            message: format!(
                "PNG interlaced (Adam7) dengan resolusi {}x{} ({:.1} MP) melebihi batas aman 25 MP karena memerlukan alokasi frame penuh di memori. Harap simpan citra sebagai PNG non-interlaced atau JPEG.",
                orig_w, orig_h, total_pixels as f64 / 1_000_000.0
            ),
        });
    }

    let channel_count = if has_alpha {
        ChannelCount::Rgba
    } else {
        ChannelCount::Rgb
    };

    let mut resampler = StreamingResampler::new(orig_w, orig_h, target_dim, channel_count)?;

    if is_interlaced {
        // Interlaced (Adam7) <= 25 MP: decode frame penuh lalu disuapkan per baris ke resampler
        let mut buf = vec![0; reader.output_buffer_size()];
        let output_info = reader.next_frame(&mut buf).map_err(|e| {
            OrthoImportError::FileKorup(format!("Gagal mendecode frame PNG interlaced: {}", e))
        })?;
        buf.truncate(output_info.buffer_size());

        let stride = match color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            _ => 3,
        };

        for y in 0..orig_h {
            let row_start = (y as usize * orig_w as usize) * stride;
            let row_end = row_start + orig_w as usize * stride;
            let row_slice = &buf[row_start..row_end];

            match color_type {
                png::ColorType::Rgba | png::ColorType::Rgb => {
                    resampler.push_row_u8(y, row_slice, None)?;
                }
                png::ColorType::Grayscale => {
                    let mut rgb_row = Vec::with_capacity(orig_w as usize * 3);
                    for &g in row_slice {
                        rgb_row.extend_from_slice(&[g, g, g]);
                    }
                    resampler.push_row_u8(y, &rgb_row, None)?;
                }
                png::ColorType::GrayscaleAlpha => {
                    let mut rgba_row = Vec::with_capacity(orig_w as usize * 4);
                    for chunk in row_slice.as_chunks::<2>().0 {
                        rgba_row.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
                    }
                    resampler.push_row_u8(y, &rgba_row, None)?;
                }
                _ => {}
            }
        }
    } else {
        // Non-interlaced: streaming langsung per baris via reader.next_row()!
        // Alokasi memori HANYA 1 baris input (< 50 KB), bukan 400 MB!
        let mut y_in = 0u32;
        while let Some(row) = reader
            .next_row()
            .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membaca baris PNG: {}", e)))?
        {
            let row_slice = row.data();
            match color_type {
                png::ColorType::Rgba | png::ColorType::Rgb => {
                    resampler.push_row_u8(y_in, row_slice, None)?;
                }
                png::ColorType::Grayscale => {
                    let mut rgb_row = Vec::with_capacity(orig_w as usize * 3);
                    for &g in row_slice {
                        rgb_row.extend_from_slice(&[g, g, g]);
                    }
                    resampler.push_row_u8(y_in, &rgb_row, None)?;
                }
                png::ColorType::GrayscaleAlpha => {
                    let mut rgba_row = Vec::with_capacity(orig_w as usize * 4);
                    for chunk in row_slice.as_chunks::<2>().0 {
                        rgba_row.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
                    }
                    resampler.push_row_u8(y_in, &rgba_row, None)?;
                }
                _ => {}
            }
            y_in += 1;
        }
    }

    let resampled = resampler.finish()?;
    if has_alpha {
        let rgba = image::RgbaImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError(
                    "Buffer RGBA hasil resampling tidak sesuai".to_string(),
                )
            })?;
        Ok((image::DynamicImage::ImageRgba8(rgba), true))
    } else {
        let rgb = image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError(
                    "Buffer RGB hasil resampling tidak sesuai".to_string(),
                )
            })?;
        Ok((image::DynamicImage::ImageRgb8(rgb), false))
    }
}

/// Resample sebuah DynamicImage ke batas target menggunakan StreamingResampler (Box filter / Area Averaging).
pub fn resample_dynamic_image_box_filter(
    image: &image::DynamicImage,
    target_dim: u32,
) -> Result<image::DynamicImage, OrthoImportError> {
    let (orig_w, orig_h) = (image.width(), image.height());
    let longest = orig_w.max(orig_h);
    if longest <= target_dim || target_dim == 0 {
        return Ok(image.clone());
    }

    let has_alpha = image.color().has_alpha();
    let channel_count = if has_alpha {
        ChannelCount::Rgba
    } else {
        ChannelCount::Rgb
    };

    let mut resampler = StreamingResampler::new(orig_w, orig_h, target_dim, channel_count)?;
    let stride = orig_w as usize * channel_count.channels();

    if has_alpha {
        let rgba = image.to_rgba8();
        let raw = rgba.as_raw();
        for y in 0..orig_h {
            let start = y as usize * stride;
            let end = start + stride;
            resampler.push_row_u8(y, &raw[start..end], None)?;
        }
        let resampled = resampler.finish()?;
        let out_img = image::RgbaImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError("Buffer RGBA resample gagal".to_string())
            })?;
        Ok(image::DynamicImage::ImageRgba8(out_img))
    } else {
        let rgb = image.to_rgb8();
        let raw = rgb.as_raw();
        for y in 0..orig_h {
            let start = y as usize * stride;
            let end = start + stride;
            resampler.push_row_u8(y, &raw[start..end], None)?;
        }
        let resampled = resampler.finish()?;
        let out_img = image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
            .ok_or_else(|| {
                OrthoImportError::InternalError("Buffer RGB resample gagal".to_string())
            })?;
        Ok(image::DynamicImage::ImageRgb8(out_img))
    }
}

/// Resample sebuah RgbImage ke batas target menggunakan StreamingResampler (Box filter / Area Averaging).
pub fn resample_rgb_image_box_filter(
    rgb: &image::RgbImage,
    target_dim: u32,
) -> Result<image::RgbImage, OrthoImportError> {
    let (orig_w, orig_h) = (rgb.width(), rgb.height());
    let longest = orig_w.max(orig_h);
    if longest <= target_dim || target_dim == 0 {
        return Ok(rgb.clone());
    }

    let mut resampler = StreamingResampler::new(orig_w, orig_h, target_dim, ChannelCount::Rgb)?;
    let stride = orig_w as usize * 3;
    let raw = rgb.as_raw();

    for y in 0..orig_h {
        let start = y as usize * stride;
        let end = start + stride;
        resampler.push_row_u8(y, &raw[start..end], None)?;
    }

    let resampled = resampler.finish()?;
    image::RgbImage::from_raw(resampled.width, resampled.height, resampled.data)
        .ok_or_else(|| OrthoImportError::InternalError("Buffer RGB resample gagal".to_string()))
}
