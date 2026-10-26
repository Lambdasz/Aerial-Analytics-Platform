//! Modul streaming resampler murni dengan algoritma Box Filter (Area Averaging).
//!
//! Karakteristik:
//! - Streaming per baris: memori akumulator aktif hanya untuk baris output yang sedang bersinggungan (< 1 MB).
//! - Input: `u8` dan `u16`.
//! - Kanal: 1 (Grayscale), 3 (RGB), 4 (RGBA).
//! - Mencegah upscaling: jika dimensi sumber <= batas target, ukuran dipertahankan 1:1.
//! - Pengecekan `CancellationToken` per baris untuk pembatalan seketika.
//! - Konservasi energi: pembobotan presisi kontinu (termasuk faktor skala pecahan dan piksel tepi).

use std::collections::VecDeque;

use super::job_manager::CancellationToken;
use crate::modules::module_03::error::OrthoImportError;

/// Konfigurasi kanal format citra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelCount {
    Gray = 1,
    Rgb = 3,
    Rgba = 4,
}

impl ChannelCount {
    pub fn channels(self) -> usize {
        self as usize
    }

    pub fn from_usize(n: usize) -> Result<Self, OrthoImportError> {
        match n {
            1 => Ok(ChannelCount::Gray),
            3 => Ok(ChannelCount::Rgb),
            4 => Ok(ChannelCount::Rgba),
            other => Err(OrthoImportError::FormatTidakDidukung(format!(
                "Jumlah kanal {} tidak didukung oleh streaming resampler (hanya 1, 3, atau 4).",
                other
            ))),
        }
    }
}

/// Kontribusi horizontal sebuah piksel input terhadap piksel output tertentu.
#[derive(Debug, Clone, Copy)]
pub struct HorizContribution {
    pub in_x: usize,
    pub weight: f64,
}

/// Pemetaan horizontal untuk satu kolom output.
#[derive(Debug, Clone)]
pub struct ColMapping {
    pub contributions: Vec<HorizContribution>,
}

/// Akumulator untuk satu baris output yang sedang aktif dikumpulkan.
#[derive(Debug, Clone)]
struct ActiveRowAccumulator {
    y_out: u32,
    /// Akumulator nilai kanal: panjang = `dst_width * channels`
    channel_sums: Vec<f64>,
    /// Akumulator bobot per kolom output: panjang = `dst_width`
    weight_sums: Vec<f64>,
}

impl ActiveRowAccumulator {
    fn new(y_out: u32, dst_width: usize, channels: usize) -> Self {
        Self {
            y_out,
            channel_sums: vec![0.0; dst_width * channels],
            weight_sums: vec![0.0; dst_width],
        }
    }
}

/// Hasil akhir citra yang di-resample.
#[derive(Debug, Clone)]
pub struct ResampledImage {
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub data: Vec<u8>,
}

/// Streaming resampler berbasis Area Averaging (Box Filter).
pub struct StreamingResampler {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    channel_type: ChannelCount,
    scale_y: f64,
    horiz_mappings: Vec<ColMapping>,
    active_rows: VecDeque<ActiveRowAccumulator>,
    final_output: Vec<u8>,
    next_expected_y_in: u32,
    u16_range_min: f64,
    u16_range_max: f64,
}

impl StreamingResampler {
    /// Membuat resampler baru.
    ///
    /// Jika `src_width <= max_target_dim && src_height <= max_target_dim`,
    /// resampler TIDAK melakukan upscale dan menetapkan dimensi tujuan sama persis dengan sumber.
    pub fn new(
        src_width: u32,
        src_height: u32,
        max_target_dim: u32,
        channel_type: ChannelCount,
    ) -> Result<Self, OrthoImportError> {
        if src_width == 0 || src_height == 0 {
            return Err(OrthoImportError::FileKorup(
                "Dimensi citra tidak boleh nol".to_string(),
            ));
        }

        let longest = src_width.max(src_height);
        let (dst_width, dst_height) = if longest > max_target_dim && max_target_dim > 0 {
            let scale = max_target_dim as f64 / longest as f64;
            let dw = ((src_width as f64 * scale).round() as u32).max(1);
            let dh = ((src_height as f64 * scale).round() as u32).max(1);
            (dw, dh)
        } else {
            // Tanpa upscale: dimensi tetap sama dengan sumber
            (src_width, src_height)
        };

        let scale_x = src_width as f64 / dst_width as f64;
        let scale_y = src_height as f64 / dst_height as f64;

        // Precompute horizontal box-filter contributions
        let mut horiz_mappings = Vec::with_capacity(dst_width as usize);
        for x_out in 0..dst_width {
            let left_in = x_out as f64 * scale_x;
            let right_in = (x_out + 1) as f64 * scale_x;

            let in_x_start = left_in.floor() as usize;
            let in_x_end = (right_in.ceil() as usize).min(src_width as usize);

            let mut contributions = Vec::with_capacity(in_x_end.saturating_sub(in_x_start));
            for in_x in in_x_start..in_x_end {
                let pixel_left = in_x as f64;
                let pixel_right = (in_x + 1) as f64;

                let overlap_left = pixel_left.max(left_in);
                let overlap_right = pixel_right.min(right_in);

                let weight = (overlap_right - overlap_left).max(0.0);
                if weight > 0.0 {
                    contributions.push(HorizContribution { in_x, weight });
                }
            }
            horiz_mappings.push(ColMapping { contributions });
        }

        let total_output_bytes = dst_width as usize * dst_height as usize * channel_type.channels();
        let final_output = vec![0u8; total_output_bytes];

        Ok(Self {
            src_width,
            src_height,
            dst_width,
            dst_height,
            channel_type,
            scale_y,
            horiz_mappings,
            active_rows: VecDeque::new(),
            final_output,
            next_expected_y_in: 0,
            u16_range_min: 0.0,
            u16_range_max: 65535.0,
        })
    }

    /// Mengatur rentang normalisasi untuk input u16 (misal untuk percentile clipping di Opsi B).
    pub fn set_u16_range(&mut self, min_val: f64, max_val: f64) {
        if max_val > min_val {
            self.u16_range_min = min_val;
            self.u16_range_max = max_val;
        }
    }

    pub fn dst_width(&self) -> u32 {
        self.dst_width
    }

    pub fn dst_height(&self) -> u32 {
        self.dst_height
    }

    pub fn channels(&self) -> usize {
        self.channel_type.channels()
    }

    /// Memasukkan satu baris sampel `u8` (panjang data harus `src_width * channels`).
    pub fn push_row_u8(
        &mut self,
        y_in: u32,
        row_data: &[u8],
        cancel_token: Option<&CancellationToken>,
    ) -> Result<(), OrthoImportError> {
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(OrthoImportError::Dibatalkan);
            }
        }

        let channels = self.channel_type.channels();
        let expected_len = self.src_width as usize * channels;
        if row_data.len() < expected_len {
            return Err(OrthoImportError::FileKorup(format!(
                "Baris {} memiliki panjang data tidak lengkap ({} < {})",
                y_in,
                row_data.len(),
                expected_len
            )));
        }

        self.process_row_generic(y_in, row_data, |val| *val as f64)
    }

    /// Memasukkan satu baris sampel `u16` (panjang data harus `src_width * channels`).
    /// Menerima parameter `(lo, hi)` hasil analisis persentil deterministik p0,5 - p99,5.
    /// Melewati stretch jika data sudah full-range (lo <= 50 && hi >= 65400).
    pub fn push_row_u16(
        &mut self,
        y_in: u32,
        row_data: &[u16],
        lo: f64,
        hi: f64,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<(), OrthoImportError> {
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(OrthoImportError::Dibatalkan);
            }
        }

        let channels = self.channel_type.channels();
        let expected_len = self.src_width as usize * channels;
        if row_data.len() < expected_len {
            return Err(OrthoImportError::FileKorup(format!(
                "Baris {} memiliki panjang data u16 tidak lengkap ({} < {})",
                y_in,
                row_data.len(),
                expected_len
            )));
        }

        let is_full_range = lo <= 50.0 && hi >= 65400.0;
        let diff = (hi - lo).max(1.0);

        self.process_row_generic(y_in, row_data, move |val| {
            if is_full_range {
                // Lewati peregangan jika data sudah full-range
                (*val as f64) * 255.0 / 65535.0
            } else {
                let v = *val as f64;
                let clamped = v.clamp(lo, hi);
                ((clamped - lo) * 255.0) / diff
            }
        })
    }

    fn process_row_generic<T, F>(
        &mut self,
        y_in: u32,
        row_data: &[T],
        to_f64: F,
    ) -> Result<(), OrthoImportError>
    where
        F: Fn(&T) -> f64,
    {
        let channels = self.channel_type.channels();
        let dst_w = self.dst_width as usize;

        let row_top = y_in as f64;
        let row_bottom = (y_in + 1) as f64;

        // Rentang baris output y_out yang bersinggungan dengan row Y input ini
        let y_out_start = (row_top / self.scale_y).floor() as u32;
        let y_out_end = ((row_bottom / self.scale_y).ceil() as u32).min(self.dst_height);

        // Pastikan accumulator aktif untuk rentang [y_out_start..y_out_end] sudah ada
        for y_out in y_out_start..y_out_end {
            if !self.active_rows.iter().any(|r| r.y_out == y_out) {
                self.active_rows
                    .push_back(ActiveRowAccumulator::new(y_out, dst_w, channels));
            }
        }

        // Akumulasikan kontribusi untuk setiap baris output aktif
        for y_out in y_out_start..y_out_end {
            let out_top = y_out as f64 * self.scale_y;
            let out_bottom = (y_out + 1) as f64 * self.scale_y;

            let overlap_top = row_top.max(out_top);
            let overlap_bottom = row_bottom.min(out_bottom);
            let weight_y = (overlap_bottom - overlap_top).max(0.0);

            if weight_y <= 0.0 {
                continue;
            }

            // Temukan accumulator untuk y_out
            if let Some(acc) = self.active_rows.iter_mut().find(|r| r.y_out == y_out) {
                for (x_out, mapping) in self.horiz_mappings.iter().enumerate() {
                    let mut col_weight_sum = 0.0;
                    for contrib in &mapping.contributions {
                        let w = contrib.weight * weight_y;
                        col_weight_sum += w;
                        let src_idx = contrib.in_x * channels;
                        let dst_idx = x_out * channels;

                        for c in 0..channels {
                            let sample_val = to_f64(&row_data[src_idx + c]);
                            acc.channel_sums[dst_idx + c] += sample_val * w;
                        }
                    }
                    acc.weight_sums[x_out] += col_weight_sum;
                }
            }
        }

        self.next_expected_y_in = y_in + 1;

        // Finalisasi dan keluarkan baris yang sudah lengkap (sudah melewati row_bottom >= out_bottom)
        while let Some(front) = self.active_rows.front() {
            let out_bottom = (front.y_out + 1) as f64 * self.scale_y;
            // Baris output selesai jika input row berikutnya sudah berada di luar batas bawah out_bottom
            // atau jika ini adalah baris input terakhir
            let is_completed =
                row_bottom >= out_bottom || self.next_expected_y_in >= self.src_height;

            if is_completed {
                let acc = self.active_rows.pop_front().unwrap();
                self.write_finished_row(&acc);
            } else {
                break;
            }
        }

        Ok(())
    }

    fn write_finished_row(&mut self, acc: &ActiveRowAccumulator) {
        let channels = self.channel_type.channels();
        let dst_w = self.dst_width as usize;
        let row_offset = acc.y_out as usize * dst_w * channels;

        for x_out in 0..dst_w {
            let weight = acc.weight_sums[x_out];
            let dst_px_idx = row_offset + x_out * channels;
            let src_px_idx = x_out * channels;

            if weight > 0.0 {
                if channels == 4 {
                    let alpha_sum = acc.channel_sums[src_px_idx + 3];
                    let alpha_weight = alpha_sum / 255.0;

                    if alpha_weight > 0.001 {
                        for c in 0..3 {
                            let avg = acc.channel_sums[src_px_idx + c] / alpha_weight;
                            self.final_output[dst_px_idx + c] = avg.round().clamp(0.0, 255.0) as u8;
                        }
                        let avg_a = alpha_sum / weight;
                        self.final_output[dst_px_idx + 3] = avg_a.round().clamp(0.0, 255.0) as u8;
                    } else {
                        self.final_output[dst_px_idx] = 0;
                        self.final_output[dst_px_idx + 1] = 0;
                        self.final_output[dst_px_idx + 2] = 0;
                        self.final_output[dst_px_idx + 3] = 0;
                    }
                } else {
                    for c in 0..channels {
                        let avg = acc.channel_sums[src_px_idx + c] / weight;
                        self.final_output[dst_px_idx + c] = avg.round().clamp(0.0, 255.0) as u8;
                    }
                }
            } else {
                // Fallback jika tidak ada bobot (edge case pembulatan numerik)
                for c in 0..channels {
                    self.final_output[dst_px_idx + c] = 0;
                }
            }
        }
    }

    /// Menyelesaikan proses resampling dan mengembalikan buffer citra hasil.
    pub fn finish(mut self) -> Result<ResampledImage, OrthoImportError> {
        // Keluarkan baris akumulator yang tersisa (jika ada sisa di akhir)
        while let Some(acc) = self.active_rows.pop_front() {
            self.write_finished_row(&acc);
        }

        Ok(ResampledImage {
            width: self.dst_width,
            height: self.dst_height,
            channels: self.channel_type.channels(),
            data: self.final_output,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests untuk StreamingResampler
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementasi referensi naif (full frame area averaging) untuk memverifikasi fidelitas.
    fn naive_box_filter(
        src: &[u8],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
        channels: usize,
    ) -> Vec<u8> {
        let scale_x = src_w as f64 / dst_w as f64;
        let scale_y = src_h as f64 / dst_h as f64;
        let mut out = vec![0u8; dst_w as usize * dst_h as usize * channels];

        for y_out in 0..dst_h {
            let top_in = y_out as f64 * scale_y;
            let bot_in = (y_out + 1) as f64 * scale_y;

            for x_out in 0..dst_w {
                let left_in = x_out as f64 * scale_x;
                let right_in = (x_out + 1) as f64 * scale_x;

                let mut sums = vec![0.0f64; channels];
                let mut total_w = 0.0f64;

                let y_start = top_in.floor() as usize;
                let y_end = (bot_in.ceil() as usize).min(src_h as usize);
                let x_start = left_in.floor() as usize;
                let x_end = (right_in.ceil() as usize).min(src_w as usize);

                for in_y in y_start..y_end {
                    let wy = (in_y as f64 + 1.0).min(bot_in) - (in_y as f64).max(top_in);
                    if wy <= 0.0 {
                        continue;
                    }
                    for in_x in x_start..x_end {
                        let wx = (in_x as f64 + 1.0).min(right_in) - (in_x as f64).max(left_in);
                        if wx <= 0.0 {
                            continue;
                        }
                        let w = wx * wy;
                        total_w += w;
                        let src_idx = (in_y * src_w as usize + in_x) * channels;
                        for c in 0..channels {
                            sums[c] += src[src_idx + c] as f64 * w;
                        }
                    }
                }

                let out_idx = (y_out as usize * dst_w as usize + x_out as usize) * channels;
                for c in 0..channels {
                    let v = if total_w > 0.0 {
                        (sums[c] / total_w).round().clamp(0.0, 255.0) as u8
                    } else {
                        0
                    };
                    out[out_idx + c] = v;
                }
            }
        }

        out
    }

    #[test]
    fn test_fidelity_against_naive_reference_synthetic_gradient_and_edges() {
        // Ukuran non-integer scale factor: 103 x 77 -> 31 x 23 (RGB 3 channel)
        let src_w = 103u32;
        let src_h = 77u32;
        let dst_w = 31u32;
        let channels = 3usize;

        let mut src_data = vec![0u8; src_w as usize * src_h as usize * channels];
        for y in 0..src_h {
            for x in 0..src_w {
                let idx = (y as usize * src_w as usize + x as usize) * channels;
                // Buat pola gradien + tepi tajam (checkerboard di pinggir)
                src_data[idx] = ((x * 255) / src_w) as u8;
                src_data[idx + 1] = ((y * 255) / src_h) as u8;
                src_data[idx + 2] = if (x + y) % 2 == 0 { 255 } else { 0 };
            }
        }

        let mut resampler = StreamingResampler::new(
            src_w,
            src_h,
            dst_w, // dst_w adalah sisi terpanjang
            ChannelCount::Rgb,
        )
        .unwrap();

        for y in 0..src_h {
            let row_start = y as usize * src_w as usize * channels;
            let row_end = row_start + src_w as usize * channels;
            resampler
                .push_row_u8(y, &src_data[row_start..row_end], None)
                .unwrap();
        }

        let resampled = resampler.finish().unwrap();
        let naive = naive_box_filter(
            &src_data,
            src_w,
            src_h,
            resampled.width,
            resampled.height,
            channels,
        );

        assert_eq!(resampled.data.len(), naive.len());

        // Verifikasi deviasi maksimal <= 1 LSB akibat pembulatan floating point
        let mut max_diff = 0i32;
        for (&act, &exp) in resampled.data.iter().zip(naive.iter()) {
            let diff = (act as i32 - exp as i32).abs();
            if diff > max_diff {
                max_diff = diff;
            }
        }
        assert!(
            max_diff <= 1,
            "Deviasi streaming resampler terhadap naive reference melebihi 1 LSB! max_diff = {}",
            max_diff
        );
    }

    #[test]
    fn test_no_upscale_when_source_smaller_than_target() {
        let src_w = 200u32;
        let src_h = 150u32;
        let max_target = 4096u32; // Jauh lebih besar dari sumber

        let resampler =
            StreamingResampler::new(src_w, src_h, max_target, ChannelCount::Rgba).unwrap();
        // Dimensi tujuan harus tetap 200 x 150 (tanpa upscale!)
        assert_eq!(resampler.dst_width(), 200);
        assert_eq!(resampler.dst_height(), 150);
    }

    #[test]
    fn test_cancellation_per_row() {
        let cancel_token = CancellationToken::new();
        let mut resampler = StreamingResampler::new(100, 100, 50, ChannelCount::Rgb).unwrap();

        // Push baris 0 sukses
        let dummy_row = vec![128u8; 100 * 3];
        resampler
            .push_row_u8(0, &dummy_row, Some(&cancel_token))
            .unwrap();

        // Batalkan
        cancel_token.cancel();

        // Push baris 1 harus langsung gagal dengan OrthoImportError::Dibatalkan
        let res = resampler.push_row_u8(1, &dummy_row, Some(&cancel_token));
        match res {
            Err(OrthoImportError::Dibatalkan) => {}
            other => panic!("Harusnya Dibatalkan, didapat: {:?}", other),
        }
    }

    #[test]
    fn test_u16_input_and_percentile_range_mapping() {
        let src_w = 10u32;
        let src_h = 10u32;
        let mut resampler = StreamingResampler::new(src_w, src_h, 5, ChannelCount::Gray).unwrap();
        resampler.set_u16_range(1000.0, 5000.0);

        // Baris berisi 1000 (min), 3000 (mid), 5000 (max)
        let row_data = vec![
            1000u16, 1000, 3000, 3000, 5000, 5000, 1000, 3000, 5000, 5000,
        ];
        for y in 0..src_h {
            resampler
                .push_row_u16(y, &row_data, 1000.0, 5000.0, None)
                .unwrap();
        }

        let res = resampler.finish().unwrap();
        assert_eq!(res.width, 5);
        assert_eq!(res.height, 5);
        // Piksel pertama adalah rata-rata dari 1000 -> harus 0
        assert_eq!(res.data[0], 0);
        // Piksel kedua adalah rata-rata dari 3000 -> harus ~128
        assert!((res.data[1] as i32 - 128).abs() <= 2);
        // Piksel ketiga adalah rata-rata dari 5000 -> harus 255
        assert_eq!(res.data[2], 255);
    }

    #[test]
    fn test_rgba_straight_alpha_resampling_no_dark_fringe() {
        let src_w = 4u32;
        let src_h = 4u32;
        let mut resampler = StreamingResampler::new(src_w, src_h, 2, ChannelCount::Rgba).unwrap();

        // 4 baris:
        // Baris 0, 1: 2 piksel transparan (0,0,0,0), lalu 2 piksel opaque (100, 150, 200, 255)
        // Baris 2, 3: seluruh 4 piksel transparan (0,0,0,0)
        let top_row = vec![
            0u8, 0, 0, 0, // Px 0: transparan
            0, 0, 0, 0, // Px 1: transparan
            100, 150, 200, 255, // Px 2: opaque
            100, 150, 200, 255, // Px 3: opaque
        ];
        let transparent_row = vec![0u8; 16];

        resampler.push_row_u8(0, &top_row, None).unwrap();
        resampler.push_row_u8(1, &top_row, None).unwrap();
        resampler.push_row_u8(2, &transparent_row, None).unwrap();
        resampler.push_row_u8(3, &transparent_row, None).unwrap();

        let res = resampler.finish().unwrap();
        assert_eq!(res.width, 2);
        assert_eq!(res.height, 2);

        // Px (0,0) [Top-Left] di output 2x2 menggabungkan 4 piksel transparan (0,0,0,0) -> harus (0,0,0,0)
        assert_eq!(&res.data[0..4], &[0, 0, 0, 0]);

        // Px (1,0) [Top-Right] di output 2x2 menggabungkan 4 piksel opaque (100, 150, 200, 255) -> harus (100, 150, 200, 255)
        assert_eq!(&res.data[4..8], &[100, 150, 200, 255]);

        // Px (0,1) & Px (1,1) [Bottom row] di output 2x2 menggabungkan baris transparan -> harus (0,0,0,0)
        assert_eq!(&res.data[8..12], &[0, 0, 0, 0]);
        assert_eq!(&res.data[12..16], &[0, 0, 0, 0]);
    }
}
