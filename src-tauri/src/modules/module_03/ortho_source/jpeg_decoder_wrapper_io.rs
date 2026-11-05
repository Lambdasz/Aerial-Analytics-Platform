//! Berkas ini berisi wrapper terisolasi untuk crate `jpeg-decoder` guna mendukung dekuantisasi DCT-domain scaling (`scale(w, h)`).
//!
//! Daftar Struktur & Fungsi:
//! - `JpegIdctDecoded`: Struktur penyimpan dimensi dan buffer piksel hasil decode IDCT.
//! - `decode_jpeg_with_idct_scale`: Mendecode berkas JPEG langsung ke skala 1/2, 1/4, atau 1/8 untuk menghemat memori (dengan penanganan khusus Progressive JPEG).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::modules::module_03::error::OrthoImportError;

/// Hasil dekompresi JPEG pada domain DCT berskala.
pub struct JpegIdctDecoded {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Mendecode berkas JPEG dengan IDCT scaling faktor 1/2, 1/4, atau 1/8
/// berdasarkan rasio terhadap `max_target_dim`.
///
/// Catatan Penting:
/// Jika citra berformat Progressive JPEG (SOF2), `decoder.scale()` sengaja TIDAK dipanggil
/// karena `jpeg-decoder` menolak scaling pada progressive scan (`ProgressiveWithScale`).
/// Citra progressive akan didekode penuh, lalu dialirkan ke `StreamingResampler` untuk downscaling.
pub fn decode_jpeg_with_idct_scale(
    source_path: &Path,
    max_target_dim: u32,
) -> Result<JpegIdctDecoded, OrthoImportError> {
    let file = File::open(source_path)
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal membuka berkas JPEG: {}", e)))?;
    let buf_reader = BufReader::new(file);

    let mut decoder = jpeg_decoder::Decoder::new(buf_reader);
    decoder.read_info().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca info header JPEG: {}", e))
    })?;

    let info = decoder
        .info()
        .ok_or_else(|| OrthoImportError::FileKorup("Header JPEG tidak memuat info".to_string()))?;
    let (orig_w, orig_h) = (info.width as u32, info.height as u32);

    let is_progressive = info.coding_process == jpeg_decoder::CodingProcess::DctProgressive;

    // Untuk Progressive JPEG: jangan panggil decoder.scale() karena crate jpeg-decoder
    // secara eksplisit menolak scaling pada progressive scan (ProgressiveWithScale error).
    // Sebagai gantinya, biarkan didecode penuh, kemudian downscaling dilakukan oleh StreamingResampler.
    let (actual_w, actual_h) =
        if !is_progressive && orig_w.max(orig_h) > max_target_dim && max_target_dim > 0 {
            let longest = orig_w.max(orig_h);
            let scale = max_target_dim as f32 / longest as f32;
            let tw = ((orig_w as f32 * scale).round() as u16).max(1);
            let th = ((orig_h as f32 * scale).round() as u16).max(1);
            decoder.scale(tw, th).map_err(|e| {
                OrthoImportError::InternalError(format!("Gagal mengatur IDCT scale: {}", e))
            })?
        } else {
            (orig_w as u16, orig_h as u16)
        };

    let mut pixels = decoder
        .decode()
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal mendecode JPEG: {}", e)))?;

    // Normalisasi format piksel: jika Grayscale (L8), ekspansi ke RGB 8-bit (3 byte per piksel)
    if info.pixel_format == jpeg_decoder::PixelFormat::L8 {
        let mut rgb = Vec::with_capacity(pixels.len() * 3);
        for p in pixels {
            rgb.extend_from_slice(&[p, p, p]);
        }
        pixels = rgb;
    }

    Ok(JpegIdctDecoded {
        width: actual_w as u32,
        height: actual_h as u32,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_progressive_jpeg_decodes_without_crash() {
        // Citra sintetis Progressive JPEG 16x16 valid
        let prog_jpeg_bytes: [u8; 520] = [
            255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 255, 219, 0,
            67, 0, 8, 6, 6, 7, 6, 5, 8, 7, 7, 7, 9, 9, 8, 10, 12, 20, 13, 12, 11, 11, 12, 25, 18,
            19, 15, 20, 29, 26, 31, 30, 29, 26, 28, 28, 32, 36, 46, 39, 32, 34, 44, 35, 28, 28, 40,
            55, 41, 44, 48, 49, 52, 52, 52, 31, 39, 57, 61, 56, 50, 60, 46, 51, 52, 50, 255, 219,
            0, 67, 1, 9, 9, 9, 12, 11, 12, 24, 13, 13, 24, 50, 33, 28, 33, 50, 50, 50, 50, 50, 50,
            50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50,
            50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50,
            255, 194, 0, 17, 8, 0, 16, 0, 16, 3, 1, 34, 0, 2, 17, 1, 3, 17, 1, 255, 196, 0, 21, 0,
            1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 255, 196, 0, 20, 1, 1, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 255, 218, 0, 12, 3, 1, 0, 2, 16, 3, 16, 0, 0, 1,
            144, 99, 31, 255, 196, 0, 20, 16, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32,
            255, 218, 0, 8, 1, 1, 0, 1, 5, 2, 31, 255, 196, 0, 20, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 255, 218, 0, 8, 1, 3, 1, 1, 63, 1, 127, 255, 196, 0, 20, 17, 1,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 218, 0, 8, 1, 2, 1, 1, 63, 1, 127,
            255, 196, 0, 20, 16, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 255, 218, 0,
            8, 1, 1, 0, 6, 63, 2, 31, 255, 196, 0, 20, 16, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 32, 255, 218, 0, 8, 1, 1, 0, 1, 63, 33, 31, 255, 218, 0, 12, 3, 1, 0, 2, 0, 3,
            0, 0, 0, 16, 11, 255, 196, 0, 20, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 255, 218, 0, 8, 1, 3, 1, 1, 63, 16, 127, 255, 196, 0, 20, 17, 1, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 218, 0, 8, 1, 2, 1, 1, 63, 16, 127, 255, 196, 0, 20,
            16, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 255, 218, 0, 8, 1, 1, 0, 1, 63,
            16, 31, 255, 217,
        ];

        let temp_dir = std::env::temp_dir().join(format!("test_prog_jpg_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("test_prog.jpg");
        {
            let mut f = File::create(&test_file).expect("Gagal membuat test file");
            f.write_all(&prog_jpeg_bytes)
                .expect("Gagal menulis progressive jpeg");
        }

        // Uji pemanggilan dengan target_dim lebih kecil (mis. 8)
        let res = decode_jpeg_with_idct_scale(&test_file, 8);
        assert!(
            res.is_ok(),
            "Harus berhasil mendecode progressive JPEG tanpa crash ProgressiveWithScale: {:?}",
            res.err()
        );
        let decoded = res.unwrap();
        assert_eq!(decoded.width, 16);
        assert_eq!(decoded.height, 16);
        assert_eq!(decoded.pixels.len(), 16 * 16 * 3);

        let _ = std::fs::remove_file(test_file);
        let _ = std::fs::remove_dir(temp_dir);
    }
}
