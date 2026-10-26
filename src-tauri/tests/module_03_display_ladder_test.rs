//! Tes logika tangga adaptif display (PNG alpha 10 MiB vs JPEG 5 MiB) dan cancel saat encode PNG.

use std::time::{Duration, Instant};

use aerial_analytics_platform_lib::modules::module_03::error::OrthoImportError;
use aerial_analytics_platform_lib::modules::module_03::ortho_source::constants::{
    DISPLAY_MAX_BYTES_JPEG, DISPLAY_MAX_BYTES_PNG_ALPHA, DISPLAY_MIN_DIMENSION_PX,
    PNG_DIMENSION_LADDER,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::{
    adaptive_encode_png, display_limit_bytes, encode_png_banded, png_initial_dimension,
    png_next_dimension, PngEncodeSettings,
};
use aerial_analytics_platform_lib::modules::module_03::ortho_source::job_manager::CancellationToken;

const MIB: u64 = 1024 * 1024;

fn noise_rgba(w: u32, h: u32) -> image::DynamicImage {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    for px in buf.chunks_exact_mut(4) {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let v = (state >> 33) as u32;
        px[0] = v as u8;
        px[1] = (v >> 8) as u8;
        px[2] = (v >> 16) as u8;
        px[3] = 255;
    }
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, buf).unwrap())
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ladder_{}_{}_{}",
        name,
        std::process::id(),
        chrono::Utc::now().timestamp_micros()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_limits_are_mib_and_differ_for_png_alpha_and_jpeg() {
    assert_eq!(DISPLAY_MAX_BYTES_JPEG, 5 * MIB);
    assert_eq!(DISPLAY_MAX_BYTES_PNG_ALPHA, 10 * MIB);
    assert_eq!(display_limit_bytes(false), 5 * 1_048_576);
    assert_eq!(display_limit_bytes(true), 10 * 1_048_576);
}

#[test]
fn test_ladder_is_strictly_descending_and_ends_at_floor() {
    assert_eq!(
        PNG_DIMENSION_LADDER,
        [4096, 3584, 3072, 2560, 2304, 2048, 1792, 1536, 1280, 1024]
    );
    assert!(PNG_DIMENSION_LADDER.windows(2).all(|w| w[0] > w[1]));
    assert_eq!(
        *PNG_DIMENSION_LADDER.last().unwrap(),
        DISPLAY_MIN_DIMENSION_PX
    );
}

#[test]
fn test_initial_dimension_selection() {
    assert_eq!(png_initial_dimension(20_111), 4096);
    assert_eq!(png_initial_dimension(4096), 4096);
    assert_eq!(png_initial_dimension(3000), 2560);
    assert_eq!(png_initial_dimension(2048), 2048);
    assert_eq!(png_initial_dimension(1500), 1280);
    // Sumber lebih kecil dari lantai -> lantai 1.024
    assert_eq!(png_initial_dimension(500), DISPLAY_MIN_DIMENSION_PX);
}

#[test]
fn test_quadratic_jump_skips_rungs_and_picks_nearest_fitting() {
    let limit = 10 * MIB;
    // 4096 px menghasilkan 34 MiB: estimasi 4096*sqrt(9.5/34) ~ 2165 -> lompat langsung ke 2048
    assert_eq!(png_next_dimension(4096, 34 * MIB, limit), 2048);
    // Sedikit di atas batas (11 MiB): estimasi ~3806 -> 3584 (hanya satu anak tangga)
    assert_eq!(png_next_dimension(4096, 11 * MIB, limit), 3584);
    // 2048 px menghasilkan 11,22 MB (~10,7 MiB): estimasi ~1925 -> 1792
    assert_eq!(png_next_dimension(2048, 11_220_298, limit), 1792);
}

#[test]
fn test_next_dimension_always_decreases_and_respects_floor() {
    let limit = 10 * MIB;
    for &cur in PNG_DIMENSION_LADDER.iter() {
        // Ukuran hanya sedikit di atas batas: tetap harus turun minimal satu anak tangga.
        let next = png_next_dimension(cur, limit + 1, limit);
        if cur > DISPLAY_MIN_DIMENSION_PX {
            assert!(next < cur, "cur={cur} next={next} harus turun");
        }
        assert!(next >= DISPLAY_MIN_DIMENSION_PX);
        assert!(PNG_DIMENSION_LADDER.contains(&next));
        // Ukuran sangat besar: turun tetapi tidak di bawah lantai.
        let next_huge = png_next_dimension(cur, 10_000 * MIB, limit);
        assert!(next_huge >= DISPLAY_MIN_DIMENSION_PX);
    }
    assert_eq!(
        png_next_dimension(DISPLAY_MIN_DIMENSION_PX, 10_000 * MIB, limit),
        DISPLAY_MIN_DIMENSION_PX
    );
}

#[test]
fn test_limit_changes_selection_png_10_vs_jpeg_5() {
    // Ukuran 8 MiB: lolos batas PNG alpha (10 MiB) tetapi melebihi batas JPEG (5 MiB).
    let size = 8 * MIB;
    assert!(size <= display_limit_bytes(true));
    assert!(size > display_limit_bytes(false));
    // Terhadap 5 MiB harus turun dari 4096; terhadap 10 MiB tidak perlu turun.
    assert!(png_next_dimension(4096, size, display_limit_bytes(false)) < 4096);
}

#[test]
fn test_adaptive_png_reaches_floor_when_limit_unreachable() {
    // Derau murni 2600x2600 tidak bisa muat 1 MiB bahkan di 1.024 px (~4 MiB) -> berhenti di lantai.
    let img = noise_rgba(2600, 2600);
    let dir = temp_dir("floor");
    let res = adaptive_encode_png(&img, &dir, None, MIB, PngEncodeSettings::default())
        .expect("encode adaptif");
    assert_eq!(res.final_dimensions[0].max(res.final_dimensions[1]), 1024);
    assert!(
        res.display_size_bytes > MIB,
        "lantai tercapai walau melebihi batas"
    );
    assert!(res
        .warnings
        .iter()
        .any(|w| w.contains("di bawah 2.048 piksel")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_adaptive_png_stops_on_ladder_rung_within_limit() {
    let img = noise_rgba(2600, 2600);
    let dir = temp_dir("rung");
    let limit = 12 * MIB;
    let res = adaptive_encode_png(&img, &dir, None, limit, PngEncodeSettings::default())
        .expect("encode adaptif");
    let dim = res.final_dimensions[0].max(res.final_dimensions[1]);
    assert!(
        PNG_DIMENSION_LADDER.contains(&dim),
        "dim {dim} harus anak tangga"
    );
    assert!(res.display_size_bytes <= limit);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_banded_png_roundtrip_matches_source() {
    let img = noise_rgba(300, 200);
    let dir = temp_dir("roundtrip");
    let path = dir.join("rt.png");
    encode_png_banded(&img, &path, PngEncodeSettings::default(), None).expect("encode");
    let decoded = image::open(&path).expect("decode").to_rgba8();
    assert_eq!(decoded.as_raw(), img.to_rgba8().as_raw());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_banded_png_cancel_between_bands_is_fast() {
    let img = noise_rgba(4096, 4096);
    let dir = temp_dir("cancel");
    let path = dir.join("c.png");
    let token = CancellationToken::new();
    let t2 = token.clone();
    let path2 = path.clone();
    let handle = std::thread::spawn(move || {
        encode_png_banded(&img, &path2, PngEncodeSettings::default(), Some(&t2))
    });
    std::thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    token.cancel();
    let res = handle.join().expect("join");
    let latency = start.elapsed();
    println!(
        "LATENSI CANCEL (encode PNG 4096 px, band 64 baris): {:?}",
        latency
    );
    assert!(
        matches!(res, Err(OrthoImportError::Dibatalkan)),
        "harus Dibatalkan: {res:?}"
    );
    assert!(
        latency < Duration::from_secs(1),
        "latensi {latency:?} >= 1 detik"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_proxy_3b_display_jpg_png_alpha_combinations() {
    use aerial_analytics_platform_lib::modules::module_03::ortho_source::image_pipeline::resample_dynamic_image_box_filter;

    let img_path = std::path::Path::new("target/ortho_real_check/display.jpg");
    if !img_path.exists() {
        println!(
            "display.jpg tidak ditemukan di target/ortho_real_check/display.jpg, melewati tes."
        );
        return;
    }
    let dyn_img = image::open(img_path).expect("open display.jpg");
    let (orig_w, orig_h) = (dyn_img.width(), dyn_img.height());
    assert_eq!((orig_w, orig_h), (4096, 2767));

    // Tambahkan mask elips transparan: radius 0.45 x W dan 0.40 x H
    let mut rgba = dyn_img.to_rgba8();
    let (cx, cy) = (orig_w as f64 / 2.0, orig_h as f64 / 2.0);
    let (rx, ry) = (orig_w as f64 * 0.45, orig_h as f64 * 0.40);
    for y in 0..orig_h {
        let dy = y as f64 - cy;
        for x in 0..orig_w {
            let dx = x as f64 - cx;
            let inside = ((dx * dx) / (rx * rx)) + ((dy * dy) / (ry * ry)) <= 1.0;
            if !inside {
                rgba.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
    }
    let base_rgba = image::DynamicImage::ImageRgba8(rgba);

    let dims = [4096u32, 3072, 2048, 1792];
    let settings_list = [
        (
            "Default (Default + Sub + Adaptive)",
            PngEncodeSettings::default(),
        ),
        (
            "Adaptive+Best (Best + Sub + Adaptive)",
            PngEncodeSettings {
                compression: png::Compression::Best,
                filter: png::FilterType::Sub,
                adaptive: png::AdaptiveFilterType::Adaptive,
            },
        ),
    ];

    let dir = temp_dir("proxy_3b");
    println!("\n================================================================================");
    println!("PENGUKURAN PROKSI 3B: DISPLAY.JPG REAL OAM DENGAN ALPHA ELIPS");
    println!("Resolusi Asal: {} x {} px", orig_w, orig_h);
    println!("================================================================================");

    for &dim in &dims {
        let resized = if dim >= orig_w.max(orig_h) {
            base_rgba.clone()
        } else {
            resample_dynamic_image_box_filter(&base_rgba, dim).expect("resample")
        };
        let (rw, rh) = (resized.width(), resized.height());

        for (label, settings) in &settings_list {
            let out_file = dir.join(format!("out_{}_{}.png", dim, label.replace(' ', "_")));
            let t0 = Instant::now();
            let size = encode_png_banded(&resized, &out_file, *settings, None).expect("encode");
            let dur = t0.elapsed();
            let mib = size as f64 / (1024.0 * 1024.0);
            println!(
                "PROXI_3B_RESULT|dim_max={}|actual={}x{}|settings={}|bytes={}|mib={:.3}|secs={:.2}",
                dim,
                rw,
                rh,
                label,
                size,
                mib,
                dur.as_secs_f64()
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
