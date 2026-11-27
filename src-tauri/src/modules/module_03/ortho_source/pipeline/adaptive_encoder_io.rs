//! Berkas ini berisi logika kompresi adaptif dan penulisan berkas display (JPEG & PNG banded).
//!
//! Daftar Struktur & Fungsi:
//! - `AdaptiveEncodeResult`: Struktur penyimpan metadata hasil encoding display (path, ukuran, dimensi, format, warning).
//! - `CancellableWriter`: Wrapper I/O writer yang memeriksa `CancellationToken` pada setiap blok penulisan.
//! - `encode_display_adaptive`: Dispatcher kompresi display adaptif berdasarkan tipe format citra.
//! - `encode_display_jpeg_adaptive`: Loop adaptif multi-pass JPEG (penurunan kualitas lalu tangga resolusi).
//! - `encode_display_png_alpha_adaptive`: Loop adaptif multi-pass PNG dengan transparansi Alpha.
//! - `encode_single_jpeg_pass`: Eksekusi penulisan berkas JPEG satu putaran dengan rasio kualitas tertentu.
//! - `encode_single_png_pass`: Eksekusi penulisan berkas PNG satu putaran dengan dimensi target.

use std::fs::{self, File};
use std::io::{BufWriter, Seek, Write};
use std::path::{Path, PathBuf};

use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::ortho_source::constants::{
    DISPLAY_MAX_BYTES_JPEG, DISPLAY_MAX_BYTES_PNG_ALPHA, DISPLAY_MIN_DIMENSION_PX,
    DISPLAY_TARGET_DIMENSION_PX, PNG_DIMENSION_LADDER,
};
use crate::modules::module_03::ortho_source::job_manager::CancellationToken;
use crate::modules::module_03::ortho_source::pipeline::image_resampling_logic::{
    resample_dynamic_image_box_filter, resample_rgb_image_box_filter,
};

/// Hasil dari loop kompresi adaptif display.
#[derive(Debug, Clone)]
pub struct AdaptiveEncodeResult {
    pub display_path: PathBuf,
    pub display_size_bytes: u64,
    pub final_dimensions: [u32; 2],
    pub format: String,
    pub warnings: Vec<String>,
}

/// Wrapper `std::io::Write` + `std::io::Seek` yang memeriksa `CancellationToken` pada setiap blok write.
/// Memungkinkan pembatalan segera (latensi rendah) bahkan saat proses encode JPEG / PNG
/// monolitik berukuran besar sedang berjalan.
pub struct CancellableWriter<'a, W: Write> {
    inner: W,
    token: Option<&'a CancellationToken>,
}

impl<'a, W: Write> CancellableWriter<'a, W> {
    /// Creates a new `CancellableWriter` wrapping an inner writer with an optional cancellation token.
    pub fn new(inner: W, token: Option<&'a CancellationToken>) -> Self {
        Self { inner, token }
    }
}

impl<'a, W: Write> Write for CancellableWriter<'a, W> {
    /// Writes buffer to inner writer, returning error if cancelled.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(t) = self.token {
            if t.is_cancelled() {
                return Err(std::io::Error::other(
                    "Operasi dibatalkan saat proses kompresi display",
                ));
            }
        }
        self.inner.write(buf)
    }

    /// Flushes inner writer, returning error if cancelled.
    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(t) = self.token {
            if t.is_cancelled() {
                return Err(std::io::Error::other(
                    "Operasi dibatalkan saat proses kompresi display",
                ));
            }
        }
        self.inner.flush()
    }
}

impl<'a, W: Write + Seek> Seek for CancellableWriter<'a, W> {
    /// Seeks inner writer, returning error if cancelled.
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        if let Some(t) = self.token {
            if t.is_cancelled() {
                return Err(std::io::Error::other(
                    "Operasi dibatalkan saat proses kompresi display",
                ));
            }
        }
        self.inner.seek(pos)
    }
}

/// Batas ukuran berkas display dalam bytes (MiB, basis 1024): JPEG 5 MiB, PNG ber-alpha 10 MiB.
pub fn display_limit_bytes(has_alpha: bool) -> u64 {
    if has_alpha {
        DISPLAY_MAX_BYTES_PNG_ALPHA
    } else {
        DISPLAY_MAX_BYTES_JPEG
    }
}

/// Anak tangga awal: dimensi tangga terbesar yang tidak melebihi sisi terpanjang sumber;
/// jika sumber lebih kecil dari lantai, kembalikan lantai (1.024 px).
pub fn png_initial_dimension(max_orig_dim: u32) -> u32 {
    PNG_DIMENSION_LADDER
        .iter()
        .copied()
        .find(|&d| d <= max_orig_dim)
        .unwrap_or(DISPLAY_MIN_DIMENSION_PX)
}

/// Anak tangga berikutnya setelah encode berukuran `size` pada dimensi `cur_dim`.
/// Estimasi kuadratik (ukuran ~ dimensi^2) dengan margin 5%, melompat ke anak tangga terdekat
/// yang diperkirakan muat; minimal turun satu anak tangga; tidak pernah di bawah lantai 1.024 px.
pub fn png_next_dimension(cur_dim: u32, size: u64, limit_bytes: u64) -> u32 {
    if cur_dim <= DISPLAY_MIN_DIMENSION_PX {
        return DISPLAY_MIN_DIMENSION_PX;
    }
    let ratio = (limit_bytes as f64 * 0.95 / size.max(1) as f64).sqrt();
    let est_dim = (cur_dim as f64 * ratio) as u32;
    PNG_DIMENSION_LADDER
        .iter()
        .copied()
        .filter(|&d| d < cur_dim)
        .find(|&d| d <= est_dim)
        .unwrap_or_else(|| {
            PNG_DIMENSION_LADDER
                .iter()
                .copied()
                .find(|&d| d < cur_dim)
                .unwrap_or(DISPLAY_MIN_DIMENSION_PX)
        })
}

/// Parameter encode PNG (kompresi dan filter).
#[derive(Debug, Clone, Copy)]
pub struct PngEncodeSettings {
    pub compression: png::Compression,
    pub filter: png::FilterType,
    pub adaptive: png::AdaptiveFilterType,
}

impl Default for PngEncodeSettings {
    /// Returns default PNG encoding settings (Default compression, Sub filter, Adaptive).
    fn default() -> Self {
        Self {
            compression: png::Compression::Default,
            filter: png::FilterType::Sub,
            adaptive: png::AdaptiveFilterType::Adaptive,
        }
    }
}

/// Jumlah baris per band saat menulis PNG; token pembatalan diperiksa di antara band.
pub const PNG_ENCODE_BAND_ROWS: usize = 64;

/// Encode RGBA8 ke PNG per band baris lewat `png::StreamWriter` sehingga pembatalan dapat
/// terjadi di antara band (bukan satu panggilan monolitik). Mengembalikan ukuran berkas (bytes).
pub fn encode_png_banded(
    image: &image::DynamicImage,
    path: &Path,
    settings: PngEncodeSettings,
    cancel_token: Option<&CancellationToken>,
) -> Result<u64, OrthoImportError> {
    let is_cancelled = || cancel_token.map(|t| t.is_cancelled()).unwrap_or(false);
    let owned;
    let rgba: &image::RgbaImage = match image.as_rgba8() {
        Some(r) => r,
        None => {
            owned = image.to_rgba8();
            &owned
        }
    };
    let (w, h) = rgba.dimensions();
    let file = File::create(path).map_err(|e| {
        OrthoImportError::InternalError(format!("Gagal membuat berkas display.png: {}", e))
    })?;
    let writer = CancellableWriter::new(BufWriter::new(file), cancel_token);
    let mut encoder = png::Encoder::new(writer, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(settings.compression);
    encoder.set_filter(settings.filter);
    encoder.set_adaptive_filter(settings.adaptive);

    let map_err = |what: &str, e: String| -> OrthoImportError {
        if is_cancelled() {
            OrthoImportError::Dibatalkan
        } else {
            OrthoImportError::InternalError(format!("{}: {}", what, e))
        }
    };

    let mut png_writer = encoder
        .write_header()
        .map_err(|e| map_err("Gagal menulis header PNG", e.to_string()))?;
    {
        let mut stream = png_writer
            .stream_writer()
            .map_err(|e| map_err("Gagal membuka stream PNG", e.to_string()))?;
        let row_bytes = w as usize * 4;
        for band in rgba.as_raw().chunks(row_bytes * PNG_ENCODE_BAND_ROWS) {
            if is_cancelled() {
                return Err(OrthoImportError::Dibatalkan);
            }
            stream
                .write_all(band)
                .map_err(|e| map_err("Gagal menulis band PNG", e.to_string()))?;
        }
        stream
            .finish()
            .map_err(|e| map_err("Gagal menyelesaikan stream PNG", e.to_string()))?;
    }
    png_writer
        .finish()
        .map_err(|e| map_err("Gagal menyelesaikan PNG", e.to_string()))?;

    Ok(fs::metadata(path).map(|m| m.len()).unwrap_or(0))
}

/// Loop adaptif PNG ber-alpha: encode, lalu lompat kuadratik ke anak tangga berikutnya
/// sampai ukuran <= `limit_bytes` atau mencapai lantai 1.024 px. Selalu mengubah ukuran
/// dari kanvas asli di memori.
pub fn adaptive_encode_png(
    image: &image::DynamicImage,
    target_dir: &Path,
    cancel_token: Option<&CancellationToken>,
    limit_bytes: u64,
    settings: PngEncodeSettings,
) -> Result<AdaptiveEncodeResult, OrthoImportError> {
    let mut warnings = Vec::new();
    let display_png = target_dir.join("display.png");
    let (orig_w, orig_h) = (image.width(), image.height());
    let max_orig_dim = orig_w.max(orig_h);
    let limit_mib = limit_bytes as f64 / (1024 * 1024) as f64;
    let mut cur_dim = png_initial_dimension(max_orig_dim);

    loop {
        if let Some(t) = cancel_token {
            if t.is_cancelled() {
                return Err(OrthoImportError::Dibatalkan);
            }
        }

        let resized = if cur_dim >= max_orig_dim {
            image.clone()
        } else {
            resample_dynamic_image_box_filter(image, cur_dim)?
        };
        let (tw, th) = (resized.width(), resized.height());
        let size = encode_png_banded(&resized, &display_png, settings, cancel_token)?;

        if size <= limit_bytes || cur_dim <= DISPLAY_MIN_DIMENSION_PX {
            if tw < orig_w || th < orig_h {
                warnings.push(format!(
                    "Resolusi pratinjau PNG disesuaikan ke {}x{} agar muat dalam batas {} MiB",
                    tw, th, limit_mib
                ));
            }
            if tw.max(th) < 2048 {
                warnings.push(format!(
                    "Peringatan: Resolusi akhir pratinjau PNG ({}x{}) berada di bawah 2.048 piksel",
                    tw, th
                ));
            }
            return Ok(AdaptiveEncodeResult {
                display_path: display_png,
                display_size_bytes: size,
                final_dimensions: [tw, th],
                format: "png".to_string(),
                warnings,
            });
        }

        cur_dim = png_next_dimension(cur_dim, size, limit_bytes);
    }
}

/// Menghasilkan berkas display adaptif (display.jpg atau display.png) dengan kontrol ketat:
/// - JPEG: batas maksimal 5 MiB (`DISPLAY_MAX_BYTES_JPEG`).
/// - PNG (citra dengan transparansi alpha): batas maksimal 10 MiB (`DISPLAY_MAX_BYTES_PNG_ALPHA`).
///   Alasan: PNG lossless pada foto udara berentropi tinggi tidak dapat mencapai 2.048 px dalam 5 MiB,
///   sedangkan JPEG bisa.
/// - Tangga dimensi (sisi terpanjang): 4096, 3584, 3072, 2560, 2304, 2048, 1792, 1536, 1280, 1024.
/// - Estimasi kuadratik: lihat [`png_next_dimension`].
/// - Downscale selalu dilakukan langsung dari kanvas asli di memori (`image`).
pub fn adaptive_encode_display(
    image: &image::DynamicImage,
    target_dir: &Path,
    has_alpha: bool,
    cancel_token: Option<&CancellationToken>,
) -> Result<AdaptiveEncodeResult, OrthoImportError> {
    let mut warnings = Vec::new();

    if has_alpha {
        adaptive_encode_png(
            image,
            target_dir,
            cancel_token,
            display_limit_bytes(true),
            PngEncodeSettings::default(),
        )
    } else {
        // Enkode JPEG: Kualitas dulu, baru dimensi
        let display_jpg = target_dir.join("display.jpg");
        let rgb_img = image.to_rgb8();
        let (orig_w, orig_h) = (rgb_img.width(), rgb_img.height());

        let quality_candidates = [85u8, 80u8, 75u8, 70u8];
        let dimension_candidates = [
            DISPLAY_TARGET_DIMENSION_PX.min(orig_w.max(orig_h)),
            3072,
            2048,
            1536,
            1024,
        ];

        for &dim in &dimension_candidates {
            if let Some(t) = cancel_token {
                if t.is_cancelled() {
                    return Err(OrthoImportError::Dibatalkan);
                }
            }

            let resized = if dim >= orig_w.max(orig_h) {
                rgb_img.clone()
            } else {
                resample_rgb_image_box_filter(&rgb_img, dim)?
            };
            let (tw, th) = (resized.width(), resized.height());

            for &quality in &quality_candidates {
                if let Some(t) = cancel_token {
                    if t.is_cancelled() {
                        return Err(OrthoImportError::Dibatalkan);
                    }
                }

                let file = File::create(&display_jpg).map_err(|e| {
                    OrthoImportError::InternalError(format!(
                        "Gagal membuat berkas display.jpg: {}",
                        e
                    ))
                })?;
                let buf_writer = BufWriter::new(file);
                let mut writer = CancellableWriter::new(buf_writer, cancel_token);

                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
                encoder.encode_image(&resized).map_err(|e| {
                    if let Some(t) = cancel_token {
                        if t.is_cancelled() {
                            return OrthoImportError::Dibatalkan;
                        }
                    }
                    OrthoImportError::InternalError(format!("Gagal encode: {}", e))
                })?;
                writer.flush().map_err(|e| {
                    if let Some(t) = cancel_token {
                        if t.is_cancelled() {
                            return OrthoImportError::Dibatalkan;
                        }
                    }
                    OrthoImportError::InternalError(format!("Gagal flush: {}", e))
                })?;

                let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
                if size <= DISPLAY_MAX_BYTES_JPEG {
                    if tw < orig_w || th < orig_h {
                        warnings.push(format!(
                            "Resolusi display diadaptasi ke {}x{} (kualitas {})",
                            tw, th, quality
                        ));
                    }
                    if tw.max(th) < 2048 {
                        warnings.push(format!(
                            "Peringatan: Resolusi akhir pratinjau JPEG ({}x{}) berada di bawah 2.048 piksel",
                            tw, th
                        ));
                    }
                    return Ok(AdaptiveEncodeResult {
                        display_path: display_jpg,
                        display_size_bytes: size,
                        final_dimensions: [tw, th],
                        format: "jpg".to_string(),
                        warnings,
                    });
                }
            }
        }

        let size = fs::metadata(&display_jpg).map(|m| m.len()).unwrap_or(0);
        Ok(AdaptiveEncodeResult {
            display_path: display_jpg,
            display_size_bytes: size,
            final_dimensions: [orig_w, orig_h],
            format: "jpg".to_string(),
            warnings,
        })
    }
}
