//! Benchmark Memori Puncak End-to-End Fase 2 (Opsi A)
//!
//! Mengukur konsumsi memori puncak sebenarnya (peak working set high-water mark dan peak pagefile / private commit)
//! via Windows API `K32GetProcessMemoryInfo` (PeakWorkingSetSize & PeakPagefileUsage) dan `K32EmptyWorkingSet`.
//! Menguji end-to-end melalui entry point worker sebenarnya (`execute_import_option_a`),
//! mencakup: validasi, streaming copy original, decode + IDCT downscaling, StreamingResampler,
//! loop adaptif display <= 5 MB, commit atomic staging, dan registrasi layer.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::{
    execute_import_option_a, execute_import_option_b,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::instance_lock::set_is_primary_instance;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::job_manager::try_acquire_job;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::store::delete_ortho_image_single;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::types::{
    ImportBoundsPayload, ImportDualModePayload,
};

#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessMemSnapshot {
    pub working_set_bytes: u64,
    pub peak_working_set_bytes: u64,
    pub pagefile_bytes: u64,
    pub peak_pagefile_bytes: u64,
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
            pagefile_bytes: counters.pagefile_usage as u64,
            peak_pagefile_bytes: counters.peak_pagefile_usage as u64,
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

/// Helper untuk membuat JPEG Baseline 100 MP (10.000 x 10.000) dengan gradien dan noise.
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

/// Helper untuk membuat JPEG Baseline 40 MP nyata (8.000 x 5.000 px) dengan chroma subsampling 4:2:0 (F_2_2).
fn ensure_baseline_jpeg_40mp_420(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 500_000 {
        return;
    }
    println!("Membuat berkas JPEG Baseline 40 MP 4:2:0 (8.000 x 5.000 px)...");
    let (width, height) = (8_000u32, 5_000u32);
    let file = File::create(path).expect("create baseline 40mp 420 jpg");
    let mut writer = BufWriter::new(file);

    let mut enc = jpeg_encoder::Encoder::new(&mut writer, 75);
    enc.set_progressive(false);
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_2_2); // 4:2:0 subsampling nyata

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
    .expect("encode baseline 40MP 420");
}

/// Helper untuk membuat JPEG Progressive 40 MP nyata menggunakan encoder progressive.
fn ensure_progressive_jpeg_40mp(path: &Path) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 500_000 {
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

/// Helper untuk membuat PNG 100 MP RGBA non-interlaced dengan gradien variatif <= 50 MB.
fn ensure_png_100mp(path: &Path) {
    let current_len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if path.is_file() && current_len > 1_000_000 && current_len <= 50_000_000 {
        return;
    }
    let _ = fs::remove_file(path);
    println!("Membuat berkas PNG 100 MP RGBA non-interlaced (10.000 x 10.000 px)...");
    let (width, height) = (10_000u32, 10_000u32);
    let file = File::create(path).expect("create png");
    let buf_writer = BufWriter::new(file);

    let mut enc = png::Encoder::new(buf_writer, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Default);

    let mut writer = enc.write_header().expect("write header");

    let mut full_buf = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let grad_y = ((y * 255) / height) as u8;
        let row_offset = (y * width * 4) as usize;
        for x in 0..width {
            let grad_x = ((x * 255) / width) as u8;
            let variation = ((x / 16) ^ (y / 16)) as u8 & 0x0F;
            let idx = row_offset + (x * 4) as usize;
            full_buf[idx] = grad_x.wrapping_add(variation);
            full_buf[idx + 1] = grad_y.wrapping_sub(variation);
            full_buf[idx + 2] = (grad_x ^ grad_y).wrapping_add(variation);
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
                let variation = ((x / 16) ^ (y / 16)) as u8 & 0x0F;
                raw_scanlines.push(grad_x.wrapping_add(variation));
                raw_scanlines.push(grad_y.wrapping_sub(variation));
                raw_scanlines.push((grad_x ^ grad_y).wrapping_add(variation));
                raw_scanlines.push(255);
                x += x_step;
            }
            y += y_step;
        }
    }

    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw_scanlines, 6);
    write_png_chunk(&mut writer, b"IDAT", &compressed);

    // IEND Chunk
    write_png_chunk(&mut writer, b"IEND", &[]);
}

/// Membuat GeoTIFF RGBA (strip, Deflate) lewat `write_data` (satu-satunya jalur yang mengaktifkan
/// kompresor di tiff 0.9.1; `write_strip` saja menulis tag Deflate tetapi data mentah).
/// Alpha: elips (di luar elips alpha = 0). Dibuat di PROSES INDUK, bukan di proses terukur.
fn ensure_geotiff_rgba(path: &Path, width: u32, height: u32, rows_per_strip: u32, bits16: bool) {
    if path.is_file() && fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 100_000 {
        return;
    }
    use tiff::encoder::compression::{Deflate, DeflateLevel};
    use tiff::encoder::{colortype, TiffEncoder};
    use tiff::tags::Tag;
    println!(
        "Membuat GeoTIFF RGBA{} {}x{} (Deflate, strip {} baris)...",
        if bits16 { "16" } else { "8" },
        width,
        height,
        rows_per_strip
    );
    let file = File::create(path).expect("create geotiff");
    let mut enc = TiffEncoder::new(BufWriter::new(file)).expect("tiff encoder");

    macro_rules! build {
        ($ct:ty, $conv:expr) => {{
            let mut img = enc
                .new_image_with_compression::<$ct, _>(
                    width,
                    height,
                    Deflate::with_level(DeflateLevel::Balanced),
                )
                .expect("new image");
            img.rows_per_strip(rows_per_strip).expect("rows per strip");
            let tiepoints = [0.0f64, 0.0, 0.0, 500000.0, 9866000.0, 0.0];
            img.encoder()
                .write_tag(Tag::ModelTiepointTag, &tiepoints[..])
                .unwrap();
            let scales = [1.0f64, 1.0, 0.0];
            img.encoder()
                .write_tag(Tag::ModelPixelScaleTag, &scales[..])
                .unwrap();
            let geokeys = [
                1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32750,
            ];
            img.encoder()
                .write_tag(Tag::GeoKeyDirectoryTag, &geokeys[..])
                .unwrap();
            img.encoder()
                .write_tag(Tag::ExtraSamples, &[2u16][..])
                .unwrap();
            let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
            let (rx, ry) = (width as f64 * 0.45, height as f64 * 0.40);
            let mut data = Vec::with_capacity(width as usize * height as usize * 4);
            for y in 0..height {
                let gy = ((y as u64 * 255) / height as u64) as u8;
                for x in 0..width {
                    let gx = ((x as u64 * 255) / width as u64) as u8;
                    let n = ((x ^ y) & 0x1F) as u8;
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let inside = (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) <= 1.0;
                    let px = [
                        gx.wrapping_add(n),
                        gy.wrapping_sub(n),
                        (gx ^ gy).wrapping_add(n),
                        if inside { 255u8 } else { 0u8 },
                    ];
                    for v in px {
                        data.push($conv(v));
                    }
                }
            }
            img.write_data(&data).expect("write_data");
        }};
    }
    if bits16 {
        build!(colortype::RGBA16, |v: u8| (v as u16) * 257);
    } else {
        build!(colortype::RGBA8, |v: u8| v);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PROSES ANAK TERISOLASI UNTUK PENGUKURAN PEAK WORKING SET & PEAK PAGEFILE
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn execute_isolated_memory_benchmarks() {
    // Mode worker terisolasi (via variabel lingkungan):
    if let Ok(bench_mode) = std::env::var("AAP_RUN_ISOLATED_BENCH") {
        let file_path = PathBuf::from(std::env::var("AAP_BENCH_FILE").expect("AAP_BENCH_FILE"));

        // Set hak akses instansi primer
        set_is_primary_instance(true);

        #[cfg(windows)]
        trim_working_set();

        #[cfg(windows)]
        let snap_before = get_memory_snapshot();

        let job_id = format!("bench_{}_{}", bench_mode, std::process::id());
        let token = try_acquire_job(&job_id).expect("try_acquire_job");

        let is_b = bench_mode.starts_with("geotiff") || bench_mode.starts_with("worst");
        let payload = ImportDualModePayload {
            mode: if is_b { "B" } else { "A" }.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            custom_filename: None,
            manual_bounds: if is_b {
                None
            } else {
                Some(ImportBoundsPayload {
                    west: 116.85,
                    south: -1.25,
                    east: 116.86,
                    north: -1.24,
                })
            },
            save_original: !is_b, // Opsi A: streaming copy file asli; Opsi B: tanpa salinan asli
        };

        let start_time = Instant::now();
        let result = if is_b {
            execute_import_option_b(&job_id, payload, token)
        } else {
            execute_import_option_a(&job_id, payload, token)
        };
        let elapsed = start_time.elapsed();

        #[cfg(windows)]
        let snap_after = get_memory_snapshot();

        let imported_meta = result.expect("impor (opsi A/B) harus berhasil");

        #[cfg(windows)]
        {
            let baseline_ws_mib = snap_before.working_set_bytes as f64 / (1024.0 * 1024.0);
            let peak_ws_mib = snap_after.peak_working_set_bytes as f64 / (1024.0 * 1024.0);
            let delta_ws_mib = peak_ws_mib - baseline_ws_mib;
            let peak_pagefile_mib = snap_after.peak_pagefile_bytes as f64 / (1024.0 * 1024.0);
            let duration_secs = elapsed.as_secs_f64();

            println!("\n================================================================================");
            println!(
                "PENGUKURAN END-TO-END WORKER IMPOR TERISOLASI: {}",
                bench_mode
            );
            println!(
                "================================================================================"
            );
            println!("- ID Citra Hasil: {}", imported_meta.id);
            println!("- Waktu Eksekusi Total: {:.2} detik", duration_secs);
            println!("- Dimensi Display Akhir: {:?}", imported_meta.dimensions);
            println!(
                "- Ukuran Display Akhir: {:.2} MiB ({} bytes)",
                imported_meta.display_size_bytes.unwrap_or(0) as f64 / (1024.0 * 1024.0),
                imported_meta.display_size_bytes.unwrap_or(0)
            );
            println!("- Baseline Working Set: {:.2} MiB", baseline_ws_mib);
            println!("- Peak Working Set Size: {:.2} MiB", peak_ws_mib);
            println!(
                "- Peak Pagefile Usage (Private Commit): {:.2} MiB",
                peak_pagefile_mib
            );
            println!("- Delta Memori Bersih: {:.2} MiB", delta_ws_mib);

            // Rincian teoritis dan sisa sebenarnya
            let (theoretical_decode_mib, theoretical_display_mib) = match bench_mode.as_str() {
                "baseline_100mp" => (75.0, 50.33), // IDCT 1/2: 5000x5000x3 + Display 4096x4096x3
                "baseline_144mp" => (108.0, 50.33), // IDCT 1/2: 6000x6000x3 + Display 4096x4096x3
                "baseline_40mp_420" => (40.0, 31.46), // IDCT 1/2: 4000x2500x3 + Display 4096x2560x3
                "progressive_40mp" => (120.0, 31.46), // Frame RGB 8000x5000x3 + Display 4096x2560x3
                "png_100mp" => (0.04, 67.11),      // Row streaming 10000x4 + Display 4096x4096x4
                "png_25mp_interlaced" => (100.0, 67.11), // Adam7 frame 5000x5000x4 + Display 4096x4096x4
                _ => (0.0, 0.0),
            };

            let theoretical_total = theoretical_decode_mib + theoretical_display_mib + 0.26; // +0.26 MiB akumulator
            let actual_remaining_mib = (delta_ws_mib - theoretical_total).max(0.0);

            println!("- Rincian Alokasi Teoritis:");
            println!(
                "  * Buffer Decode / Scanline: {:.2} MiB",
                theoretical_decode_mib
            );
            println!(
                "  * Buffer Output Display: {:.2} MiB",
                theoretical_display_mib
            );
            println!("  * Resampler Band Accumulator (RGBA 4096 px 2-baris f64): 0.26 MiB");
            println!(
                "  * Sisa Sebenarnya (Runtime Tauri, Heap Overhead, File Buffers): {:.2} MiB",
                actual_remaining_mib
            );
            println!(
                "BENCH_RESULT|{}|{:.2}|{:.2}|{:.2}|{:.2}|{:.2}|{:?}|{}|{}",
                bench_mode,
                baseline_ws_mib,
                peak_ws_mib,
                peak_pagefile_mib,
                delta_ws_mib,
                duration_secs,
                imported_meta.dimensions,
                imported_meta.display_size_bytes.unwrap_or(0),
                if cfg!(debug_assertions) {
                    "test/dev (crate uji opt-level 0; png/tiff/jpeg/image opt-level 3)"
                } else {
                    "release"
                }
            );
            println!("- Target Sasaran Desain: <= 400.00 MiB");
            println!("================================================================================\n");

            // Pemeriksaan isi display nyata (setelah snapshot memori): dimensi, % piksel tidak transparan, rata-rata RGB
            {
                use aerial_analytics_platform_lib::modules::module_03::storage_utils::get_storage_root;
                let disp = get_storage_root()
                    .join("images")
                    .join(&imported_meta.id)
                    .join(format!("display.{}", imported_meta.extension));
                let rgba = image::open(&disp)
                    .expect("display harus bisa didekode")
                    .to_rgba8();
                let n = (rgba.width() as u64) * (rgba.height() as u64);
                let opaque = rgba.pixels().filter(|p| p[3] > 0).count() as u64;
                let (mut sr, mut sg, mut sb) = (0u64, 0u64, 0u64);
                for p in rgba.pixels().filter(|p| p[3] > 0) {
                    sr += p[0] as u64;
                    sg += p[1] as u64;
                    sb += p[2] as u64;
                }
                let d = opaque.max(1) as f64;
                println!(
                    "DISPLAY_CHECK|{}|{}x{}|opaque {:.1}%|mean RGB {:.1}/{:.1}/{:.1}",
                    bench_mode,
                    rgba.width(),
                    rgba.height(),
                    opaque as f64 * 100.0 / n as f64,
                    sr as f64 / d,
                    sg as f64 / d,
                    sb as f64 / d
                );
            }

            // Bersihkan hasil import benchmark dari storage agar tidak membebani disk
            let _ = delete_ortho_image_single(&imported_meta.id);

            assert!(
                peak_ws_mib <= 400.0,
                "Puncak memori Working Set ({:.2} MiB) melebihi batas 400 MiB",
                peak_ws_mib
            );
        }
        return;
    }

    // MODE ORCHESTRATOR: fixture dibuat di proses induk; setiap skenario di SUBPROSES terpisah.
    let cache_dir = get_cache_dir();
    let current_exe = std::env::current_exe().expect("current_exe");

    let p_base = cache_dir.join("test_baseline_100mp.jpg");
    ensure_baseline_jpeg_100mp(&p_base);
    let p_base_144 = cache_dir.join("test_baseline_144mp.jpg");
    ensure_baseline_jpeg_144mp(&p_base_144);
    let p_base_420 = cache_dir.join("test_baseline_40mp_420.jpg");
    ensure_baseline_jpeg_40mp_420(&p_base_420);
    let p_prog = cache_dir.join("test_progressive_40mp.jpg");
    ensure_progressive_jpeg_40mp(&p_prog);
    let p_png = cache_dir.join("test_png_100mp.png");
    ensure_png_100mp(&p_png);
    let p_png_adam7 = cache_dir.join("test_png_25mp_interlaced.png");
    ensure_png_25mp_interlaced(&p_png_adam7);
    let p_gt8 = cache_dir.join("test_geotiff_rgba8_8000x6000.tif");
    ensure_geotiff_rgba(&p_gt8, 8_000, 6_000, 64, false);
    let p_gt16 = cache_dir.join("test_geotiff_rgba16_6000x4500.tif");
    ensure_geotiff_rgba(&p_gt16, 6_000, 4_500, 64, true);
    let p_w22 = cache_dir.join("test_worst_22000x4000_rgba16.tif");
    ensure_geotiff_rgba(&p_w22, 22_000, 4_000, 64, true);

    let scenarios: Vec<(&str, &PathBuf)> = vec![
        ("baseline_100mp", &p_base),
        ("baseline_144mp", &p_base_144),
        ("baseline_40mp_420", &p_base_420),
        ("progressive_40mp", &p_prog),
        ("png_100mp", &p_png),
        ("png_25mp_interlaced", &p_png_adam7),
        ("geotiff_rgba8", &p_gt8),
        ("geotiff_rgba16", &p_gt16),
        ("worst_22k_rgba16", &p_w22),
    ];

    let mut table: Vec<String> = Vec::new();
    for (mode, path) in scenarios {
        println!("\n--> Subproses terisolasi: {} ({:?})", mode, path);
        let out = Command::new(&current_exe)
            .env("AAP_RUN_ISOLATED_BENCH", mode)
            .env("AAP_BENCH_FILE", path)
            .arg("--exact")
            .arg("execute_isolated_memory_benchmarks")
            .arg("--nocapture")
            .output()
            .expect("execute subprocess");
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        println!("{}", stdout);
        if !out.status.success() {
            eprintln!("STDERR {}:\n{}", mode, String::from_utf8_lossy(&out.stderr));
        }
        assert!(out.status.success(), "Subproses {} harus sukses", mode);
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("BENCH_RESULT|") {
                table.push(rest.to_string());
            }
            if let Some(rest) = line.strip_prefix("DISPLAY_CHECK|") {
                table.push(format!("DISPLAY_CHECK|{}", rest));
            }
        }
    }

    println!("\nRINGKASAN (MiB; skenario|baseline WS|peak WS|peak pagefile|delta WS|detik|dimensi display|bytes display|mode build)");
    for row in &table {
        println!("RINGKASAN|{}", row);
    }
}
