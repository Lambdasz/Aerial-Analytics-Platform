//! Benchmark Memori Puncak Fase 2 (Opsi A) - Pengukuran Nyata Berbasis PeakWorkingSetSize
//!
//! Mengukur konsumsi memori puncak sebenarnya (peak working set high-water mark) via Windows API
//! `K32GetProcessMemoryInfo` (PeakWorkingSetSize) dan `K32EmptyWorkingSet`.
//! Menjalankan proses decoding di proses anak terisolasi agar memori proses tidak tercemar
//! oleh alokasi pembuatan berkas sintetis.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::{
    decode_and_scale_jpeg_idct, decode_and_scale_png,
};

#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessMemSnapshot {
    pub working_set_bytes: u64,
    pub peak_working_set_bytes: u64,
}

#[cfg(windows)]
pub fn get_memory_snapshot() -> ProcessMemSnapshot {
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            ppsmps: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }
    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb);
        ProcessMemSnapshot {
            working_set_bytes: counters.working_set_size as u64,
            peak_working_set_bytes: counters.peak_working_set_size as u64,
        }
    }
}

#[cfg(windows)]
pub fn trim_working_set() {
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32EmptyWorkingSet(process: isize) -> i32;
    }
    unsafe {
        K32EmptyWorkingSet(GetCurrentProcess());
    }
}

fn get_cache_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("aap_memory_bench_cache");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Helper untuk membuat JPEG Baseline 100 MP (10.000 x 10.000) dengan gradien dan noise (bukan warna rata).
fn ensure_baseline_jpeg_100mp(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 1_000_000 {
        return;
    }
    println!("Membuat berkas JPEG Baseline 100 MP (10.000 x 10.000 px)...");
    let (width, height) = (10_000u32, 10_000u32);
    let file = File::create(path).expect("create baseline jpg");
    let mut writer = BufWriter::new(file);

    let mut enc = jpeg_encoder::Encoder::new(&mut writer, 75);
    enc.set_progressive(false);

    let mut row_buf = vec![0u8; (width * 3) as usize];
    let mut full_buf = Vec::with_capacity((width * height * 3) as usize);

    for y in 0..height {
        let grad_y = ((y * 255) / height) as u8;
        for x in 0..width {
            let grad_x = ((x * 255) / width) as u8;
            let noise = ((x ^ y) & 0x1F) as u8;
            let idx = (x * 3) as usize;
            row_buf[idx] = grad_x.wrapping_add(noise);
            row_buf[idx + 1] = grad_y.wrapping_sub(noise);
            row_buf[idx + 2] = (grad_x ^ grad_y).wrapping_add(noise);
        }
        full_buf.extend_from_slice(&row_buf);
    }

    enc.encode(
        &full_buf,
        width as u16,
        height as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .expect("encode baseline 100MP");
}

/// Helper untuk membuat JPEG Baseline 144 MP (12.000 x 12.000) batas desain dengan gradien dan noise.
fn ensure_baseline_jpeg_144mp(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 1_000_000 {
        return;
    }
    println!("Membuat berkas JPEG Baseline 144 MP (12.000 x 12.000 px)...");
    let (width, height) = (12_000u32, 12_000u32);
    let file = File::create(path).expect("create baseline 144mp jpg");
    let mut writer = BufWriter::new(file);

    let mut enc = jpeg_encoder::Encoder::new(&mut writer, 75);
    enc.set_progressive(false);

    let mut row_buf = vec![0u8; (width * 3) as usize];
    let mut full_buf = Vec::with_capacity((width * height * 3) as usize);

    for y in 0..height {
        let grad_y = ((y * 255) / height) as u8;
        for x in 0..width {
            let grad_x = ((x * 255) / width) as u8;
            let noise = ((x ^ y) & 0x1F) as u8;
            let idx = (x * 3) as usize;
            row_buf[idx] = grad_x.wrapping_add(noise);
            row_buf[idx + 1] = grad_y.wrapping_sub(noise);
            row_buf[idx + 2] = (grad_x ^ grad_y).wrapping_add(noise);
        }
        full_buf.extend_from_slice(&row_buf);
    }

    enc.encode(
        &full_buf,
        width as u16,
        height as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .expect("encode baseline 144MP");
}

/// Helper untuk membuat JPEG Progressive 40 MP nyata menggunakan encoder progressive.
fn ensure_progressive_jpeg_40mp(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 1_000_000 {
        return;
    }
    println!("Membuat berkas JPEG Progressive 40 MP (8.000 x 5.000 px)...");
    let (width, height) = (8_000u32, 5_000u32);
    let file = File::create(path).expect("create prog jpg");
    let mut writer = BufWriter::new(file);

    let mut enc = jpeg_encoder::Encoder::new(&mut writer, 75);
    enc.set_progressive(true); // Progressive Mode Nyata (SOF2)

    let mut full_buf = Vec::with_capacity((width * height * 3) as usize);
    let mut row_buf = vec![0u8; (width * 3) as usize];

    for y in 0..height {
        let grad_y = ((y * 255) / height) as u8;
        for x in 0..width {
            let grad_x = ((x * 255) / width) as u8;
            let noise = ((x ^ y) & 0x1F) as u8;
            let idx = (x * 3) as usize;
            row_buf[idx] = grad_x.wrapping_add(noise);
            row_buf[idx + 1] = grad_y.wrapping_sub(noise);
            row_buf[idx + 2] = (grad_x ^ grad_y).wrapping_add(noise);
        }
        full_buf.extend_from_slice(&row_buf);
    }

    enc.encode(
        &full_buf,
        width as u16,
        height as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .expect("encode progressive 40MP");
}

/// Helper untuk membuat PNG 100 MP RGBA non-interlaced dengan gradien + noise.
fn ensure_png_100mp(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 1_000_000 {
        return;
    }
    println!("Membuat berkas PNG 100 MP RGBA non-interlaced (10.000 x 10.000 px)...");
    let (width, height) = (10_000u32, 10_000u32);
    let file = File::create(path).expect("create png");
    let buf_writer = BufWriter::new(file);

    let mut enc = png::Encoder::new(buf_writer, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);

    let mut writer = enc.write_header().expect("write header");

    let mut full_buf = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let grad_y = ((y * 255) / height) as u8;
        let row_offset = (y * width * 4) as usize;
        for x in 0..width {
            let grad_x = ((x * 255) / width) as u8;
            let noise = ((x ^ y) & 0x1F) as u8;
            let idx = row_offset + (x * 4) as usize;
            full_buf[idx] = grad_x.wrapping_add(noise);
            full_buf[idx + 1] = grad_y.wrapping_sub(noise);
            full_buf[idx + 2] = (grad_x ^ grad_y).wrapping_add(noise);
            full_buf[idx + 3] = 255;
        }
    }
    writer
        .write_image_data(&full_buf)
        .expect("write full png data");
}

fn write_png_chunk<W: Write>(w: &mut W, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    w.write_all(&len.to_be_bytes()).unwrap();
    w.write_all(chunk_type).unwrap();
    w.write_all(data).unwrap();

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    let crc = hasher.finalize();
    w.write_all(&crc.to_be_bytes()).unwrap();
}

/// Helper untuk membuat PNG 25 MP RGBA Interlaced (Adam7) pada batas aman baru.
fn ensure_png_25mp_interlaced(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 100_000 {
        return;
    }
    println!("Membuat berkas PNG Interlaced (Adam7) 25 MP (5.000 x 5.000 px)...");
    let (width, height) = (5_000u32, 5_000u32);
    let file = File::create(path).expect("create interlaced png");
    let mut writer = BufWriter::new(file);

    // PNG Signature
    writer
        .write_all(&[137, 80, 78, 71, 13, 10, 26, 10])
        .unwrap();

    // IHDR Chunk: 13 bytes
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // 8-bit depth
    ihdr_data.push(6); // RGBA
    ihdr_data.push(0); // compression deflate
    ihdr_data.push(0); // filter
    ihdr_data.push(1); // Adam7 INTERLACED!

    write_png_chunk(&mut writer, b"IHDR", &ihdr_data);

    // Scanline 7 pass Adam7
    let passes = [
        (0u32, 0u32, 8u32, 8u32), // pass 1
        (4, 0, 8, 8),             // pass 2
        (0, 4, 4, 8),             // pass 3
        (2, 0, 4, 4),             // pass 4
        (0, 2, 2, 4),             // pass 5
        (1, 0, 2, 2),             // pass 6
        (0, 1, 1, 2),             // pass 7
    ];

    let mut raw_scanlines = Vec::new();
    for &(x_start, y_start, x_step, y_step) in &passes {
        if x_start >= width || y_start >= height {
            continue;
        }
        let mut y = y_start;
        while y < height {
            raw_scanlines.push(0u8); // Filter 0 (None)
            let grad_y = ((y * 255) / height) as u8;
            let mut x = x_start;
            while x < width {
                let grad_x = ((x * 255) / width) as u8;
                let noise = ((x ^ y) & 0x1F) as u8;
                raw_scanlines.push(grad_x.wrapping_add(noise));
                raw_scanlines.push(grad_y.wrapping_sub(noise));
                raw_scanlines.push((grad_x ^ grad_y).wrapping_add(noise));
                raw_scanlines.push(255);
                x += x_step;
            }
            y += y_step;
        }
    }

    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw_scanlines, 1);
    write_png_chunk(&mut writer, b"IDAT", &compressed);

    // IEND Chunk
    write_png_chunk(&mut writer, b"IEND", &[]);
}

// ─────────────────────────────────────────────────────────────────────────────
// PROSES ANAK TERISOLASI UNTUK PENGUKURAN PEAK WORKING SET MURNI
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn execute_isolated_memory_benchmarks() {
    // Jika dipanggil dalam mode worker terisolasi (via variabel lingkungan):
    if let Ok(bench_mode) = std::env::var("AAP_RUN_ISOLATED_BENCH") {
        let file_path = PathBuf::from(std::env::var("AAP_BENCH_FILE").expect("AAP_BENCH_FILE"));

        #[cfg(windows)]
        trim_working_set();

        #[cfg(windows)]
        let snap_before = get_memory_snapshot();

        match bench_mode.as_str() {
            "baseline_100mp" => {
                let decoded = decode_and_scale_jpeg_idct(&file_path, 4096).expect("decode idct");
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!("\n=== [PENGUKURAN TERISOLASI] JPEG BASELINE 100 MP ===");
                    println!("- Skala IDCT yang dipilih: 1/2 (5.000 x 5.000) -> 4.096 x 4.096 px via StreamingResampler");
                    println!(
                        "- Dimensi hasil akhir: {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Rincian Alokasi Teoritis:");
                    println!("  * Buffer Decode IDCT (5000x5000x3): 75.00 MB");
                    println!("  * Buffer Output Display (4096x4096x3): 50.33 MB");
                    println!("  * Resampler Active Band Accumulator: < 0.1 MB");
                    println!(
                        "  * Sisa (Runtime Tauri, Heap Overhead, Page Cache): {:.2} MB",
                        (delta_mb - 125.33).max(0.0)
                    );
                    println!("- Target Desain: <= 400 MB");
                    println!("====================================================\n");
                    assert!(
                        peak_mb <= 400.0,
                        "Puncak memori JPEG 100 MP ({:.2} MB) melampaui batas 400 MB",
                        peak_mb
                    );
                }
            }
            "baseline_144mp" => {
                let decoded =
                    decode_and_scale_jpeg_idct(&file_path, 4096).expect("decode idct 144mp");
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!(
                        "\n=== [PENGUKURAN TERISOLASI] JPEG BASELINE 144 MP (BATAS DESAIN) ==="
                    );
                    println!("- Skala IDCT yang dipilih: 1/2 (6.000 x 6.000) -> 4.096 x 4.096 px via StreamingResampler");
                    println!(
                        "- Dimensi hasil akhir: {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Rincian Alokasi Teoritis:");
                    println!("  * Buffer Decode IDCT (6000x6000x3): 108.00 MB");
                    println!("  * Buffer Output Display (4096x4096x3): 50.33 MB");
                    println!("  * Resampler Active Band Accumulator: < 0.1 MB");
                    println!(
                        "  * Sisa (Runtime Tauri, Heap Overhead, Page Cache): {:.2} MB",
                        (delta_mb - 158.33).max(0.0)
                    );
                    println!("- Target Desain: <= 400 MB");
                    println!(
                        "===================================================================\n"
                    );
                    assert!(
                        peak_mb <= 400.0,
                        "Puncak memori JPEG 144 MP ({:.2} MB) melampaui batas 400 MB",
                        peak_mb
                    );
                }
            }
            "progressive_40mp" => {
                let decoded = decode_and_scale_jpeg_idct(&file_path, 4096).expect("decode prog");
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!("\n=== [PENGUKURAN TERISOLASI] JPEG PROGRESSIVE 40 MP ===");
                    println!(
                        "- Dimensi sumber: 8.000 x 5.000 px -> {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Rincian Alokasi Teoritis:");
                    println!("  * Koefisien Spektral DCT (8000x5000x2x3): ~80-120 MB");
                    println!("  * Frame RGB Decoded (8000x5000x3): 120.00 MB");
                    println!("  * Buffer Output Display (4096x2560x3): 31.46 MB");
                    println!("  * Resampler Active Band Accumulator: < 0.1 MB");
                    println!(
                        "  * Sisa (Runtime Tauri, Heap Overhead, Page Cache): {:.2} MB",
                        (delta_mb - 271.46).max(0.0)
                    );
                    println!("- Target Desain: <= 400 MB");
                    println!("======================================================\n");
                    assert!(
                        peak_mb <= 400.0,
                        "Puncak memori JPEG Progressive 40 MP ({:.2} MB) melampaui batas 400 MB",
                        peak_mb
                    );
                }
            }
            "png_100mp" => {
                let (decoded, has_alpha) =
                    decode_and_scale_png(&file_path, 4096).expect("decode png 100mp");
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!("\n=== [PENGUKURAN TERISOLASI] PNG 100 MP RGBA NON-INTERLACED ===");
                    println!("- Metode: Row-by-Row Streaming via reader.next_row() ke StreamingResampler");
                    println!("- Memiliki alpha: {}", has_alpha);
                    println!(
                        "- Dimensi hasil akhir: {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Rincian Alokasi Teoritis:");
                    println!("  * Scanline Decode Buffer (10.000 x 4 byte): 0.04 MB");
                    println!("  * Buffer Output Display (4096x4096x4): 67.11 MB");
                    println!("  * Resampler Active Band Accumulator: < 0.1 MB");
                    println!(
                        "  * Sisa (Zlib stream, runtime, page cache): {:.2} MB",
                        (delta_mb - 67.15).max(0.0)
                    );
                    println!("- Target Desain: <= 400 MB");
                    println!("==============================================================\n");
                    assert!(
                        peak_mb <= 400.0,
                        "Puncak memori PNG 100 MP ({:.2} MB) melampaui batas 400 MB",
                        peak_mb
                    );
                }
            }
            "png_25mp_interlaced" => {
                let (decoded, has_alpha) =
                    decode_and_scale_png(&file_path, 4096).expect("decode png 25mp interlaced");
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!("\n=== [PENGUKURAN TERISOLASI] PNG 25 MP RGBA INTERLACED (ADAM7) ===");
                    println!("- Metode: Full frame decode (syarat Adam7) disuapkan per baris ke StreamingResampler");
                    println!("- Memiliki alpha: {}", has_alpha);
                    println!(
                        "- Dimensi hasil akhir: {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Rincian Alokasi Teoritis:");
                    println!("  * Frame Buffer Decode Adam7 (5000x5000x4): 100.00 MB");
                    println!("  * Buffer Output Display (4096x4096x4): 67.11 MB");
                    println!("  * Resampler Active Band Accumulator: < 0.1 MB");
                    println!(
                        "  * Sisa (Zlib stream, runtime, page cache): {:.2} MB",
                        (delta_mb - 167.11).max(0.0)
                    );
                    println!("- Target Desain: <= 400 MB");
                    println!("=================================================================\n");
                    assert!(
                        peak_mb <= 400.0,
                        "Puncak memori PNG 25 MP Interlaced ({:.2} MB) melampaui batas 400 MB",
                        peak_mb
                    );
                }
            }
            _ => panic!("Mode benchmark tidak dikenal: {}", bench_mode),
        }
        return;
    }

    // MODE ORCHESTRATOR:
    let cache_dir = get_cache_dir();
    let current_exe = std::env::current_exe().expect("current_exe");

    // 1. JPEG Baseline 100 MP
    let p_base = cache_dir.join("test_baseline_100mp.jpg");
    ensure_baseline_jpeg_100mp(&p_base);
    println!("\n--> Menjalankan subprocess terisolasi untuk JPEG Baseline 100 MP...");
    let out1 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "baseline_100mp")
        .env("AAP_BENCH_FILE", &p_base)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 1");
    println!("{}", String::from_utf8_lossy(&out1.stdout));
    assert!(out1.status.success(), "Subprocess 1 harus sukses");

    // 2. JPEG Baseline 144 MP
    let p_base_144 = cache_dir.join("test_baseline_144mp.jpg");
    ensure_baseline_jpeg_144mp(&p_base_144);
    println!("\n--> Menjalankan subprocess terisolasi untuk JPEG Baseline 144 MP...");
    let out2 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "baseline_144mp")
        .env("AAP_BENCH_FILE", &p_base_144)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 2");
    println!("{}", String::from_utf8_lossy(&out2.stdout));
    assert!(out2.status.success(), "Subprocess 2 harus sukses");

    // 3. JPEG Progressive 40 MP
    let p_prog = cache_dir.join("test_progressive_40mp.jpg");
    ensure_progressive_jpeg_40mp(&p_prog);
    println!("\n--> Menjalankan subprocess terisolasi untuk JPEG Progressive 40 MP...");
    let out3 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "progressive_40mp")
        .env("AAP_BENCH_FILE", &p_prog)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 3");
    println!("{}", String::from_utf8_lossy(&out3.stdout));
    assert!(out3.status.success(), "Subprocess 3 harus sukses");

    // 4. PNG 100 MP RGBA Non-interlaced
    let p_png = cache_dir.join("test_png_100mp.png");
    ensure_png_100mp(&p_png);
    println!("\n--> Menjalankan subprocess terisolasi untuk PNG 100 MP RGBA...");
    let out4 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "png_100mp")
        .env("AAP_BENCH_FILE", &p_png)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 4");
    println!("{}", String::from_utf8_lossy(&out4.stdout));
    assert!(out4.status.success(), "Subprocess 4 harus sukses");

    // 5. PNG 25 MP RGBA Interlaced (Adam7)
    let p_png_adam7 = cache_dir.join("test_png_25mp_interlaced.png");
    ensure_png_25mp_interlaced(&p_png_adam7);
    println!("\n--> Menjalankan subprocess terisolasi untuk PNG 25 MP Interlaced (Adam7)...");
    let out5 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "png_25mp_interlaced")
        .env("AAP_BENCH_FILE", &p_png_adam7)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 5");
    println!("{}", String::from_utf8_lossy(&out5.stdout));
    assert!(out5.status.success(), "Subprocess 5 harus sukses");
}
