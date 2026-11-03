//! Pembaca World File (.jgw, .pgw, .wld, .tfw) dan file proyeksi pendamping (.prj).
//! Menerapkan pergeseran setengah piksel (half-pixel shift), penolakan rotasi/kemiringan,
//! dan reproyeksi koordinat ke WGS84.

use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::module_03::error::OrthoImportError;
use crate::modules::module_03::geometry::projection::compute_wgs84_envelope_from_corners;

/// Struktur parameter affine 6-baris dari World File standard ESRI.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldFileParameters {
    /// Garis 1 (A): Ukuran piksel pada sumbu X (mis. Easting atau Longitude per piksel)
    pub a: f64,
    /// Garis 2 (D): Komponen rotasi sumbu Y
    pub d: f64,
    /// Garis 3 (B): Komponen rotasi sumbu X
    pub b: f64,
    /// Garis 4 (E): Ukuran piksel pada sumbu Y (biasanya bernilai negatif)
    pub e: f64,
    /// Garis 5 (C): Koordinat X pusat piksel kiri-atas
    pub c: f64,
    /// Garis 6 (F): Koordinat Y pusat piksel kiri-atas
    pub f: f64,
}

/// Hasil pembacaan World File lengkap dengan batas WGS84 dan deteksi CRS.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldFileResult {
    /// Bounding box WGS84: [[south, west], [north, east]]
    pub bounds: [[f64; 2]; 2],
    /// Kode EPSG terdeteksi (mis. 4326, 32750, 3857)
    pub detected_crs: u32,
    /// Nama file pendamping yang ditemukan (.jgw/.wld dan opsional .prj)
    pub world_file_path: PathBuf,
    pub prj_file_path: Option<PathBuf>,
}

/// Mencari kandidat world file yang bersesuaian dengan citra sumber.
/// Contoh: `DJI_0042.jpg` -> `DJI_0042.jgw`, `DJI_0042.wld`, `DJI_0042.jpgw`
pub fn find_candidate_world_file(image_path: &Path) -> Option<PathBuf> {
    let parent = image_path.parent()?;
    let stem = image_path.file_stem()?.to_str()?;
    let ext = image_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let specific_ext = match ext.as_str() {
        "jpg" | "jpeg" => vec!["jgw", "wld", "jpgw", "jpegw"],
        "png" => vec!["pgw", "wld", "pngw"],
        "tif" | "tiff" => vec!["tfw", "wld", "tiffw"],
        _ => vec!["wld"],
    };

    for candidate_ext in specific_ext {
        let candidate = parent.join(format!("{}.{}", stem, candidate_ext));
        if candidate.is_file() {
            return Some(candidate);
        }
        // Coba versi huruf besar
        let candidate_upper = parent.join(format!("{}.{}", stem, candidate_ext.to_uppercase()));
        if candidate_upper.is_file() {
            return Some(candidate_upper);
        }
    }

    None
}

/// Mencari file pendamping .prj untuk citra sumber.
pub fn find_candidate_prj_file(image_path: &Path) -> Option<PathBuf> {
    let parent = image_path.parent()?;
    let stem = image_path.file_stem()?.to_str()?;

    let prj_lower = parent.join(format!("{}.prj", stem));
    if prj_lower.is_file() {
        return Some(prj_lower);
    }
    let prj_upper = parent.join(format!("{}.PRJ", stem));
    if prj_upper.is_file() {
        return Some(prj_upper);
    }

    None
}

/// Mengurai isi teks world file 6 baris.
pub fn parse_world_file_content(content: &str) -> Result<WorldFileParameters, OrthoImportError> {
    let lines: Vec<&str> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 6 {
        return Err(OrthoImportError::FileKorup(format!(
            "World file tidak valid: membutuhkan 6 baris numerik, hanya ditemukan {}",
            lines.len()
        )));
    }

    let a = lines[0].parse::<f64>().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca baris 1 (A) world file: {}", e))
    })?;
    let d = lines[1].parse::<f64>().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca baris 2 (D) world file: {}", e))
    })?;
    let b = lines[2].parse::<f64>().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca baris 3 (B) world file: {}", e))
    })?;
    let e = lines[3].parse::<f64>().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca baris 4 (E) world file: {}", e))
    })?;
    let c = lines[4].parse::<f64>().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca baris 5 (C) world file: {}", e))
    })?;
    let f = lines[5].parse::<f64>().map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca baris 6 (F) world file: {}", e))
    })?;

    Ok(WorldFileParameters { a, d, b, e, c, f })
}

/// Mengurai kode EPSG dari teks .prj (WKT CRS).
pub fn parse_prj_crs(prj_content: &str) -> Result<u32, OrthoImportError> {
    let upper = prj_content.to_uppercase();

    // Deteksi UTM 50S (Balikpapan / IKN)
    if upper.contains("32750")
        || upper.contains("UTM ZONE 50S")
        || upper.contains("UTM ZONE 50, SOUTHERN")
        || upper.contains("WGS_1984_UTM_ZONE_50S")
        || upper.contains("WGS 84 / UTM ZONE 50S")
    {
        return Ok(32750);
    }

    // Deteksi UTM 50N
    if upper.contains("32650")
        || upper.contains("UTM ZONE 50N")
        || upper.contains("WGS_1984_UTM_ZONE_50N")
    {
        return Ok(32650);
    }

    // Deteksi Web Mercator (EPSG:3857)
    if upper.contains("3857")
        || upper.contains("PSEUDO-MERCATOR")
        || upper.contains("POPULAR VISUALISATION CRS")
        || upper.contains("WGS 84 / PSEUDO-MERCATOR")
    {
        return Ok(3857);
    }

    // Deteksi Geografis WGS84 (EPSG:4326)
    if upper.contains("4326")
        || upper.contains("GCS_WGS_1984")
        || upper.contains("WGS_1984") && !upper.contains("UTM")
    {
        return Ok(4326);
    }

    // Pencarian tag AUTHORITY["EPSG","<code>"]
    if let Some(pos) = upper.rfind(r#"AUTHORITY["EPSG","#) {
        let remainder = &upper[pos + 18..];
        if let Some(end_quote) = remainder.find('"') {
            if let Ok(code) = remainder[..end_quote].trim().parse::<u32>() {
                return Ok(code);
            }
        }
    }

    Err(OrthoImportError::CrsTidakDidukung(format!(
        "Sistem koordinat pada file .prj belum didukung otomatis: {}",
        &prj_content[..prj_content.len().min(80)]
    )))
}

/// Menghitung batas koordinat WGS84 dari World File dan dimensi citra.
///
/// # Syarat:
/// 1. Pergeseran setengah piksel: koordinat C dan F adalah pusat piksel kiri-atas,
///    sehingga tepi luar citra adalah (C - A/2, F - E/2).
/// 2. Tolak rotasi/skew: jika B != 0 atau D != 0, return `RotasiTidakDidukung`.
/// 3. Deteksi CRS: jika tanpa .prj dan koordinat tampak derajat desimal (antara -180..180 dan -90..90),
///    anggap EPSG:4326. Jika tidak, tolak dengan `CrsTidakDidukung`.
pub fn read_world_file_envelope(
    world_file_path: &Path,
    prj_file_path: Option<&Path>,
    width_px: u32,
    height_px: u32,
) -> Result<WorldFileResult, OrthoImportError> {
    if width_px == 0 || height_px == 0 {
        return Err(OrthoImportError::MelebihiBatas {
            width: width_px,
            height: height_px,
            max_dimension: 16384,
            message: "Dimensi citra tidak valid (0 piksel)".to_string(),
        });
    }

    let content = fs::read_to_string(world_file_path).map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca berkas world file: {}", e))
    })?;

    let params = parse_world_file_content(&content)?;

    // Periksa rotasi dan skew
    if params.b.abs() > 1e-9 || params.d.abs() > 1e-9 {
        let angle_approx = params.b.abs().max(params.d.abs());
        return Err(OrthoImportError::RotasiTidakDidukung(angle_approx));
    }

    // Pergeseran setengah piksel (half-pixel shift) ke batas luar terluar citra
    let x_left = params.c - (params.a / 2.0);
    let y_top = params.f - (params.e / 2.0);

    let x_right = x_left + (width_px as f64 * params.a);
    let y_bottom = y_top + (height_px as f64 * params.e);

    let x_min = x_left.min(x_right);
    let x_max = x_left.max(x_right);
    let y_min = y_bottom.min(y_top);
    let y_max = y_bottom.max(y_top);

    // Tentukan CRS
    let detected_crs = if let Some(prj_path) = prj_file_path {
        if prj_path.is_file() {
            let prj_text = fs::read_to_string(prj_path).unwrap_or_default();
            parse_prj_crs(&prj_text)?
        } else {
            fallback_crs_detection(x_min, x_max, y_min, y_max)?
        }
    } else {
        fallback_crs_detection(x_min, x_max, y_min, y_max)?
    };

    let corners = vec![
        (x_min, y_max), // Top-left
        (x_max, y_max), // Top-right
        (x_max, y_min), // Bottom-right
        (x_min, y_min), // Bottom-left
    ];

    let wgs84_bounds = compute_wgs84_envelope_from_corners(&corners, detected_crs)?;

    Ok(WorldFileResult {
        bounds: wgs84_bounds,
        detected_crs,
        world_file_path: world_file_path.to_path_buf(),
        prj_file_path: prj_file_path.map(|p| p.to_path_buf()),
    })
}

/// Attempts fallback CRS detection by inspecting coordinate bounds magnitude (detects EPSG:4326 if in -180..180 / -90..90).
fn fallback_crs_detection(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
) -> Result<u32, OrthoImportError> {
    // Jika koordinat tampak seperti derajat desimal geografis
    if x_min >= -180.0 && x_max <= 180.0 && y_min >= -90.0 && y_max <= 90.0 {
        Ok(4326)
    } else {
        Err(OrthoImportError::CrsTidakDidukung(
            "World file menggunakan koordinat terproyeksi tanpa berkas pendamping .prj. Silakan masukkan koordinat WGS84 secara manual."
                .to_string(),
        ))
    }
}
