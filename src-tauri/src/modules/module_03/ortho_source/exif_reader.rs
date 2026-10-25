//! Pembaca metadata EXIF kamera drone murni Rust via `kamadak-exif`.
//! Mengekstrak make, model, timestamp, dan satu titik GPS posisi kamera (bukan bounding box).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{Exif, In, Reader, Tag, Value};

use super::types::ExifSummary;
use crate::modules::module_03::error::OrthoImportError;

/// Membaca metadata EXIF dari berkas citra JPEG atau TIFF.
/// Mengembalikan `None` secara aman jika file tidak memuat data EXIF tanpa menggagalkan impor.
pub fn read_exif_metadata(image_path: &Path) -> Result<Option<ExifSummary>, OrthoImportError> {
    let file = match File::open(image_path) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };

    let mut buf_reader = BufReader::new(file);
    let exif: Exif = match Reader::new().read_from_container(&mut buf_reader) {
        Ok(exif) => exif,
        Err(_) => return Ok(None),
    };

    let mut make = None;
    let mut model = None;
    let mut captured_at = None;

    if let Some(field) = exif.get_field(Tag::Make, In::PRIMARY) {
        make = Some(field.display_value().to_string().trim().replace('"', ""));
    }
    if let Some(field) = exif.get_field(Tag::Model, In::PRIMARY) {
        model = Some(field.display_value().to_string().trim().replace('"', ""));
    }
    if let Some(field) = exif
        .get_field(Tag::DateTimeOriginal, In::PRIMARY)
        .or_else(|| exif.get_field(Tag::DateTime, In::PRIMARY))
    {
        captured_at = Some(field.display_value().to_string().trim().replace('"', ""));
    }

    // Ekstraksi GPS (titik tunggal kamera drone)
    let mut lat = None;
    let mut lon = None;
    let mut alt = None;

    if let Some(lat_field) = exif.get_field(Tag::GPSLatitude, In::PRIMARY) {
        if let Some(lat_ref_field) = exif.get_field(Tag::GPSLatitudeRef, In::PRIMARY) {
            let lat_ref = lat_ref_field.display_value().to_string();
            if let Some(deg) = parse_dms_to_degrees(&lat_field.value) {
                lat = Some(if lat_ref.contains('S') { -deg } else { deg });
            }
        }
    }

    if let Some(lon_field) = exif.get_field(Tag::GPSLongitude, In::PRIMARY) {
        if let Some(lon_ref_field) = exif.get_field(Tag::GPSLongitudeRef, In::PRIMARY) {
            let lon_ref = lon_ref_field.display_value().to_string();
            if let Some(deg) = parse_dms_to_degrees(&lon_field.value) {
                lon = Some(if lon_ref.contains('W') { -deg } else { deg });
            }
        }
    }

    if let Some(alt_field) = exif.get_field(Tag::GPSAltitude, In::PRIMARY) {
        if let Value::Rational(ref rationals) = alt_field.value {
            if let Some(r) = rationals.first() {
                if r.denom != 0 {
                    alt = Some(r.num as f64 / r.denom as f64);
                }
            }
        }
    }

    if make.is_none() && model.is_none() && captured_at.is_none() && lat.is_none() && lon.is_none()
    {
        return Ok(None);
    }

    Ok(Some(ExifSummary {
        camera_make: make,
        camera_model: model,
        captured_at,
        gps_latitude: lat,
        gps_longitude: lon,
        gps_altitude_agl_m: alt,
    }))
}

/// Mengonversi nilai derajat-menit-detik (DMS) dari EXIF Rational ke derajat desimal.
fn parse_dms_to_degrees(value: &Value) -> Option<f64> {
    if let Value::Rational(ref rationals) = *value {
        if rationals.len() >= 3 {
            let d = rationals[0].num as f64 / rationals[0].denom.max(1) as f64;
            let m = rationals[1].num as f64 / rationals[1].denom.max(1) as f64;
            let s = rationals[2].num as f64 / rationals[2].denom.max(1) as f64;
            return Some(d + (m / 60.0) + (s / 3600.0));
        }
    }
    None
}
