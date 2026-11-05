//! Layanan mutasi semantik dan audit edit manual untuk Spatial Results.
//!
//! Menangani pembaruan field aman (nama layer, warna, icon, gambar, custom fields, edit fitur)
//! tanpa mengubah struktur koordinat GeoJSON orisinal.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::pure::{
    is_reserved_key, validate_custom_field_key, validate_custom_fields_count, validate_field_label,
    validate_field_unit, validate_hex_color, validate_name, validate_property_value_type,
    validate_spatial_result_id,
};
use super::reader::{
    compute_concurrency_token, get_saved_spatial_result_in_dir, read_decomposed_layer,
    read_flat_layer,
};
use super::types::{
    LayerDisplayMode, LayerPayload, ManualEditsCheckResult, UpdateSpatialResultPayload,
};
use super::writer::{cleanup_result_image_file, save_result_image};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename, write_atomic};

/// Memperbarui field aman hasil analitik spasial (layer_name, display_color, icon, image, fields, feature_edits)
/// secara semantik tanpa mendeserialisasi atau mengubah struktur koordinat dan fitur.
pub fn update_spatial_result_in_dir(
    storage_dir: &Path,
    payload: UpdateSpatialResultPayload,
) -> Result<LayerPayload, MapControllerError> {
    validate_spatial_result_id(&payload.id)?;

    if let Some(name) = &payload.name {
        validate_name(name)?;
    }
    if let Some(color) = &payload.display_color {
        validate_hex_color(color)?;
    }

    // Validasi skema fields jika disediakan
    if let Some(ref fields) = payload.fields {
        validate_custom_fields_count(fields)?;
        let mut seen_keys = HashSet::new();
        for field in fields {
            if !seen_keys.insert(&field.key) {
                return Err(MapControllerError::StorageError(format!(
                    "Key field duplikat: '{}'",
                    field.key
                )));
            }
            if field.custom {
                validate_custom_field_key(&field.key)?;
            }
            validate_field_label(&field.label)?;
            validate_field_unit(field.unit.as_deref())?;
        }
    }

    let root_dir = storage_dir.join("spatial_results");
    let safe_id = sanitize_filename(&payload.id);
    let now_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    // Resolusi aksi gambar jika ditentukan
    let mut resolved_new_image_url: Option<String> = None;
    let is_delete_image = match payload.image_action.as_deref() {
        Some("delete") => {
            cleanup_result_image_file(storage_dir, &payload.id);
            true
        }
        Some("replace") => {
            if let Some(src_path) = &payload.image_path {
                let web_url = save_result_image(storage_dir, &payload.id, src_path)?;
                resolved_new_image_url = Some(web_url);
            }
            false
        }
        _ => false,
    };

    // 1. Cek folder dekomposisi
    let folder_path = root_dir.join(&safe_id);
    let layer_file = folder_path.join("layer.json");
    if folder_path.is_dir() && layer_file.is_file() {
        let content = fs::read_to_string(&layer_file)?;
        let mut val: serde_json::Value = serde_json::from_str(&content)?;

        // Concurrency token check
        let current_token =
            compute_concurrency_token(&layer_file, val.get("updated_at").and_then(|v| v.as_str()));
        if let Some(ref expected) = payload.expected_updated_at {
            if expected != &current_token {
                return Err(MapControllerError::StorageError(
                    "Data hasil analisis telah diperbarui oleh proses lain. Silakan muat ulang data sebelum menyimpan.".to_string(),
                ));
            }
        }

        if let Some(name) = &payload.name {
            val["layer_name"] = serde_json::Value::String(name.trim().to_string());
        }
        if let Some(color) = &payload.display_color {
            if let Some(disp) = val.get_mut("display_preference") {
                disp["color"] = serde_json::Value::String(color.clone());
            }
        }
        if let Some(icon) = &payload.icon {
            if let Some(disp) = val.get_mut("display_preference") {
                let trimmed_icon = icon.trim();
                if trimmed_icon.is_empty() {
                    disp["icon"] = serde_json::Value::String("".to_string());
                } else {
                    disp["icon"] = serde_json::Value::String(trimmed_icon.to_string());
                }
            }
        }

        // Mutasi gambar level layer pada display_preference (TIDAK PERNAH menyentuh fitur)
        if is_delete_image {
            if let Some(disp) = val
                .get_mut("display_preference")
                .and_then(|d| d.as_object_mut())
            {
                if disp.get("type").and_then(|t| t.as_str()) == Some("image_overlay") {
                    disp.insert(
                        "image_url".to_string(),
                        serde_json::Value::String("".to_string()),
                    );
                } else {
                    disp.remove("image_url");
                }
            }
        } else if let Some(new_url) = &resolved_new_image_url {
            if let Some(disp) = val
                .get_mut("display_preference")
                .and_then(|d| d.as_object_mut())
            {
                disp.insert(
                    "image_url".to_string(),
                    serde_json::Value::String(new_url.clone()),
                );
            }
        }

        // Mutasi fields pada display_preference jika disediakan
        if let Some(ref fields) = payload.fields {
            if let Some(disp) = val
                .get_mut("display_preference")
                .and_then(|d| d.as_object_mut())
            {
                disp.insert("fields".to_string(), serde_json::to_value(fields)?);
            }
        }

        // Mutasi feature_edits pada berkas fitur individual
        let features_dir = folder_path.join("features");
        if let Some(ref edits) = payload.feature_edits {
            if !features_dir.is_dir() {
                return Err(MapControllerError::StorageError(
                    "Direktori features tidak ditemukan di format dekomposisi".to_string(),
                ));
            }
            let mut feat_entries = fs::read_dir(&features_dir)?.flatten().collect::<Vec<_>>();
            feat_entries.sort_by_key(|e| e.file_name());

            for edit in edits {
                if edit.index >= feat_entries.len() {
                    return Err(MapControllerError::StorageError(format!(
                        "Indeks fitur {} di luar batas (total: {})",
                        edit.index,
                        feat_entries.len()
                    )));
                }
                let f_path = feat_entries[edit.index].path();
                let f_content = fs::read_to_string(&f_path)?;

                let mut f_val: serde_json::Value = serde_json::from_str(&f_content)?;
                let res = f_val
                    .get_mut("result")
                    .and_then(|r| r.as_object_mut())
                    .ok_or_else(|| {
                        MapControllerError::StorageError(
                            "Format berkas fitur tidak valid".to_string(),
                        )
                    })?;

                // Apply set
                for (k, v) in &edit.set {
                    if is_reserved_key(k) {
                        return Err(MapControllerError::StorageError(format!(
                            "Kunci properti '{}' merupakan kata terlarang",
                            k
                        )));
                    }
                    let existing_val = res.get("properties").and_then(|p| p.get(k)).cloned();

                    let field_kind = payload
                        .fields
                        .as_ref()
                        .and_then(|f_list| f_list.iter().find(|f| &f.key == k))
                        .and_then(|f| f.kind.as_deref());
                    validate_property_value_type(existing_val.as_ref(), v, field_kind)?;

                    let props = res
                        .entry("properties")
                        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                        .as_object_mut()
                        .ok_or_else(|| {
                            MapControllerError::StorageError(
                                "Properties fitur bukan objek".to_string(),
                            )
                        })?;
                    props.insert(k.clone(), v.clone());

                    // Record edited_fields
                    let edited_arr = res
                        .entry("edited_fields")
                        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                    if let Some(arr) = edited_arr.as_array_mut() {
                        let str_val = serde_json::Value::String(k.clone());
                        if !arr.contains(&str_val) {
                            arr.push(str_val);
                        }
                    }
                }

                // Apply remove (only allowed for custom fields)
                for k in &edit.remove {
                    let is_custom = payload
                        .fields
                        .as_ref()
                        .and_then(|f_list| f_list.iter().find(|f| &f.key == k))
                        .map(|f| f.custom)
                        .unwrap_or(false);
                    if !is_custom {
                        return Err(MapControllerError::StorageError(format!(
                            "Field '{}' bawaan plugin tidak dapat dihapus, hanya dapat disembunyikan",
                            k
                        )));
                    }
                    if let Some(props) = res.get_mut("properties").and_then(|p| p.as_object_mut()) {
                        props.remove(k);
                    }
                    if let Some(arr) = res.get_mut("edited_fields").and_then(|a| a.as_array_mut()) {
                        arr.retain(|x| x.as_str() != Some(k.as_str()));
                    }
                    let should_remove = res
                        .get("edited_fields")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(false);
                    if should_remove {
                        res.remove("edited_fields");
                    }
                }

                write_atomic(&f_path, &serde_json::to_string_pretty(&f_val)?)?;
            }
        }

        val["updated_at"] = serde_json::Value::String(now_utc.clone());
        write_atomic(&layer_file, &serde_json::to_string_pretty(&val)?)?;

        // Sinkronkan layer registration di layers/{id}.json jika ada
        let layer_reg = storage_dir.join("layers").join(format!("{}.json", safe_id));
        if layer_reg.is_file() {
            if let Ok(reg_content) = fs::read_to_string(&layer_reg) {
                if let Ok(mut reg_val) = serde_json::from_str::<serde_json::Value>(&reg_content) {
                    if let Some(name) = &payload.name {
                        reg_val["layer_name"] = serde_json::Value::String(name.trim().to_string());
                    }
                    let _ = write_atomic(&layer_reg, &serde_json::to_string_pretty(&reg_val)?);
                }
            }
        }

        return read_decomposed_layer(&folder_path, &payload.id);
    }

    // 2. Cek flat file JSON
    let flat_path = root_dir.join(format!("{}.json", safe_id));
    if flat_path.is_file() {
        let content = fs::read_to_string(&flat_path)?;
        let mut val: serde_json::Value = serde_json::from_str(&content)?;

        // Concurrency token check
        let current_token =
            compute_concurrency_token(&flat_path, val.get("updated_at").and_then(|v| v.as_str()));
        if let Some(ref expected) = payload.expected_updated_at {
            if expected != &current_token {
                return Err(MapControllerError::StorageError(
                    "Data hasil analisis telah diperbarui oleh proses lain. Silakan muat ulang data sebelum menyimpan.".to_string(),
                ));
            }
        }

        if let Some(name) = &payload.name {
            val["layer_name"] = serde_json::Value::String(name.trim().to_string());
        }
        if let Some(color) = &payload.display_color {
            if let Some(disp) = val.get_mut("display_preference") {
                disp["color"] = serde_json::Value::String(color.clone());
            }
        }
        if let Some(icon) = &payload.icon {
            if let Some(disp) = val.get_mut("display_preference") {
                let trimmed_icon = icon.trim();
                if trimmed_icon.is_empty() {
                    disp["icon"] = serde_json::Value::String("".to_string());
                } else {
                    disp["icon"] = serde_json::Value::String(trimmed_icon.to_string());
                }
            }
        }

        // Mutasi gambar level layer pada display_preference (TIDAK PERNAH menyentuh fitur)
        if is_delete_image {
            if let Some(disp) = val
                .get_mut("display_preference")
                .and_then(|d| d.as_object_mut())
            {
                if disp.get("type").and_then(|t| t.as_str()) == Some("image_overlay") {
                    disp.insert(
                        "image_url".to_string(),
                        serde_json::Value::String("".to_string()),
                    );
                } else {
                    disp.remove("image_url");
                }
            }
        } else if let Some(new_url) = &resolved_new_image_url {
            if let Some(disp) = val
                .get_mut("display_preference")
                .and_then(|d| d.as_object_mut())
            {
                disp.insert(
                    "image_url".to_string(),
                    serde_json::Value::String(new_url.clone()),
                );
            }
        }

        // Mutasi fields pada display_preference jika disediakan
        if let Some(ref fields) = payload.fields {
            if let Some(disp) = val
                .get_mut("display_preference")
                .and_then(|d| d.as_object_mut())
            {
                disp.insert("fields".to_string(), serde_json::to_value(fields)?);
            }
        }

        // Mutasi feature_edits pada spatial_results
        if let Some(ref edits) = payload.feature_edits {
            let feats = val
                .get_mut("spatial_results")
                .and_then(|f| f.as_array_mut())
                .ok_or_else(|| {
                    MapControllerError::StorageError("spatial_results bukan array".to_string())
                })?;

            for edit in edits {
                if edit.index >= feats.len() {
                    return Err(MapControllerError::StorageError(format!(
                        "Indeks fitur {} di luar batas (total: {})",
                        edit.index,
                        feats.len()
                    )));
                }
                let feat_obj = feats[edit.index].as_object_mut().ok_or_else(|| {
                    MapControllerError::StorageError("Fitur bukan objek JSON".to_string())
                })?;

                // Apply set
                for (k, v) in &edit.set {
                    if is_reserved_key(k) {
                        return Err(MapControllerError::StorageError(format!(
                            "Kunci properti '{}' merupakan kata terlarang",
                            k
                        )));
                    }
                    let existing_val = feat_obj.get("properties").and_then(|p| p.get(k)).cloned();

                    let field_kind = payload
                        .fields
                        .as_ref()
                        .and_then(|f_list| f_list.iter().find(|f| &f.key == k))
                        .and_then(|f| f.kind.as_deref());
                    validate_property_value_type(existing_val.as_ref(), v, field_kind)?;

                    let props = feat_obj
                        .entry("properties")
                        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                        .as_object_mut()
                        .ok_or_else(|| {
                            MapControllerError::StorageError(
                                "Properties fitur bukan objek".to_string(),
                            )
                        })?;
                    props.insert(k.clone(), v.clone());

                    // Record edited_fields
                    let edited_arr = feat_obj
                        .entry("edited_fields")
                        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                    if let Some(arr) = edited_arr.as_array_mut() {
                        let str_val = serde_json::Value::String(k.clone());
                        if !arr.contains(&str_val) {
                            arr.push(str_val);
                        }
                    }
                }

                // Apply remove (only allowed for custom fields)
                for k in &edit.remove {
                    let is_custom = payload
                        .fields
                        .as_ref()
                        .and_then(|f_list| f_list.iter().find(|f| &f.key == k))
                        .map(|f| f.custom)
                        .unwrap_or(false);
                    if !is_custom {
                        return Err(MapControllerError::StorageError(format!(
                            "Field '{}' bawaan plugin tidak dapat dihapus, hanya dapat disembunyikan",
                            k
                        )));
                    }
                    if let Some(props) = feat_obj
                        .get_mut("properties")
                        .and_then(|p| p.as_object_mut())
                    {
                        props.remove(k);
                    }
                    if let Some(arr) = feat_obj
                        .get_mut("edited_fields")
                        .and_then(|a| a.as_array_mut())
                    {
                        arr.retain(|x| x.as_str() != Some(k.as_str()));
                    }
                    let should_remove = feat_obj
                        .get("edited_fields")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(false);
                    if should_remove {
                        feat_obj.remove("edited_fields");
                    }
                }
            }
        }

        val["updated_at"] = serde_json::Value::String(now_utc);
        write_atomic(&flat_path, &serde_json::to_string_pretty(&val)?)?;

        // Sinkronkan layer registration di layers/{id}.json jika ada
        let layer_reg = storage_dir.join("layers").join(format!("{}.json", safe_id));
        if layer_reg.is_file() {
            if let Ok(reg_content) = fs::read_to_string(&layer_reg) {
                if let Ok(mut reg_val) = serde_json::from_str::<serde_json::Value>(&reg_content) {
                    if let Some(name) = &payload.name {
                        reg_val["layer_name"] = serde_json::Value::String(name.trim().to_string());
                    }
                    let _ = write_atomic(&layer_reg, &serde_json::to_string_pretty(&reg_val)?);
                }
            }
        }

        return read_flat_layer(&flat_path, &payload.id);
    }

    Err(MapControllerError::NotFound(format!(
        "Spatial result dengan ID '{}' tidak ditemukan di storage",
        payload.id
    )))
}

/// Memperbarui field aman Spatial Result (pembungkus publik ke storage default).
pub fn update_saved_spatial_result(
    payload: UpdateSpatialResultPayload,
) -> Result<LayerPayload, MapControllerError> {
    update_spatial_result_in_dir(&get_storage_root(), payload)
}

/// Memeriksa keberadaan edit manual pada layer spasial (command baca-saja).
pub fn check_spatial_result_manual_edits_in_dir(
    storage_dir: &Path,
    layer_id: &str,
) -> Result<ManualEditsCheckResult, MapControllerError> {
    validate_spatial_result_id(layer_id)?;
    let layer_res = get_saved_spatial_result_in_dir(storage_dir, layer_id);
    match layer_res {
        Ok(layer) => {
            let custom_fields_count = match &layer.display_preference {
                LayerDisplayMode::Polygon { fields, .. }
                | LayerDisplayMode::Point { fields, .. }
                | LayerDisplayMode::Line { fields, .. }
                | LayerDisplayMode::ImageOverlay { fields, .. } => fields
                    .as_ref()
                    .map(|f| f.iter().filter(|x| x.custom).count())
                    .unwrap_or(0),
            };
            let edited_features_count = layer
                .spatial_results
                .iter()
                .filter(|f| {
                    f.edited_fields
                        .as_ref()
                        .map(|e| !e.is_empty())
                        .unwrap_or(false)
                })
                .count();
            let has_manual_edits = custom_fields_count > 0 || edited_features_count > 0;
            Ok(ManualEditsCheckResult {
                layer_id: layer_id.to_string(),
                exists: true,
                has_manual_edits,
                edited_features_count,
                custom_fields_count,
            })
        }
        Err(MapControllerError::NotFound(_)) => Ok(ManualEditsCheckResult {
            layer_id: layer_id.to_string(),
            exists: false,
            has_manual_edits: false,
            edited_features_count: 0,
            custom_fields_count: 0,
        }),
        Err(e) => Err(e),
    }
}

/// Memeriksa keberadaan edit manual pada layer spasial (pembungkus publik ke storage default).
pub fn check_spatial_result_manual_edits(
    layer_id: &str,
) -> Result<ManualEditsCheckResult, MapControllerError> {
    check_spatial_result_manual_edits_in_dir(&get_storage_root(), layer_id)
}
