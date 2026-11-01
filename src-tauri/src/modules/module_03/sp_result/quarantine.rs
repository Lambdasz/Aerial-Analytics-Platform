//! Layanan penghapusan transaksional 2-fase, karantina sementara, dan rollback untuk Spatial Results.
//!
//! Menangani:
//! 1. Karantina file layer, folder dekomposisi, dan aset foto ke `.trash_sr_{id}_{timestamp}`
//! 2. Rollback atomik jika pelepasan registrasi layer di `layers/{id}.json` gagal
//! 3. Pembersihan residu karantina lama atau pemulihan berkas saat startup

use std::fs;
use std::path::Path;

use super::pure::{is_owned_result_photo, validate_spatial_result_id};
use super::reader::read_flat_layer;
use super::types::DeleteSpatialResultSummary;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename};

/// Membersihkan folder sisa karantina `.trash_sr_*` di direktori `spatial_results/` saat startup.
/// Jika registrasi layer masih ada di `layers/{safe_id}.json`, pulihkan berkas dari karantina.
/// Jika registrasi sudah tidak ada, hapus permanen folder karantina.
pub fn cleanup_orphaned_trash(storage_dir: &Path) {
    let sr_dir = storage_dir.join("spatial_results");
    if !sr_dir.is_dir() {
        return;
    }

    if let Ok(entries) = fs::read_dir(&sr_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(after_prefix) = name.strip_prefix(".trash_sr_") {
                    let safe_id = if let Some(last_underscore) = after_prefix.rfind('_') {
                        &after_prefix[..last_underscore]
                    } else {
                        after_prefix
                    };

                    let layer_reg = storage_dir.join("layers").join(format!("{}.json", safe_id));
                    if layer_reg.is_file() {
                        // Registrasi layer masih ada: PULIHKAN dari karantina
                        let trash_folder = path.join("decomposed_folder");
                        let trash_flat = path.join("flat.json");
                        let trash_photos = path.join("photos");

                        if trash_folder.is_dir() {
                            let target = sr_dir.join(safe_id);
                            let _ = fs::rename(&trash_folder, &target);
                        }
                        if trash_flat.is_file() {
                            let target = sr_dir.join(format!("{}.json", safe_id));
                            let _ = fs::rename(&trash_flat, &target);
                        } else if path.is_file() {
                            let target = sr_dir.join(format!("{}.json", safe_id));
                            let _ = fs::rename(&path, &target);
                        } else if path.is_dir() {
                            if let Ok(entries) = fs::read_dir(&path) {
                                for e in entries.flatten() {
                                    let ep = e.path();
                                    if ep.is_file()
                                        && ep.extension().and_then(|s| s.to_str()) == Some("json")
                                    {
                                        let fname =
                                            ep.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                        if fname != "layer_reg.json" {
                                            let target = sr_dir.join(format!("{}.json", safe_id));
                                            let _ = fs::rename(&ep, &target);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if trash_photos.is_dir() {
                            let img_dir = storage_dir.join("images").join("result_image");
                            let _ = fs::create_dir_all(&img_dir);
                            if let Ok(photo_entries) = fs::read_dir(&trash_photos) {
                                for p_entry in photo_entries.flatten() {
                                    let p_path = p_entry.path();
                                    if let Some(p_name) = p_path.file_name() {
                                        let _ = fs::rename(&p_path, img_dir.join(p_name));
                                    }
                                }
                            }
                        }
                        let _ = fs::remove_dir_all(&path);
                    } else {
                        // Registrasi sudah terhapus: Hapus permanen karantina
                        if path.is_dir() {
                            let _ = fs::remove_dir_all(&path);
                        } else if path.is_file() {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }
}

/// Menghitung dependensi layer spasial yang akan terdampak sebelum penghapusan.
pub fn count_spatial_result_dependents_in_dir(
    storage_dir: &Path,
    layer_id: &str,
) -> Result<DeleteSpatialResultSummary, MapControllerError> {
    validate_spatial_result_id(layer_id)?;

    let root_dir = storage_dir.join("spatial_results");
    let safe_id = sanitize_filename(layer_id);

    let folder_path = root_dir.join(&safe_id);
    let flat_path = root_dir.join(format!("{}.json", safe_id));

    let mut exists = false;
    let mut feature_count = 0;

    if folder_path.is_dir() {
        exists = true;
        let feat_dir = folder_path.join("features");
        if feat_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(feat_dir) {
                feature_count = entries
                    .flatten()
                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
                    .count();
            }
        }
    } else if flat_path.is_file() {
        exists = true;
        if let Ok(layer) = read_flat_layer(&flat_path, layer_id) {
            feature_count = layer.spatial_results.len();
        }
    }

    if !exists {
        return Err(MapControllerError::NotFound(format!(
            "Spatial result dengan ID '{}' tidak ditemukan di storage",
            layer_id
        )));
    }

    let layer_reg = storage_dir.join("layers").join(format!("{}.json", safe_id));
    let layers_removed = if layer_reg.is_file() { 1 } else { 0 };

    let mut photos_count = 0;
    let result_img_dir = storage_dir.join("images").join("result_image");
    if result_img_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&result_img_dir) {
            for entry in entries.flatten() {
                let p_path = entry.path();
                if p_path.is_file() {
                    if let Some(filename) = p_path.file_name().and_then(|n| n.to_str()) {
                        if is_owned_result_photo(&safe_id, filename) {
                            photos_count += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(DeleteSpatialResultSummary {
        id: layer_id.to_string(),
        deleted_from_disk: true,
        layers_removed,
        feature_count,
        photos_count,
    })
}

/// Menghitung dependensi sebelum penghapusan (pembungkus publik ke storage default).
pub fn count_spatial_result_dependents(
    layer_id: &str,
) -> Result<DeleteSpatialResultSummary, MapControllerError> {
    count_spatial_result_dependents_in_dir(&get_storage_root(), layer_id)
}

/// Menghapus Spatial Result yang tersimpan dengan mekanisme transaksional 2 fase:
/// 1. Rename folder dan/atau flat file serta foto unggahan ke karantina sementara (`.trash_sr_{id}_{timestamp}`).
/// 2. Hapus registrasi layer terkait di `layers/{id}.json`.
/// 3. Jika fase 2 berhasil, hapus permanen folder karantina.
/// 4. Jika fase 2 gagal, pulihkan (rollback) berkas dan foto dari karantina ke nama aslinya.
pub fn delete_saved_spatial_result_in_dir(
    storage_dir: &Path,
    layer_id: &str,
) -> Result<bool, MapControllerError> {
    validate_spatial_result_id(layer_id)?;

    let root_dir = storage_dir.join("spatial_results");
    let safe_id = sanitize_filename(layer_id);
    let layer_dir = root_dir.join(&safe_id);
    let flat_file = root_dir.join(format!("{}.json", safe_id));

    let folder_exists = layer_dir.is_dir();
    let flat_exists = flat_file.is_file();

    if !folder_exists && !flat_exists {
        return Err(MapControllerError::NotFound(format!(
            "Spatial result dengan ID '{}' tidak ditemukan di storage",
            layer_id
        )));
    }

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let trash_dir = root_dir.join(format!(".trash_sr_{}_{}", safe_id, now_ts));
    fs::create_dir_all(&trash_dir)?;

    let trash_folder = trash_dir.join("decomposed_folder");
    let trash_flat = trash_dir.join("flat.json");
    let trash_photos = trash_dir.join("photos");

    // Fase 1: Pindahkan layer ke karantina
    if folder_exists {
        fs::rename(&layer_dir, &trash_folder).map_err(|e| {
            let _ = fs::remove_dir_all(&trash_dir);
            MapControllerError::StorageError(format!("Gagal mengkarantina folder: {}", e))
        })?;
    }
    if flat_exists {
        fs::rename(&flat_file, &trash_flat).map_err(|e| {
            if folder_exists {
                let _ = fs::rename(&trash_folder, &layer_dir);
            }
            let _ = fs::remove_dir_all(&trash_dir);
            MapControllerError::StorageError(format!("Gagal mengkarantina flat file: {}", e))
        })?;
    }

    // Karantina foto unggahan milik layer ini
    let result_img_dir = storage_dir.join("images").join("result_image");
    let mut quarantined_photos: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    if result_img_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&result_img_dir) {
            for entry in entries.flatten() {
                let p_path = entry.path();
                if p_path.is_file() {
                    if let Some(filename) = p_path.file_name().and_then(|n| n.to_str()) {
                        if is_owned_result_photo(&safe_id, filename) {
                            let _ = fs::create_dir_all(&trash_photos);
                            let target_photo = trash_photos.join(filename);
                            if fs::rename(&p_path, &target_photo).is_ok() {
                                quarantined_photos.push((p_path, target_photo));
                            }
                        }
                    }
                }
            }
        }
    }

    // Fase 2: Hapus registrasi layer di `storage_dir/layers/{safe_id}.json`
    let layer_reg = storage_dir.join("layers").join(format!("{}.json", safe_id));
    let layer_reg_backup = trash_dir.join("layer_reg.json");
    let had_layer_reg = layer_reg.is_file();

    let execute_phase2 = || -> Result<(), MapControllerError> {
        if had_layer_reg {
            fs::copy(&layer_reg, &layer_reg_backup)?;
            fs::remove_file(&layer_reg)?;
        }
        Ok(())
    };

    if let Err(e) = execute_phase2() {
        // ROLLBACK: Kembalikan dari karantina ke posisi semula
        if folder_exists {
            let _ = fs::rename(&trash_folder, &layer_dir);
        }
        if flat_exists {
            let _ = fs::rename(&trash_flat, &flat_file);
        }
        for (orig, target) in quarantined_photos {
            let _ = fs::rename(&target, &orig);
        }
        if had_layer_reg && layer_reg_backup.is_file() {
            let _ = fs::copy(&layer_reg_backup, &layer_reg);
        }
        let _ = fs::remove_dir_all(&trash_dir);
        return Err(MapControllerError::StorageError(format!(
            "Gagal menghapus layer terdaftar, rollback berhasil dilakukan: {}",
            e
        )));
    }

    // Finalisasi: Hapus permanen karantina
    let _ = fs::remove_dir_all(&trash_dir);

    Ok(true)
}

/// Menghapus Spatial Result yang tersimpan (pembungkus publik ke storage default).
pub fn delete_saved_spatial_result(layer_id: &str) -> Result<bool, MapControllerError> {
    delete_saved_spatial_result_in_dir(&get_storage_root(), layer_id)
}
