//! Fungsi murni untuk validasi dan komputasi hasil pengukuran spasial Module 3.

use super::types::{FieldDef, LayerPayload, SpatialGeometry};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::geometry::{
    calculate_area, calculate_distance, calculate_perimeter,
};

/// Memvalidasi seluruh payload hasil spasial analitik secara murni (kompatibilitas plugin).
pub fn validate_spatial_payload(payload: &LayerPayload) -> Result<(), MapControllerError> {
    payload.validate()
}

/// Memvalidasi format ID spatial result menggunakan allowlist regex `^[A-Za-z0-9_-]{1,64}$`.
/// Mencegah path traversal dan karakter berbahaya.
pub fn validate_spatial_result_id(id: &str) -> Result<(), MapControllerError> {
    if id.is_empty() {
        return Err(MapControllerError::StorageError(
            "ID spatial result tidak boleh kosong".to_string(),
        ));
    }

    if id.len() > 64 {
        return Err(MapControllerError::StorageError(
            "ID spatial result maksimal 64 karakter".to_string(),
        ));
    }

    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(MapControllerError::StorageError(format!(
            "ID '{}' memuat karakter terlarang / traversal",
            id
        )));
    }

    let is_valid = id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !is_valid {
        return Err(MapControllerError::StorageError(format!(
            "Format ID '{}' tidak valid. Hanya diizinkan alfanumerik, underscore (_), dan strip (-).",
            id
        )));
    }

    Ok(())
}

/// Memvalidasi nama hasil pengukuran (1..100 karakter, bukan hanya whitespace).
pub fn validate_name(name: &str) -> Result<(), MapControllerError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(MapControllerError::StorageError(
            "Nama hasil pengukuran tidak boleh kosong atau hanya berisi spasi".to_string(),
        ));
    }
    if trimmed.chars().count() > 100 {
        return Err(MapControllerError::StorageError(
            "Nama hasil pengukuran maksimal 100 karakter".to_string(),
        ));
    }
    Ok(())
}

/// Memvalidasi catatan hasil pengukuran (maksimal 500 karakter).
pub fn validate_notes(notes: Option<&str>) -> Result<(), MapControllerError> {
    if let Some(n) = notes {
        if n.chars().count() > 500 {
            return Err(MapControllerError::StorageError(
                "Catatan hasil pengukuran maksimal 500 karakter".to_string(),
            ));
        }
    }
    Ok(())
}

/// Memvalidasi format warna hex (`^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$`).
pub fn validate_hex_color(color: &str) -> Result<(), MapControllerError> {
    let is_valid = if let Some(hex) = color.strip_prefix('#') {
        (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
    } else {
        false
    };

    if is_valid {
        Ok(())
    } else {
        Err(MapControllerError::StorageError(format!(
            "Format warna '{}' tidak valid. Harus format HEX (#RGB atau #RRGGBB).",
            color
        )))
    }
}

/// Memvalidasi magic bytes untuk format foto yang didukung (JPEG, PNG, WebP).
/// Mengembalikan tuple `(content_type, file_extension)` jika valid.
pub fn validate_photo_magic_bytes(
    bytes: &[u8],
) -> Result<(&'static str, &'static str), MapControllerError> {
    // 1. JPEG: 0xFF, 0xD8, 0xFF
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Ok(("image/jpeg", "jpg"));
    }

    // 2. PNG: 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A
    if bytes.len() >= 8 && bytes[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Ok(("image/png", "png"));
    }

    // 3. WebP: RIFF....WEBP
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok(("image/webp", "webp"));
    }

    Err(MapControllerError::StorageError(
        "Format berkas tidak didukung atau berkas korup. Hanya JPG, PNG, dan WebP yang diizinkan."
            .to_string(),
    ))
}

/// Menghitung ulang nilai terverifikasi dari geometri secara deterministik dan murni.
pub fn calculate_verified_value(
    measurement_type: &str,
    geometry: &SpatialGeometry,
) -> Result<(f64, String), MapControllerError> {
    match measurement_type {
        "area" => {
            let coords = match geometry {
                SpatialGeometry::Polygon { coordinates } => coordinates,
                _ => {
                    return Err(MapControllerError::InvalidGeometry(
                        "Tipe pengukuran 'area' memerlukan geometri tipe Polygon".to_string(),
                    ));
                }
            };

            let poly: Vec<Vec<[f64; 2]>> = coords
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|p| {
                            if p.len() >= 2 {
                                [p[0], p[1]]
                            } else {
                                [0.0, 0.0]
                            }
                        })
                        .collect()
                })
                .collect();

            let area_sq_m = calculate_area(&poly);
            Ok((area_sq_m, "m²".to_string()))
        }
        "perimeter" => {
            let coords = match geometry {
                SpatialGeometry::Polygon { coordinates } => coordinates,
                _ => {
                    return Err(MapControllerError::InvalidGeometry(
                        "Tipe pengukuran 'perimeter' memerlukan geometri tipe Polygon".to_string(),
                    ));
                }
            };

            let poly: Vec<Vec<[f64; 2]>> = coords
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|p| {
                            if p.len() >= 2 {
                                [p[0], p[1]]
                            } else {
                                [0.0, 0.0]
                            }
                        })
                        .collect()
                })
                .collect();

            let perimeter_m = calculate_perimeter(&poly);
            Ok((perimeter_m, "m".to_string()))
        }
        "distance" => {
            let coords = match geometry {
                SpatialGeometry::LineString { coordinates } => coordinates,
                _ => {
                    return Err(MapControllerError::InvalidGeometry(
                        "Tipe pengukuran 'distance' memerlukan geometri tipe LineString"
                            .to_string(),
                    ));
                }
            };

            if coords.len() < 2 {
                return Err(MapControllerError::InvalidGeometry(
                    "Geometri LineString untuk pengukuran jarak memerlukan minimal 2 koordinat"
                        .to_string(),
                ));
            }

            let mut total_distance_m = 0.0;
            for i in 0..(coords.len() - 1) {
                let p1 = [coords[i][0], coords[i][1]];
                let p2 = [coords[i + 1][0], coords[i + 1][1]];
                total_distance_m += calculate_distance(p1, p2);
            }

            Ok((total_distance_m, "m".to_string()))
        }
        other => Err(MapControllerError::StorageError(format!(
            "Tipe pengukuran '{}' tidak dikenal. Pilihan: 'area', 'perimeter', 'distance'.",
            other
        ))),
    }
}

/// Daftar kata kunci yang dilarang untuk dijadikan nama custom field
/// dan tidak boleh ditampilkan di popup.
pub const RESERVED_KEYS: &[&str] = &[
    "image_url",
    "photo_url",
    "thumbnail_url",
    "image",
    "edited_fields",
    "source",
    "geometry",
    "properties",
    "__proto__",
    "constructor",
    "prototype",
];

/// Memeriksa apakah suatu string merupakan reserved key.
pub fn is_reserved_key(key: &str) -> bool {
    RESERVED_KEYS.contains(&key)
}

/// Memeriksa apakah suatu berkas gambar di `images/result_image/` merupakan
/// milik layer tertentu sesuai regex: `^{layer_id}_\d{10,}\.(jpg|jpeg|png|webp)$`.
pub fn is_owned_result_photo(layer_id: &str, filename: &str) -> bool {
    let prefix = format!("{}_", layer_id);
    if !filename.starts_with(&prefix) {
        return false;
    }
    let rest = &filename[prefix.len()..];
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 2 {
        return false;
    }
    let timestamp_str = parts[0];
    let ext_str = parts[1];

    if timestamp_str.len() < 10 || !timestamp_str.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    let ext_lower = ext_str.to_ascii_lowercase();
    matches!(ext_lower.as_str(), "jpg" | "jpeg" | "png" | "webp")
}

/// Memvalidasi format key field custom baru: regex `^[a-z][a-z0-9_]{0,39}$`.
/// Catatan: Key bawaan plugin tidak divalidasi dengan fungsi ini.
pub fn validate_custom_field_key(key: &str) -> Result<(), MapControllerError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(MapControllerError::StorageError(
            "Key field custom tidak boleh kosong".to_string(),
        ));
    }
    if trimmed.len() > 40 {
        return Err(MapControllerError::StorageError(
            "Key field custom maksimal 40 karakter".to_string(),
        ));
    }
    if is_reserved_key(trimmed) {
        return Err(MapControllerError::StorageError(format!(
            "Key '{}' merupakan kata terlarang (reserved key)",
            trimmed
        )));
    }
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return Err(MapControllerError::StorageError(format!(
            "Key field custom '{}' harus diawali dengan huruf kecil (a-z)",
            trimmed
        )));
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(MapControllerError::StorageError(format!(
            "Key field custom '{}' hanya boleh memuat huruf kecil, angka, dan garis bawah (_)",
            trimmed
        )));
    }
    Ok(())
}

/// Memvalidasi label field (1..40 karakter setelah di-trim).
pub fn validate_field_label(label: &str) -> Result<(), MapControllerError> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(MapControllerError::StorageError(
            "Label field tidak boleh kosong atau hanya berisi spasi".to_string(),
        ));
    }
    if trimmed.chars().count() > 40 {
        return Err(MapControllerError::StorageError(
            "Label field maksimal 40 karakter".to_string(),
        ));
    }
    Ok(())
}

/// Memvalidasi unit satuan field (0..12 karakter setelah di-trim).
pub fn validate_field_unit(unit: Option<&str>) -> Result<(), MapControllerError> {
    if let Some(u) = unit {
        let trimmed = u.trim();
        if trimmed.chars().count() > 12 {
            return Err(MapControllerError::StorageError(
                "Unit field maksimal 12 karakter".to_string(),
            ));
        }
    }
    Ok(())
}

/// Memvalidasi batas maksimal field custom (maksimal 30 field dengan custom == true).
pub fn validate_custom_fields_count(fields: &[FieldDef]) -> Result<(), MapControllerError> {
    let custom_count = fields.iter().filter(|f| f.custom).count();
    if custom_count > 30 {
        return Err(MapControllerError::StorageError(format!(
            "Jumlah field custom melebihi batas maksimal 30 (saat ini: {})",
            custom_count
        )));
    }
    Ok(())
}

/// Memvalidasi preservasi tipe nilai JSON saat pengeditan properti.
pub fn validate_property_value_type(
    existing_val: Option<&serde_json::Value>,
    new_val: &serde_json::Value,
    kind: Option<&str>,
) -> Result<(), MapControllerError> {
    if let Some(existing) = existing_val {
        match existing {
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                Err(MapControllerError::StorageError(
                    "Struktur data kompleks (objek/array) tidak dapat diedit manual".to_string(),
                ))
            }
            serde_json::Value::Number(_) => {
                if !new_val.is_number() {
                    Err(MapControllerError::StorageError(
                        "Nilai harus bertipe angka (Number) sesuai tipe properti aslinya"
                            .to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            serde_json::Value::Bool(_) => {
                if !new_val.is_boolean() {
                    Err(MapControllerError::StorageError(
                        "Nilai harus bertipe boolean sesuai tipe properti aslinya".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            serde_json::Value::String(_) => {
                if let Some(s) = new_val.as_str() {
                    if s.chars().count() > 200 {
                        Err(MapControllerError::StorageError(
                            "Nilai teks maksimal 200 karakter".to_string(),
                        ))
                    } else {
                        Ok(())
                    }
                } else {
                    Err(MapControllerError::StorageError(
                        "Nilai harus bertipe string sesuai tipe properti aslinya".to_string(),
                    ))
                }
            }
            serde_json::Value::Null => {
                if let Some(s) = new_val.as_str() {
                    if s.chars().count() > 200 {
                        Err(MapControllerError::StorageError(
                            "Nilai teks maksimal 200 karakter".to_string(),
                        ))
                    } else {
                        Ok(())
                    }
                } else if !new_val.is_number() && !new_val.is_boolean() && !new_val.is_null() {
                    Err(MapControllerError::StorageError(
                        "Nilai hanya boleh bertipe teks, angka, atau boolean".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    } else {
        match kind {
            Some("number") => {
                if !new_val.is_number() {
                    Err(MapControllerError::StorageError(
                        "Nilai harus bertipe angka (Number) untuk field bertipe number".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            Some("boolean") => {
                if !new_val.is_boolean() {
                    Err(MapControllerError::StorageError(
                        "Nilai harus bertipe boolean untuk field bertipe boolean".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            Some("text") | None => {
                if let Some(s) = new_val.as_str() {
                    if s.chars().count() > 200 {
                        Err(MapControllerError::StorageError(
                            "Nilai teks maksimal 200 karakter".to_string(),
                        ))
                    } else {
                        Ok(())
                    }
                } else if !new_val.is_null() {
                    Err(MapControllerError::StorageError(
                        "Nilai harus bertipe teks untuk field bertipe text".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            Some(other) => Err(MapControllerError::StorageError(format!(
                "Tipe kind '{}' tidak dikenal",
                other
            ))),
        }
    }
}
