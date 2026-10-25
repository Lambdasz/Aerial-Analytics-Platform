//! Benchmark Memori Puncak Fase 2 (Opsi A) - Pengukuran Nyata Berbasis PeakWorkingSetSize
//!
//! Mengukur konsumsi memori puncak sebenarnya (peak working set high-water mark) via Windows API
//! `K32GetProcessMemoryInfo` (PeakWorkingSetSize) dan `K32EmptyWorkingSet`.
//! Menjalankan proses decoding di proses anak terisolasi agar memori proses tidak tercemar
//! oleh alokasi pembuatan berkas sintetis (buffer generator 300 MB).

use std::fs::{self, File};
use std::io::BufWriter;
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
    println!("Membuat berkas PNG 100 MP RGBA (10.000 x 10.000 px)...");
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
                    println!("- Skala IDCT yang dipilih: 1/2 (5.000 x 5.000) -> 4.096 x 4.096 px");
                    println!(
                        "- Dimensi hasil akhir: {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Batas Desain Dokumen: 233 MB");
                    println!("====================================================\n");
                }
            }
            "progressive_40mp" => {
                let file = File::open(&file_path).unwrap();
                let buf_reader = std::io::BufReader::new(file);
                let mut decoder = jpeg_decoder::Decoder::new(buf_reader);
                let pixels = decoder.decode().expect("decode prog");
                let img = image::RgbImage::from_raw(8000, 5000, pixels).unwrap();
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!("\n=== [PENGUKURAN TERISOLASI] JPEG PROGRESSIVE 40 MP ===");
                    println!(
                        "- Dimensi sumber: {}x{} px (40 MP)",
                        img.width(),
                        img.height()
                    );
                    println!("- Skala IDCT: Tidak didukung decoder progressive (full spectral coefficient accumulation)");
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("- Ambang Batas Evaluasi: 400 MB");
                    println!("======================================================\n");
                }
            }
            "png_100mp" => {
                let (decoded, has_alpha) =
                    decode_and_scale_png(&file_path, 4096).expect("decode png");
                #[cfg(windows)]
                {
                    let snap_after = get_memory_snapshot();
                    let baseline_mb = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
                    let peak_mb = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
                    let delta_mb = peak_mb - baseline_mb;
                    println!("\n=== [PENGUKURAN TERISOLASI] PNG 100 MP RGBA ===");
                    println!("- Memiliki alpha: {}", has_alpha);
                    println!(
                        "- Dimensi hasil akhir: {}x{} px",
                        decoded.width(),
                        decoded.height()
                    );
                    println!("- Baseline Working Set: {:.2} MB", baseline_mb);
                    println!("- Peak Working Set Size: {:.2} MB", peak_mb);
                    println!("- Delta Memori: {:.2} MB", delta_mb);
                    println!("===============================================\n");
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

    // 2. JPEG Progressive 40 MP
    let p_prog = cache_dir.join("test_progressive_40mp.jpg");
    ensure_progressive_jpeg_40mp(&p_prog);
    println!("\n--> Menjalankan subprocess terisolasi untuk JPEG Progressive 40 MP...");
    let out2 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "progressive_40mp")
        .env("AAP_BENCH_FILE", &p_prog)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 2");
    println!("{}", String::from_utf8_lossy(&out2.stdout));
    assert!(out2.status.success(), "Subprocess 2 harus sukses");

    // 3. PNG 100 MP RGBA
    let p_png = cache_dir.join("test_png_100mp.png");
    ensure_png_100mp(&p_png);
    println!("\n--> Menjalankan subprocess terisolasi untuk PNG 100 MP RGBA...");
    let out3 = Command::new(&current_exe)
        .env("AAP_RUN_ISOLATED_BENCH", "png_100mp")
        .env("AAP_BENCH_FILE", &p_png)
        .arg("--exact")
        .arg("execute_isolated_memory_benchmarks")
        .arg("--nocapture")
        .output()
        .expect("execute subprocess 3");
    println!("{}", String::from_utf8_lossy(&out3.stdout));
    assert!(out3.status.success(), "Subprocess 3 harus sukses");
}
