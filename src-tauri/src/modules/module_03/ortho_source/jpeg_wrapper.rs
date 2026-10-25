//! Wrapper tipis terisolasi untuk crate `jpeg-decoder` (=0.3.2).
//!
//! Alasan pembungkusan:
//! Crate `jpeg-decoder` saat ini berada dalam status maintenance mode oleh upstream (image-rs),
//! namun sengaja dipertahankan dan di-pin pada versi `=0.3.2` karena merupakan satu-satunya
//! decoder JPEG pure Rust yang menyediakan fitur dekuantisasi DCT-domain scaling (`scale(w, h)`).
//! Modul ini membatasi ketergantungan langsung kode aplikasi terhadap API internal `jpeg-decoder`,
//! sehingga memudahkan migrasi ke decoder masa depan (misal zune-jpeg jika kelak mendukung
//! sub-sampling DCT, atau decoder kustom).

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

    let longest = orig_w.max(orig_h);
    let (scaled_w, scaled_h) = if longest > max_target_dim && max_target_dim > 0 {
        let scale = max_target_dim as f32 / longest as f32;
        let tw = ((orig_w as f32 * scale).round() as u16).max(1);
        let th = ((orig_h as f32 * scale).round() as u16).max(1);
        decoder.scale(tw, th).map_err(|e| {
            OrthoImportError::InternalError(format!("Gagal mengatur IDCT scale: {}", e))
        })?
    } else {
        (orig_w as u16, orig_h as u16)
    };

    let pixels = decoder
        .decode()
        .map_err(|e| OrthoImportError::FileKorup(format!("Gagal mendecode JPEG: {}", e)))?;

    Ok(JpegIdctDecoded {
        width: scaled_w as u32,
        height: scaled_h as u32,
        pixels,
    })
}
