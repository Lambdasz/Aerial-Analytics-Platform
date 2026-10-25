//! Pengelola state in-memory dan write-through cache untuk Module 03.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use crate::modules::module_03::annotations::types::AnnotationFeature;
use crate::modules::module_03::aoi::types::AoiFeature;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::layers::types::LayerState;
use crate::modules::module_03::photos::types::PhotoMetadata;
use crate::modules::module_03::sp_result::types::LayerPayload;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename, write_atomic};

static GLOBAL_STATE: OnceLock<Module03State> = OnceLock::new();

/// Mengembalikan referensi ke state global singleton Module 03.
pub fn get_global_state() -> &'static Module03State {
    GLOBAL_STATE.get_or_init(Module03State::default)
}

/// State terpusat untuk Module 03 yang memegang cache in-memory
/// dan mengimplementasikan kebijakan write-through disk-first.
#[derive(Debug)]
pub struct Module03State {
    pub storage_dir: PathBuf,
    pub annotations: Arc<RwLock<HashMap<String, AnnotationFeature>>>,
    pub aois: Arc<RwLock<HashMap<String, AoiFeature>>>,
    pub layers: Arc<RwLock<HashMap<String, LayerState>>>,
    pub spatial_results: Arc<RwLock<HashMap<String, LayerPayload>>>,
    pub photos: Arc<RwLock<HashMap<String, PhotoMetadata>>>,
    pub images: Arc<
        RwLock<
            HashMap<String, crate::modules::module_03::ortho_source::types::StoredImageMetadata>,
        >,
    >,
}

impl Default for Module03State {
    fn default() -> Self {
        Self::new(get_storage_root())
    }
}

impl Module03State {
    /// Membuat instance `Module03State` baru pada direktori penyimpanan tertentu,
    /// serta menginisialisasi cache memory dari file disk yang valid.
    pub fn new(storage_dir: PathBuf) -> Self {
        let state = Self {
            storage_dir: storage_dir.clone(),
            annotations: Arc::new(RwLock::new(HashMap::new())),
            aois: Arc::new(RwLock::new(HashMap::new())),
            layers: Arc::new(RwLock::new(HashMap::new())),
            spatial_results: Arc::new(RwLock::new(HashMap::new())),
            photos: Arc::new(RwLock::new(HashMap::new())),
            images: Arc::new(RwLock::new(HashMap::new())),
        };

        // Bersihkan sisa folder karantina .trash_* akibat crash sesi sebelumnya
        crate::modules::module_03::sp_result::store::cleanup_orphaned_trash(&storage_dir);

        state.refresh_from_disk();

        state
    }

    /// Membaca ulang seluruh data dari disk ke memory cache.
    /// Berkas yang rusak atau ID-nya tidak cocok dilewati secara aman.
    pub fn refresh_from_disk(&self) {
        // 1. Annotations
        let anno_dir = self.storage_dir.join("annotations");
        if anno_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(anno_dir) {
                let mut map = self.annotations.write().unwrap();
                map.clear();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(anno) = serde_json::from_str::<AnnotationFeature>(&content) {
                                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                if anno.id == stem && anno.schema_version == 1 {
                                    map.insert(anno.id.clone(), anno);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. AOIs
        let aoi_dir = self.storage_dir.join("aois");
        if aoi_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(aoi_dir) {
                let mut map = self.aois.write().unwrap();
                map.clear();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(aoi) = serde_json::from_str::<AoiFeature>(&content) {
                                map.insert(aoi.properties.aoi_id.clone(), aoi);
                            }
                        }
                    }
                }
            }
        }

        // 3. Layers
        let layer_dir = self.storage_dir.join("layers");
        if layer_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(layer_dir) {
                let mut map = self.layers.write().unwrap();
                map.clear();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(layer) = serde_json::from_str::<LayerState>(&content) {
                                map.insert(layer.layer_id.clone(), layer);
                            }
                        }
                    }
                }
            }
        }

        // 4. Spatial Results
        if let Ok(srs) =
            crate::modules::module_03::sp_result::store::get_saved_spatial_results_in_dir(
                &self.storage_dir,
            )
        {
            let mut map = self.spatial_results.write().unwrap();
            map.clear();
            for sr in srs {
                map.insert(sr.layer_id.clone(), sr);
            }
        }

        // 5. Photos
        let photo_dir = self.storage_dir.join("photos");
        if photo_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(photo_dir) {
                let mut map = self.photos.write().unwrap();
                map.clear();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(photo) = serde_json::from_str::<PhotoMetadata>(&content) {
                                map.insert(photo.id.clone(), photo);
                            }
                        }
                    }
                }
            }
        }

        // 6. Images
        if let Ok(imgs) = crate::modules::module_03::ortho_source::store::get_saved_images() {
            let mut map = self.images.write().unwrap();
            map.clear();
            for img in imgs {
                map.insert(img.id.clone(), img);
            }
        }
    }

    // ── Annotation CRUD (Write-Through Policy) ──────────────────────────

    /// Simpan anotasi baru (tulis disk terlebih dahulu, lalu update memory).
    pub fn save_annotation(
        &self,
        annotation: AnnotationFeature,
    ) -> Result<AnnotationFeature, MapControllerError> {
        let dir = self.storage_dir.join("annotations");
        let safe_id = sanitize_filename(&annotation.id);
        let target_file = dir.join(format!("{}.json", safe_id));

        let json_content = serde_json::to_string(&annotation)?;
        if json_content.len() > 65536 {
            return Err(MapControllerError::InvalidGeometry(
                "Ukuran JSON anotasi melebihi batas 64 KB".to_string(),
            ));
        }

        // 1. Tulis atomik ke disk
        write_atomic(&target_file, &json_content)?;

        // 2. Jika sukses, perbarui cache memory
        let mut map = self.annotations.write().unwrap();
        map.insert(annotation.id.clone(), annotation.clone());

        Ok(annotation)
    }

    /// Mengambil seluruh anotasi yang tersimpan (dari memory cache).
    pub fn get_saved_annotations(&self) -> Vec<AnnotationFeature> {
        let map = self.annotations.read().unwrap();
        let mut list: Vec<AnnotationFeature> = map.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    /// Mengambil satu anotasi spesifik berdasarkan ID.
    pub fn get_annotation_by_id(&self, id: &str) -> Result<AnnotationFeature, MapControllerError> {
        let map = self.annotations.read().unwrap();
        map.get(id).cloned().ok_or_else(|| {
            MapControllerError::NotFound(format!("Anotasi '{}' tidak ditemukan", id))
        })
    }

    /// Memperbarui catatan anotasi (tulis disk terlebih dahulu, lalu update memory).
    pub fn update_annotation(
        &self,
        annotation: AnnotationFeature,
    ) -> Result<AnnotationFeature, MapControllerError> {
        let dir = self.storage_dir.join("annotations");
        let safe_id = sanitize_filename(&annotation.id);
        let target_file = dir.join(format!("{}.json", safe_id));

        if !target_file.exists() {
            return Err(MapControllerError::NotFound(format!(
                "Anotasi dengan ID '{}' tidak ditemukan untuk diperbarui",
                annotation.id
            )));
        }

        let json_content = serde_json::to_string(&annotation)?;
        if json_content.len() > 65536 {
            return Err(MapControllerError::InvalidGeometry(
                "Ukuran JSON anotasi melebihi batas 64 KB".to_string(),
            ));
        }

        // 1. Tulis atomik ke disk
        write_atomic(&target_file, &json_content)?;

        // 2. Perbarui cache memory
        let mut map = self.annotations.write().unwrap();
        map.insert(annotation.id.clone(), annotation.clone());

        Ok(annotation)
    }

    /// Menghapus anotasi dari disk dan memory cache.
    pub fn delete_annotation(&self, id: &str) -> Result<bool, MapControllerError> {
        let dir = self.storage_dir.join("annotations");
        let safe_id = sanitize_filename(id);
        let target_file = dir.join(format!("{}.json", safe_id));

        let deleted_from_disk = if target_file.exists() {
            fs::remove_file(target_file)?;
            true
        } else {
            false
        };

        let mut map = self.annotations.write().unwrap();
        map.remove(id);

        Ok(deleted_from_disk)
    }

    // ── AOI CRUD (Write-Through Policy) ──────────────────────────────────

    pub fn save_aoi(&self, aoi: AoiFeature) -> Result<AoiFeature, MapControllerError> {
        crate::modules::module_03::geometry::validate_polygon(&aoi.geometry.coordinates)?;

        let dir = self.storage_dir.join("aois");
        let safe_id = sanitize_filename(&aoi.properties.aoi_id);
        let target_file = dir.join(format!("{}.json", safe_id));

        let json_content = serde_json::to_string(&aoi)?;
        if json_content.len() > 65536 {
            return Err(MapControllerError::InvalidGeometry(
                "Ukuran JSON AOI melebihi batas 64 KB".to_string(),
            ));
        }

        write_atomic(&target_file, &json_content)?;

        let mut map = self.aois.write().unwrap();
        map.insert(aoi.properties.aoi_id.clone(), aoi.clone());

        Ok(aoi)
    }

    pub fn get_saved_aois(&self) -> Vec<AoiFeature> {
        let map = self.aois.read().unwrap();
        let mut list: Vec<AoiFeature> = map.values().cloned().collect();
        list.sort_by(|a, b| b.properties.created_at.cmp(&a.properties.created_at));
        list
    }

    pub fn delete_saved_aoi(&self, aoi_id: &str) -> Result<bool, MapControllerError> {
        let dir = self.storage_dir.join("aois");
        let safe_id = sanitize_filename(aoi_id);
        let target_file = dir.join(format!("{}.json", safe_id));

        let deleted = if target_file.exists() {
            fs::remove_file(target_file)?;
            true
        } else {
            false
        };

        let mut map = self.aois.write().unwrap();
        map.remove(aoi_id);

        Ok(deleted)
    }

    // ── Image / Ortho Source Management (Write-Through Policy) ───────────

    pub fn import_ortho_image(
        &self,
        payload: crate::modules::module_03::ortho_source::types::ImportImagePayload,
    ) -> Result<
        crate::modules::module_03::ortho_source::types::StoredImageMetadata,
        MapControllerError,
    > {
        let meta = crate::modules::module_03::ortho_source::store::import_ortho_image(payload)?;
        self.refresh_from_disk();
        Ok(meta)
    }

    pub fn save_image_from_path(
        &self,
        source_path: &str,
    ) -> Result<
        crate::modules::module_03::ortho_source::types::StoredImageMetadata,
        MapControllerError,
    > {
        let meta =
            crate::modules::module_03::ortho_source::store::save_image_from_path(source_path)?;
        self.refresh_from_disk();
        Ok(meta)
    }

    pub fn get_saved_images(
        &self,
    ) -> Vec<crate::modules::module_03::ortho_source::types::StoredImageMetadata> {
        self.refresh_from_disk();
        let map = self.images.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn delete_saved_image(
        &self,
        category: &str,
        filename: &str,
    ) -> Result<bool, MapControllerError> {
        let deleted =
            crate::modules::module_03::ortho_source::store::delete_saved_image(category, filename)?;
        self.refresh_from_disk();
        Ok(deleted)
    }

    pub fn list_ortho_images(
        &self,
    ) -> Result<
        Vec<crate::modules::module_03::ortho_source::types::StoredImageMetadata>,
        MapControllerError,
    > {
        self.refresh_from_disk();
        crate::modules::module_03::ortho_source::store::list_ortho_images()
    }

    pub fn get_ortho_image(
        &self,
        id: &str,
    ) -> Result<
        crate::modules::module_03::ortho_source::types::StoredImageMetadata,
        MapControllerError,
    > {
        crate::modules::module_03::ortho_source::store::get_ortho_image(id)
    }

    pub fn update_ortho_image(
        &self,
        payload: crate::modules::module_03::ortho_source::types::UpdateOrthoImagePayload,
    ) -> Result<
        crate::modules::module_03::ortho_source::types::StoredImageMetadata,
        MapControllerError,
    > {
        let meta = crate::modules::module_03::ortho_source::store::update_ortho_image(payload)?;
        self.refresh_from_disk();
        Ok(meta)
    }

    pub fn get_ortho_cascade_summary(
        &self,
        id: &str,
    ) -> Result<
        crate::modules::module_03::ortho_source::types::DeleteOrthoSummary,
        MapControllerError,
    > {
        crate::modules::module_03::ortho_source::store::get_ortho_cascade_summary(id)
    }

    pub fn delete_ortho_image(
        &self,
        id: &str,
    ) -> Result<
        crate::modules::module_03::ortho_source::types::DeleteOrthoSummary,
        MapControllerError,
    > {
        let summary =
            crate::modules::module_03::ortho_source::store::delete_ortho_image_single(id)?;
        self.refresh_from_disk();
        Ok(summary)
    }

    // ── Spatial Results Management (Write-Through Policy) ────────────────

    pub fn save_spatial_result(
        &self,
        payload: LayerPayload,
    ) -> Result<LayerPayload, MapControllerError> {
        let saved = crate::modules::module_03::sp_result::store::save_spatial_result_in_dir(
            &self.storage_dir,
            payload,
        )?;
        self.refresh_from_disk();
        Ok(saved)
    }

    pub fn get_saved_spatial_results(&self) -> Vec<LayerPayload> {
        let map = self.spatial_results.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn get_saved_spatial_results_summary(
        &self,
    ) -> Result<
        Vec<crate::modules::module_03::sp_result::types::SpatialResultSummary>,
        MapControllerError,
    > {
        crate::modules::module_03::sp_result::store::get_saved_spatial_results_summary_in_dir(
            &self.storage_dir,
        )
    }

    pub fn get_saved_spatial_result(
        &self,
        layer_id: &str,
    ) -> Result<LayerPayload, MapControllerError> {
        crate::modules::module_03::sp_result::store::get_saved_spatial_result_in_dir(
            &self.storage_dir,
            layer_id,
        )
    }

    pub fn update_spatial_result(
        &self,
        payload: crate::modules::module_03::sp_result::types::UpdateSpatialResultPayload,
    ) -> Result<LayerPayload, MapControllerError> {
        let updated = crate::modules::module_03::sp_result::store::update_spatial_result_in_dir(
            &self.storage_dir,
            payload,
        )?;
        self.refresh_from_disk();
        Ok(updated)
    }

    pub fn count_spatial_result_dependents(
        &self,
        layer_id: &str,
    ) -> Result<
        crate::modules::module_03::sp_result::types::DeleteSpatialResultSummary,
        MapControllerError,
    > {
        crate::modules::module_03::sp_result::store::count_spatial_result_dependents_in_dir(
            &self.storage_dir,
            layer_id,
        )
    }

    pub fn delete_saved_spatial_result(&self, layer_id: &str) -> Result<bool, MapControllerError> {
        let deleted =
            crate::modules::module_03::sp_result::store::delete_saved_spatial_result_in_dir(
                &self.storage_dir,
                layer_id,
            )?;
        self.refresh_from_disk();
        Ok(deleted)
    }

    pub fn check_spatial_result_manual_edits(
        &self,
        layer_id: &str,
    ) -> Result<
        crate::modules::module_03::sp_result::types::ManualEditsCheckResult,
        MapControllerError,
    > {
        crate::modules::module_03::sp_result::store::check_spatial_result_manual_edits_in_dir(
            &self.storage_dir,
            layer_id,
        )
    }
}
