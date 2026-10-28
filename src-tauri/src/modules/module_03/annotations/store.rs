//! Layanan persistensi atomic untuk anotasi pada `map_storage/annotations/{id}.json`.

use std::fs;
use std::path::PathBuf;

use super::types::AnnotationFeature;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::state::get_global_state;
use crate::modules::module_03::storage_utils::get_storage_root;

/// Mengembalikan direktori penyimpanan anotasi (`map_storage/annotations/`).
pub fn get_annotations_dir() -> PathBuf {
    let dir = get_storage_root().join("annotations");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

/// Menyimpan data anotasi ke file JSON pada `map_storage/annotations/{id}.json` secara atomik (write-through).
pub fn save_annotation(
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, MapControllerError> {
    get_global_state().save_annotation(annotation)
}

/// Mengambil seluruh data anotasi dari `map_storage/annotations/` (memory cache terurut).
pub fn get_saved_annotations() -> Result<Vec<AnnotationFeature>, MapControllerError> {
    Ok(get_global_state().get_saved_annotations())
}

/// Mengambil satu anotasi spesifik berdasarkan ID.
pub fn get_annotation_by_id(id: &str) -> Result<AnnotationFeature, MapControllerError> {
    get_global_state().get_annotation_by_id(id)
}

/// Mengupdate data anotasi yang sudah ada (write-through).
pub fn update_annotation(
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, MapControllerError> {
    get_global_state().update_annotation(annotation)
}

/// Menghapus anotasi dari `map_storage/annotations/{id}.json` dan memory cache.
pub fn delete_saved_annotation(id: &str) -> Result<bool, MapControllerError> {
    get_global_state().delete_annotation(id)
}
