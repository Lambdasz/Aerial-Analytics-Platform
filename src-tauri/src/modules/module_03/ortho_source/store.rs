//! Layanan persistensi atomic untuk citra orthomosaic (JPG/PNG) pada `map_storage/images/{id}/`.

use std::fs;
use std::path::{Path, PathBuf};

use super::image_pipeline::process_image_import;
use super::types::{ImportImagePayload, StoredImageMetadata};
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename};

/// Mengembalikan direktori root penyimpanan citra (`map_storage/images/`).
pub fn get_images_dir() -> PathBuf {
    let dir = get_storage_root().join("images");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

/// Mengimpor citra orthomosaic baru (JPG/PNG) beserta koordinat WGS84.
pub fn import_ortho_image(
    payload: ImportImagePayload,
) -> Result<StoredImageMetadata, MapControllerError> {
    process_image_import(payload)
}

/// Menyalin file citra yang ada dari suatu path ke dalam direktori storage modul 3 secara atomik (tanpa georeferensi).
pub fn save_image_from_path(source_path: &str) -> Result<StoredImageMetadata, MapControllerError> {
    let src = Path::new(source_path);
    let size_bytes = super::image_pipeline::validate_image_file_size(src)?;
    let ext = super::image_pipeline::validate_magic_bytes(src)?;
    let (width, height) = super::image_pipeline::validate_image_dimensions(src)?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let image_id = format!("raster_{}", now_ms);
    let target_dir = get_images_dir().join(&image_id);
    fs::create_dir_all(&target_dir)?;

    let original_file = target_dir.join(format!("original.{}", ext));
    fs::copy(src, &original_file)?;

    let display_size_bytes = match super::image_pipeline::generate_display_preview(
        &original_file,
        &target_dir,
        width,
        height,
    ) {
        Ok((_, size)) => Some(size),
        Err(_) => None,
    };

    let raw_name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&image_id)
        .to_string();

    let web_display_url = format!("/map_storage/images/{}/display.jpg", image_id);
    let web_original_url = format!("/map_storage/images/{}/original.{}", image_id, ext);

    let meta = StoredImageMetadata {
        schema_version: 1,
        id: image_id.clone(),
        filename: raw_name.clone(),
        category: super::types::ImageCategory::Raster,
        extension: ext,
        file_path: web_original_url,
        size_bytes,
        dimensions: Some([width, height]),
        thumbnail_url: Some(web_display_url),
        bounds: None,
        aspect_ratio: Some(width as f64 / height as f64),
        display_size_bytes,
        imported_at: Some(chrono::Utc::now().to_rfc3339()),
        updated_at: None,
        is_corrupted: Some(false),
    };

    let meta_json = target_dir.join("meta.json");
    let meta_str = serde_json::to_string_pretty(&meta)?;
    crate::modules::module_03::storage_utils::write_atomic(&meta_json, &meta_str)?;

    Ok(meta)
}

/// Menyimpan bytes citra langsung ke subfolder storage secara atomik.
pub fn save_image(filename: &str, data: &[u8]) -> Result<StoredImageMetadata, MapControllerError> {
    let temp_dir = tempfile::tempdir()?;
    let temp_file = temp_dir.path().join(filename);
    fs::write(&temp_file, data)?;
    save_image_from_path(&temp_file.to_string_lossy())
}

/// Memvalidasi ID citra dengan allowlist regex `^ortho_[0-9]+$` serta pencegahan path traversal.
pub fn validate_ortho_id(id: &str) -> Result<(), MapControllerError> {
    let is_valid_pattern = id
        .strip_prefix("ortho_")
        .map(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false);

    if !is_valid_pattern {
        return Err(MapControllerError::InvalidGeometry(format!(
            "ID citra '{}' tidak valid. Format harus memenuhi regex ^ortho_[0-9]+$",
            id
        )));
    }

    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(MapControllerError::InvalidGeometry(format!(
            "ID citra '{}' mengandung karakter path traversal terlarang",
            id
        )));
    }

    Ok(())
}

/// Mengambil daftar seluruh citra orthomosaic terurut descending (terbaru di atas).
/// Folder dengan berkas meta.json korup tetap dimuat dengan label 'Metadata rusak' dan is_corrupted = true.
pub fn list_ortho_images() -> Result<Vec<StoredImageMetadata>, MapControllerError> {
    let mut images = Vec::new();
    let root_dir = get_images_dir();

    if !root_dir.exists() {
        return Ok(images);
    }

    for entry in fs::read_dir(&root_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            // Abaikan folder tersembunyi / internal (.staging_*, .trash_*, dll)
            if folder_name.starts_with('.') {
                continue;
            }
            // Hanya proses folder yang mengikuti pola ID ortho atau raster
            let is_ortho = validate_ortho_id(&folder_name).is_ok();
            let is_raster = folder_name
                .strip_prefix("raster_")
                .map(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false);
            if !is_ortho && !is_raster {
                continue;
            }

            let meta_file = path.join("meta.json");
            let mut is_corrupt = false;
            let mut loaded_meta: Option<StoredImageMetadata> = None;

            if meta_file.exists() {
                if let Ok(content) = fs::read_to_string(&meta_file) {
                    if let Ok(mut meta) = serde_json::from_str::<StoredImageMetadata>(&content) {
                        if meta.display_size_bytes.is_none() {
                            let display_jpg = path.join("display.jpg");
                            let display_png = path.join("display.png");
                            let size = if display_jpg.exists() {
                                fs::metadata(&display_jpg).map(|m| m.len()).ok()
                            } else if display_png.exists() {
                                fs::metadata(&display_png).map(|m| m.len()).ok()
                            } else {
                                None
                            };
                            meta.display_size_bytes = size;
                        }
                        meta.is_corrupted = Some(false);
                        loaded_meta = Some(meta);
                    } else {
                        is_corrupt = true;
                    }
                } else {
                    is_corrupt = true;
                }
            } else {
                is_corrupt = true;
            }

            if let Some(meta) = loaded_meta {
                images.push(meta);
            } else if is_corrupt {
                let fallback_created = entry
                    .metadata()
                    .and_then(|m| m.created().or_else(|_| m.modified()))
                    .ok()
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                images.push(StoredImageMetadata {
                    schema_version: 1,
                    id: folder_name.clone(),
                    filename: format!("{} (Metadata rusak)", folder_name),
                    category: super::types::ImageCategory::Ortho,
                    extension: "unknown".to_string(),
                    file_path: format!("/map_storage/images/{}", folder_name),
                    size_bytes: 0,
                    dimensions: None,
                    thumbnail_url: None,
                    bounds: None,
                    aspect_ratio: None,
                    display_size_bytes: None,
                    imported_at: Some(fallback_created),
                    updated_at: None,
                    is_corrupted: Some(true),
                });
            }
        }
    }

    // Pindai juga berkas foto hasil analisis spasial di images/result_image/
    // agar tampil di Galeri (kategori: Raster), namun bounds: None agar tidak dimuat di canvas map.
    let result_img_dir = root_dir.join("result_image");
    if result_img_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&result_img_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let filename = match p.file_name().and_then(|n| n.to_str()) {
                        Some(f) => f.to_string(),
                        None => continue,
                    };
                    if filename.starts_with('.') {
                        continue;
                    }
                    let ext = p
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("png")
                        .to_string();
                    let size_bytes = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    let mtime_str = p
                        .metadata()
                        .and_then(|m| m.modified().or_else(|_| m.created()))
                        .ok()
                        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                    // Ekstraksi layer_id dari pattern {layer_id}_{timestamp}.{ext}
                    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(&filename);
                    let layer_id = if let Some(last_under) = stem.rfind('_') {
                        let candidate = &stem[..last_under];
                        if candidate.is_empty() {
                            stem
                        } else {
                            candidate
                        }
                    } else {
                        stem
                    };

                    images.push(StoredImageMetadata {
                        schema_version: 1,
                        id: layer_id.to_string(),
                        filename: filename.clone(),
                        category: super::types::ImageCategory::Raster,
                        extension: ext,
                        file_path: format!("/map_storage/images/result_image/{}", filename),
                        size_bytes,
                        dimensions: None,
                        thumbnail_url: Some(format!(
                            "/map_storage/images/result_image/{}",
                            filename
                        )),
                        bounds: None, // Foto hasil analisis tidak memiliki koordinat bounds peta (non-georeferenced)
                        aspect_ratio: None,
                        display_size_bytes: Some(size_bytes),
                        imported_at: Some(mtime_str),
                        updated_at: None,
                        is_corrupted: Some(false),
                    });
                }
            }
        }
    }

    // Urutkan descending (terbaru di atas) berdasarkan imported_at atau id
    images.sort_by(|a, b| {
        let b_key = b.imported_at.as_deref().unwrap_or(&b.id);
        let a_key = a.imported_at.as_deref().unwrap_or(&a.id);
        b_key.cmp(a_key)
    });

    Ok(images)
}

/// Mengambil daftar seluruh citra orthomosaic yang tersimpan di storage atomik.
pub fn get_saved_images() -> Result<Vec<StoredImageMetadata>, MapControllerError> {
    list_ortho_images()
}

/// Mengambil satu metadata citra berdasarkan ID.
pub fn get_ortho_image(id: &str) -> Result<StoredImageMetadata, MapControllerError> {
    validate_ortho_id(id)?;
    let target_dir = get_images_dir().join(id);
    if !target_dir.exists() || !target_dir.is_dir() {
        return Err(MapControllerError::NotFound(format!(
            "Citra dengan ID '{}' tidak ditemukan",
            id
        )));
    }

    let meta_file = target_dir.join("meta.json");
    if meta_file.exists() {
        if let Ok(content) = fs::read_to_string(&meta_file) {
            if let Ok(mut meta) = serde_json::from_str::<StoredImageMetadata>(&content) {
                if meta.display_size_bytes.is_none() {
                    let display_jpg = target_dir.join("display.jpg");
                    let display_png = target_dir.join("display.png");
                    let size = if display_jpg.exists() {
                        fs::metadata(&display_jpg).map(|m| m.len()).ok()
                    } else if display_png.exists() {
                        fs::metadata(&display_png).map(|m| m.len()).ok()
                    } else {
                        None
                    };
                    meta.display_size_bytes = size;
                }
                meta.is_corrupted = Some(false);
                return Ok(meta);
            }
        }
    }

    // Fallback jika meta.json korup
    let fallback_created = fs::metadata(&target_dir)
        .and_then(|m| m.created().or_else(|_| m.modified()))
        .ok()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    Ok(StoredImageMetadata {
        schema_version: 1,
        id: id.to_string(),
        filename: format!("{} (Metadata rusak)", id),
        category: super::types::ImageCategory::Ortho,
        extension: "unknown".to_string(),
        file_path: format!("/map_storage/images/{}", id),
        size_bytes: 0,
        dimensions: None,
        thumbnail_url: None,
        bounds: None,
        aspect_ratio: None,
        display_size_bytes: None,
        imported_at: Some(fallback_created),
        updated_at: None,
        is_corrupted: Some(true),
    })
}

/// Memperbarui nama dan koordinat bounds citra orthomosaic.
/// Menjamin field immutable (`id`, `imported_at`, `schema_version`) tidak berubah.
pub fn update_ortho_image(
    payload: super::types::UpdateOrthoImagePayload,
) -> Result<StoredImageMetadata, MapControllerError> {
    validate_ortho_id(&payload.id)?;
    let target_dir = get_images_dir().join(&payload.id);
    if !target_dir.exists() || !target_dir.is_dir() {
        return Err(MapControllerError::NotFound(format!(
            "Citra dengan ID '{}' tidak ditemukan",
            payload.id
        )));
    }

    let meta_file = target_dir.join("meta.json");
    if !meta_file.exists() {
        return Err(MapControllerError::StorageError(format!(
            "Berkas meta.json untuk citra '{}' tidak ditemukan atau korup",
            payload.id
        )));
    }

    let content = fs::read_to_string(&meta_file)?;
    let mut current_meta: StoredImageMetadata = serde_json::from_str(&content)
        .map_err(|e| MapControllerError::StorageError(format!("Gagal membaca meta.json: {}", e)))?;

    // Update nama jika diberikan
    if let Some(new_name) = payload.filename {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(MapControllerError::InvalidGeometry(
                "Nama citra tidak boleh kosong".to_string(),
            ));
        }
        current_meta.filename = trimmed.to_string();
    }

    // Update bounds jika salah satu koordinat diberikan
    if payload.west.is_some()
        || payload.south.is_some()
        || payload.east.is_some()
        || payload.north.is_some()
    {
        let current_bounds = current_meta.bounds.unwrap_or([[0.0, 0.0], [0.0, 0.0]]);
        let west = payload.west.unwrap_or(current_bounds[0][1]);
        let south = payload.south.unwrap_or(current_bounds[0][0]);
        let east = payload.east.unwrap_or(current_bounds[1][1]);
        let north = payload.north.unwrap_or(current_bounds[1][0]);

        super::image_pipeline::validate_wgs84_bounds(west, south, east, north)?;
        current_meta.bounds = Some([[south, west], [north, east]]);
    }

    current_meta.updated_at = Some(chrono::Utc::now().to_rfc3339());
    current_meta.is_corrupted = Some(false);

    // Tulis atomik ke meta.json
    let meta_str = serde_json::to_string_pretty(&current_meta)?;
    crate::modules::module_03::storage_utils::write_atomic(&meta_file, &meta_str)?;

    // Sinkronkan layer di map_storage/layers/{id}.json jika ada
    let layer_file = get_storage_root()
        .join("layers")
        .join(format!("{}.json", payload.id));
    if layer_file.exists() {
        if let Ok(layer_content) = fs::read_to_string(&layer_file) {
            if let Ok(mut layer_state) = serde_json::from_str::<
                crate::modules::module_03::layers::types::LayerState,
            >(&layer_content)
            {
                layer_state.layer_name = current_meta.filename.clone();
                let _ = crate::modules::module_03::storage_utils::write_atomic(
                    &layer_file,
                    &serde_json::to_string_pretty(&layer_state).unwrap_or_default(),
                );
            }
        }
    }

    Ok(current_meta)
}

/// Menghitung data cascade (layer dan anotasi terikat) sebelum penghapusan citra.
pub fn get_ortho_cascade_summary(
    id: &str,
) -> Result<super::types::DeleteOrthoSummary, MapControllerError> {
    validate_ortho_id(id)?;

    let layer_file = get_storage_root()
        .join("layers")
        .join(format!("{}.json", id));
    let layer_exists = layer_file.exists();

    let mut anno_count = 0;
    let anno_dir = get_storage_root().join("annotations");
    if anno_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&anno_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(anno) = serde_json::from_str::<
                            crate::modules::module_03::annotations::types::AnnotationFeature,
                        >(&content)
                        {
                            if anno.image_id.as_deref() == Some(id) {
                                anno_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(super::types::DeleteOrthoSummary {
        id: id.to_string(),
        image_deleted: false,
        layer_deleted: layer_exists,
        annotations_deleted_count: anno_count,
    })
}

/// Menghapus satu citra orthomosaic secara tuntas dengan mekanisme 2 fase:
/// 1. Rename folder ke folder karantina sementara (.trash_{id}_{timestamp})
/// 2. Hapus layer dan anotasi terikat (image_id == id)
/// 3. Hapus permanen folder karantina.
///
/// Jika ada error pada langkah 2, rollback rename folder karantina kembali ke nama aslinya.
pub fn delete_ortho_image_single(
    id: &str,
) -> Result<super::types::DeleteOrthoSummary, MapControllerError> {
    validate_ortho_id(id)?;

    let images_dir = get_images_dir();
    let image_dir = images_dir.join(id);
    if !image_dir.exists() || !image_dir.is_dir() {
        return Err(MapControllerError::NotFound(format!(
            "Citra dengan ID '{}' tidak ditemukan",
            id
        )));
    }

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let trash_dir = images_dir.join(format!(".trash_{}_{}", id, now_ts));

    // Langkah 1: Karantina folder citra via rename atomik
    fs::rename(&image_dir, &trash_dir).map_err(|e| {
        MapControllerError::StorageError(format!("Gagal memindahkan folder ke karantina: {}", e))
    })?;

    // Langkah 2: Hapus data turunan
    let dep_result: Result<(bool, usize), MapControllerError> = (|| {
        // Hapus layer terkait jika ada
        let layer_file = get_storage_root()
            .join("layers")
            .join(format!("{}.json", id));
        let mut layer_deleted = false;
        if layer_file.exists() {
            fs::remove_file(&layer_file)?;
            layer_deleted = true;
        }

        // Hapus anotasi terikat (image_id == id)
        let mut anno_count = 0;
        let anno_dir = get_storage_root().join("annotations");
        if anno_dir.is_dir() {
            for entry in fs::read_dir(&anno_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(anno) = serde_json::from_str::<
                            crate::modules::module_03::annotations::types::AnnotationFeature,
                        >(&content)
                        {
                            if anno.image_id.as_deref() == Some(id) {
                                fs::remove_file(&path)?;
                                anno_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok((layer_deleted, anno_count))
    })();

    match dep_result {
        Ok((layer_deleted, anno_count)) => {
            // Langkah 3: Bersihkan folder karantina permanen
            let _ = fs::remove_dir_all(&trash_dir);

            Ok(super::types::DeleteOrthoSummary {
                id: id.to_string(),
                image_deleted: true,
                layer_deleted,
                annotations_deleted_count: anno_count,
            })
        }
        Err(err) => {
            // ROLLBACK: Kembalikan folder karantina ke nama aslinya
            let _ = fs::rename(&trash_dir, &image_dir);
            Err(MapControllerError::StorageError(format!(
                "Penghapusan gagal di tengah proses dan berhasil di-rollback: {}",
                err
            )))
        }
    }
}

/// Menghapus citra dari storage berdasarkan nama folder / image_id (legacy fallback).
pub fn delete_saved_image(category: &str, filename: &str) -> Result<bool, MapControllerError> {
    let safe_id = sanitize_filename(filename);
    let stem = Path::new(&safe_id)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&safe_id);

    let mut deleted = false;

    for dir_name in &[stem, &safe_id] {
        let target_dir = get_images_dir().join(dir_name);
        if target_dir.exists() && target_dir.is_dir() {
            fs::remove_dir_all(&target_dir)?;
            deleted = true;
        }
    }

    // Periksa juga jika ada di subfolder kategori legacy
    let legacy_dir = get_images_dir().join(category).join(&safe_id);
    if legacy_dir.exists() {
        let _ = fs::remove_file(&legacy_dir);
        deleted = true;
    }

    // Hapus layer terkait jika ada
    let _ = crate::modules::module_03::layers::store::delete_saved_layer(filename);
    let _ = crate::modules::module_03::layers::store::delete_saved_layer(stem);
    let _ = crate::modules::module_03::layers::store::delete_saved_layer(&safe_id);

    Ok(deleted)
}

/// Rekonsiliasi konsistensi storage saat startup aplikasi:
/// 1. Penanganan folder karantina `.trash_*`:
///    - Jika file `layers/{id}.json` masih ada, pulihkan kembali `.trash_*` ke `images/{id}`.
///    - Jika file `layers/{id}.json` sudah terhapus, tuntaskan penghapusan permanen `.trash_*`.
/// 2. Kasus A (Auto-heal layer hilang):
///    - Jika folder `images/{id}/` ada dengan `meta.json` valid namun `layers/{id}.json` belum ada,
///      daftarkan ulang layer default secara otomatis.
/// 3. Kasus B (Hapus layer yatim):
///    - Jika file `layers/{id}.json` bertipe raster ada namun folder `images/{id}/` tidak ada,
///      hapus berkas layer yatim tersebut agar tidak merender layer hantu di Leaflet.
pub fn reconcile_storage_layers() -> Result<(), MapControllerError> {
    let images_dir = get_images_dir();
    let layers_dir = crate::modules::module_03::layers::store::get_layers_dir();

    // 1. Tangani sisa folder karantina .trash_*
    if images_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&images_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(".trash_") {
                        // Format penamaan: .trash_{id}_{timestamp}
                        // Contoh: .trash_ortho_1792850000000_1792850005000
                        let without_prefix = match name.strip_prefix(".trash_") {
                            Some(w) => w,
                            None => continue,
                        };
                        let image_id = if let Some(last_under) = without_prefix.rfind('_') {
                            &without_prefix[..last_under]
                        } else {
                            without_prefix
                        };

                        let layer_file = layers_dir.join(format!("{}.json", image_id));
                        if layer_file.exists() {
                            let restored_dir = images_dir.join(image_id);
                            if !restored_dir.exists() {
                                let _ = fs::rename(&path, &restored_dir);
                            } else {
                                let _ = fs::remove_dir_all(&path);
                            }
                        } else {
                            let _ = fs::remove_dir_all(&path);
                        }
                    }
                }
            }
        }
    }

    // 2. Kasus A (Auto-heal layer hilang)
    if images_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&images_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = entry.file_name().to_string_lossy().to_string();
                    if folder_name.starts_with('.') {
                        continue;
                    }
                    let is_ortho = validate_ortho_id(&folder_name).is_ok();
                    let is_raster = folder_name.starts_with("raster_");
                    if !is_ortho && !is_raster {
                        continue;
                    }

                    let layer_file = layers_dir.join(format!("{}.json", folder_name));
                    if !layer_file.exists() {
                        let meta_file = path.join("meta.json");
                        if meta_file.exists() {
                            if let Ok(content) = fs::read_to_string(&meta_file) {
                                if let Ok(meta) =
                                    serde_json::from_str::<StoredImageMetadata>(&content)
                                {
                                    let layer_state =
                                        crate::modules::module_03::layers::types::LayerState {
                                            layer_id: folder_name.clone(),
                                            layer_name: meta.filename.clone(),
                                            category: "raster".to_string(),
                                            is_visible: true,
                                            opacity: 1.0,
                                            z_index: 10,
                                            source_type: Some("raster".to_string()),
                                            source_id: Some(folder_name.clone()),
                                        };
                                    let _ = crate::modules::module_03::layers::store::save_layer(
                                        layer_state,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Kasus B (Hapus layer yatim - HANYA untuk layer bertipe raster/ortho)
    if layers_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&layers_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let file_stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    let is_ortho = validate_ortho_id(file_stem).is_ok();
                    let is_raster = file_stem.starts_with("raster_");
                    if is_ortho || is_raster {
                        // Verifikasi isi file layer: hanya proses jika category bernilai "raster"
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(layer_state) = serde_json::from_str::<
                                crate::modules::module_03::layers::types::LayerState,
                            >(&content)
                            {
                                // Lindungi layer non-raster (mis. vektor, aoi, anotasi) agar tidak terhapus
                                if layer_state.category != "raster" {
                                    continue;
                                }

                                let image_folder = images_dir.join(file_stem);
                                if !image_folder.exists() {
                                    let result_img_dir = images_dir.join("result_image");
                                    let mut found_in_result = false;
                                    if result_img_dir.is_dir() {
                                        if let Ok(r_entries) = fs::read_dir(&result_img_dir) {
                                            for r_entry in r_entries.flatten() {
                                                let r_name = r_entry
                                                    .file_name()
                                                    .to_string_lossy()
                                                    .to_string();
                                                if r_name.starts_with(file_stem) {
                                                    found_in_result = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if !found_in_result {
                                        let _ = fs::remove_file(&path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Mendaftarkan konfigurasi layer orthomosaic baru ke `map_storage/layers/{id}.json` secara atomik
/// (menulis ke file .tmp terlebih dahulu sebelum di-rename secara atomik).
pub fn register_ortho_layer(
    image_id: &str,
    layer_name: &str,
) -> Result<crate::modules::module_03::layers::types::LayerState, MapControllerError> {
    validate_ortho_id(image_id)?;

    let layer_state = crate::modules::module_03::layers::types::LayerState {
        layer_id: image_id.to_string(),
        layer_name: layer_name.to_string(),
        category: "raster".to_string(),
        is_visible: true,
        opacity: 1.0,
        z_index: 10,
        source_type: Some("raster".to_string()),
        source_id: Some(image_id.to_string()),
    };

    crate::modules::module_03::layers::store::save_layer(layer_state)
}
