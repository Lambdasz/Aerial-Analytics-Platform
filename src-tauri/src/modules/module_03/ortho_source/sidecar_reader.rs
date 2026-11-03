//! Pembaca berkas metadata Sidecar JSON (.json atau _meta.json) untuk citra orthomosaic.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::modules::module_03::error::OrthoImportError;

#[derive(Debug, Deserialize)]
struct GenericSidecarMeta {
    #[serde(default)]
    bounds: Option<serde_json::Value>,
    #[serde(default)]
    envelope: Option<serde_json::Value>,
    #[serde(default)]
    coordinates: Option<serde_json::Value>,
    #[serde(default)]
    west: Option<f64>,
    #[serde(default)]
    south: Option<f64>,
    #[serde(default)]
    east: Option<f64>,
    #[serde(default)]
    north: Option<f64>,
}

/// Mencari kandidat berkas Sidecar JSON untuk citra sumber.
/// Contoh: `ortho_01.jpg` -> `ortho_01.json`, `ortho_01_meta.json`
pub fn find_candidate_sidecar_file(image_path: &Path) -> Option<PathBuf> {
    let parent = image_path.parent()?;
    let stem = image_path.file_stem()?.to_str()?;

    let c1 = parent.join(format!("{}.json", stem));
    if c1.is_file() {
        return Some(c1);
    }

    let c2 = parent.join(format!("{}_meta.json", stem));
    if c2.is_file() {
        return Some(c2);
    }

    None
}

/// Membaca bounding box WGS84 dari berkas Sidecar JSON.
pub fn read_sidecar_envelope(sidecar_path: &Path) -> Result<[[f64; 2]; 2], OrthoImportError> {
    let content = fs::read_to_string(sidecar_path).map_err(|e| {
        OrthoImportError::FileKorup(format!("Gagal membaca berkas Sidecar JSON: {}", e))
    })?;

    let meta: GenericSidecarMeta = serde_json::from_str(&content).map_err(|e| {
        OrthoImportError::FileKorup(format!("Format berkas Sidecar JSON tidak valid: {}", e))
    })?;

    // Format 1: field eksplisit west, south, east, north
    if let (Some(w), Some(s), Some(e), Some(n)) = (meta.west, meta.south, meta.east, meta.north) {
        return validate_envelope(s, w, n, e);
    }

    // Format 2: bounds berupa array [[south, west], [north, east]]
    if let Some(val) = meta.bounds.or(meta.envelope).or(meta.coordinates) {
        if let Some(arr) = val.as_array() {
            if arr.len() == 2 {
                if let (Some(c1), Some(c2)) = (arr[0].as_array(), arr[1].as_array()) {
                    if c1.len() == 2 && c2.len() == 2 {
                        let s = c1[0].as_f64().unwrap_or(f64::NAN);
                        let w = c1[1].as_f64().unwrap_or(f64::NAN);
                        let n = c2[0].as_f64().unwrap_or(f64::NAN);
                        let e = c2[1].as_f64().unwrap_or(f64::NAN);
                        return validate_envelope(s, w, n, e);
                    }
                }
            } else if arr.len() == 4 {
                // Format [west, south, east, north] (GeoJSON standard BBox)
                let w = arr[0].as_f64().unwrap_or(f64::NAN);
                let s = arr[1].as_f64().unwrap_or(f64::NAN);
                let e = arr[2].as_f64().unwrap_or(f64::NAN);
                let n = arr[3].as_f64().unwrap_or(f64::NAN);
                return validate_envelope(s, w, n, e);
            }
        } else if let Some(obj) = val.as_object() {
            let s = obj.get("south").and_then(|v| v.as_f64());
            let w = obj.get("west").and_then(|v| v.as_f64());
            let n = obj.get("north").and_then(|v| v.as_f64());
            let e = obj.get("east").and_then(|v| v.as_f64());
            if let (Some(s), Some(w), Some(n), Some(e)) = (s, w, n, e) {
                return validate_envelope(s, w, n, e);
            }
        }
    }

    Err(OrthoImportError::KoordinatTidakValid(
        "Sidecar JSON tidak memuat format bounding box WGS84 yang dapat dikenali".to_string(),
    ))
}

/// Validates that bounding box coordinates are within WGS84 ranges and form a non-degenerate envelope.
fn validate_envelope(s: f64, w: f64, n: f64, e: f64) -> Result<[[f64; 2]; 2], OrthoImportError> {
    if s.is_nan() || w.is_nan() || n.is_nan() || e.is_nan() {
        return Err(OrthoImportError::KoordinatTidakValid(
            "Nilai koordinat pada Sidecar JSON memuat NaN".to_string(),
        ));
    }
    if !(-90.0..=90.0).contains(&s) || !(-90.0..=90.0).contains(&n) {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Nilai latitude di luar rentang [-90, 90]: south={}, north={}",
            s, n
        )));
    }
    if !(-180.0..=180.0).contains(&w) || !(-180.0..=180.0).contains(&e) {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Nilai longitude di luar rentang [-180, 180]: west={}, east={}",
            w, e
        )));
    }
    if s >= n {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "South ({}) harus lebih kecil dari North ({})",
            s, n
        )));
    }
    if w >= e {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "West ({}) harus lebih kecil dari East ({})",
            w, e
        )));
    }

    Ok([[s, w], [n, e]])
}
