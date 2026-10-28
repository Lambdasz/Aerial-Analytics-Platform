//! Module 4 - Eksperimen mask (di luar kontrak plugin)
//!
//! Cara pakai:
//!     cargo run --release --bin experiment -- --images images/ --out hasil_mask
//!     (opsional: --method ExG|ExR|VARI  --threshold 0.10)

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use rgb_vegetation::{
    apply_threshold, calculate_vegetation_index, count_green_pixels, default_threshold,
    generate_mask_artifact, load_image, AnalysisError,
};
use serde_json::{json, Value};

const IMAGE_EXTS: [&str; 5] = [".jpg", ".jpeg", ".png", ".tif", ".tiff"];

struct MaskExport {
    vegetation_mask: String,
    green_coverage: usize,
    image_size: (usize, usize), // (tinggi, lebar)
}

fn export_mask_for_downstream(
    image_path: &str,
    output_dir: &str,
    method: &str,
    threshold_value: f64,
) -> Result<MaskExport, AnalysisError> {
    let rgb = load_image(image_path)?;
    let index = calculate_vegetation_index(&rgb, method)?;
    let mask = apply_threshold(&index, threshold_value)?;

    fs::create_dir_all(output_dir).map_err(|e| AnalysisError::Io(e.to_string()))?;
    let stem = Path::new(image_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mask_path = Path::new(output_dir).join(format!("{stem}_{method}_vegetation_mask.png"));
    let vegetation_mask = generate_mask_artifact(&mask, rgb.width, rgb.height, &mask_path.to_string_lossy())?;

    Ok(MaskExport {
        vegetation_mask,
        green_coverage: count_green_pixels(&mask),
        image_size: (rgb.height, rgb.width),
    })
}

/// Baca ulang mask PNG: (nilai mentah 0..255, lebar, tinggi).
fn read_mask_raw(mask_path: &str) -> Result<(Vec<u8>, usize, usize), AnalysisError> {
    let gray = image::open(mask_path)
        .map_err(|e| AnalysisError::Io(e.to_string()))?
        .to_luma8();
    Ok((gray.as_raw().clone(), gray.width() as usize, gray.height() as usize))
}

/// Proses satu gambar: (baris ringkasan JSON, ok?, teks log).
fn process_image(images_dir: &str, out_dir: &str, name: &str, method: &str, threshold: f64) -> (Value, bool, String) {
    let verify = || -> Result<(Value, bool, String), AnalysisError> {
        let path = Path::new(images_dir).join(name);
        let hasil = export_mask_for_downstream(&path.to_string_lossy(), out_dir, method, threshold)?;
        let (raw, mask_w, mask_h) = read_mask_raw(&hasil.vegetation_mask)?;
        let (h, w) = hasil.image_size;

        let values: BTreeSet<u8> = raw.iter().copied().collect();
        let values_ok = values.is_subset(&BTreeSet::from([0, 255]));
        let shape_ok = (mask_h, mask_w) == (h, w);
        let binary_sum = raw.iter().filter(|&&v| v > 0).count();
        let count_ok = binary_sum == hasil.green_coverage;
        let ok = values_ok && shape_ok && count_ok;

        let row = json!({
            "image": name,
            "vegetation_mask": hasil.vegetation_mask,
            "green_coverage": hasil.green_coverage,
            "image_size": [h, w],
            "mask_values_0_255": values_ok,
            "mask_shape_sama_dengan_gambar": shape_ok,
            "hitungan_pixel_konsisten": count_ok,
            "ok": ok,
        });
        let log = format!(
            "[{}] {name}  {w}x{h}px  green_coverage={} (~{:.1}% pixel)\n        mask_path: {}",
            if ok { "OK " } else { "GAGAL" },
            hasil.green_coverage,
            100.0 * hasil.green_coverage as f64 / (h * w) as f64,
            hasil.vegetation_mask,
        );
        Ok((row, ok, log))
    };

    verify().unwrap_or_else(|err| {
        let error = err.to_string();
        (
            json!({ "image": name, "ok": false, "error": error }),
            false,
            format!("[GAGAL] {name}  {error}"),
        )
    })
}

fn run_experiment(images_dir: &str, out_dir: &str, method: &str, threshold: Option<f64>) -> ExitCode {
    if !Path::new(images_dir).is_dir() {
        println!("Folder foto tidak ditemukan: {images_dir}");
        return ExitCode::FAILURE;
    }
    let threshold = threshold.or_else(|| default_threshold(method)).unwrap_or(0.10);

    let names: BTreeSet<String> = fs::read_dir(images_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let lower = n.to_lowercase();
            IMAGE_EXTS.iter().any(|ext| lower.ends_with(ext)) && !n.contains("_vegetation_mask")
        })
        .collect();

    if names.is_empty() {
        println!("Tidak ada gambar {IMAGE_EXTS:?} di folder: {images_dir}");
        return ExitCode::FAILURE;
    }

    println!("method={method}  threshold={threshold:?}\n");
    let results: Vec<(Value, bool, String)> = names
        .iter()
        .map(|name| process_image(images_dir, out_dir, name, method, threshold))
        .collect();
    results.iter().for_each(|(_, _, log)| println!("{log}"));

    let summary: Vec<&Value> = results.iter().map(|(row, _, _)| row).collect();
    let summary_path = Path::new(out_dir).join(format!("experiment_summary_{method}.json"));
    let written = fs::create_dir_all(out_dir).and_then(|()| {
        fs::write(&summary_path, serde_json::to_string_pretty(&summary).unwrap_or_default())
    });
    if let Err(e) = written {
        eprintln!("Gagal menulis ringkasan: {e}");
        return ExitCode::FAILURE;
    }

    let ok_count = results.iter().filter(|(_, ok, _)| *ok).count();
    println!(
        "\n{ok_count}/{} gambar lolos. Ringkasan: {}",
        results.len(),
        summary_path.display()
    );
    if ok_count == results.len() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let Some(images) = flag_value(&args, "--images") else {
        eprintln!("error: --images wajib diisi\nUsage: experiment --images <folder> [--out hasil_mask] [--method ExG|ExR|VARI] [--threshold 0.10]");
        return ExitCode::from(2);
    };
    let out = flag_value(&args, "--out").unwrap_or("hasil_mask");
    let method = flag_value(&args, "--method").unwrap_or("ExG");
    if default_threshold(method).is_none() {
        eprintln!("error: --method harus salah satu dari ExG, ExR, VARI");
        return ExitCode::from(2);
    }
    let threshold = match flag_value(&args, "--threshold").map(str::parse::<f64>) {
        None => None,
        Some(Ok(t)) => Some(t),
        Some(Err(_)) => {
            eprintln!("error: --threshold harus berupa angka");
            return ExitCode::from(2);
        }
    };

    run_experiment(images, out, method, threshold)
}
