//! Layanan persistensi SQLite untuk konfigurasi layer peta.

use std::fs;
use std::path::PathBuf;

use rusqlite::{params, Connection};

use super::types::LayerState;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::storage_utils::{get_storage_root, sanitize_filename, write_atomic};

/// Mengembalikan direktori penyimpanan layer disk legacy (`map_storage/layers/`).
pub fn get_layers_dir() -> PathBuf {
    let dir = get_storage_root().join("layers");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

/// Menyimpan atau memperbarui konfigurasi layer ke database SQLite.
pub fn save_layer(conn: &Connection, layer: LayerState) -> Result<LayerState, MapControllerError> {
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO layer (
            id, project_id, session_id, name, category,
            is_visible, opacity, z_index, source_type, source_id,
            created_at, updated_at
        ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            category = excluded.category,
            is_visible = excluded.is_visible,
            opacity = excluded.opacity,
            z_index = excluded.z_index,
            source_type = excluded.source_type,
            source_id = excluded.source_id,
            updated_at = excluded.updated_at",
        params![
            layer.layer_id,
            layer.layer_name,
            layer.category,
            if layer.is_visible { 1 } else { 0 },
            layer.opacity,
            layer.z_index as i64,
            layer.source_type,
            layer.source_id,
            now,
            now,
        ],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    Ok(layer)
}

/// Membaca seluruh definisi layer dari tabel SQLite `layer` terurut berdasarkan `z_index`.
pub fn get_saved_layers(conn: &Connection) -> Result<Vec<LayerState>, MapControllerError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, category, is_visible, opacity, z_index, source_type, source_id
             FROM layer ORDER BY z_index ASC",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            let layer_id: String = row.get(0)?;
            let layer_name: String = row.get(1)?;
            let category: String = row.get(2)?;
            let is_visible_i: i64 = row.get(3)?;
            let opacity: f64 = row.get(4)?;
            let z_index_i: i64 = row.get(5)?;
            let source_type: Option<String> = row.get(6)?;
            let source_id: Option<String> = row.get(7)?;

            Ok(LayerState {
                layer_id,
                layer_name,
                category,
                is_visible: is_visible_i != 0,
                opacity,
                z_index: z_index_i as usize,
                source_type,
                source_id,
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let layers = rows.flatten().collect();
    Ok(layers)
}

/// Menghapus file layer berdasarkan ID dari tabel SQLite `layer`.
pub fn delete_saved_layer(conn: &Connection, layer_id: &str) -> Result<bool, MapControllerError> {
    let changes = conn
        .execute("DELETE FROM layer WHERE id = ?1", params![layer_id])
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
    Ok(changes > 0)
}

/// Menyimpan layer ke disk JSON (kompatibilitas backward untuk ortho_source pipeline).
pub fn save_layer_disk(layer: LayerState) -> Result<LayerState, MapControllerError> {
    let dir = get_layers_dir();
    let safe_id = sanitize_filename(&layer.layer_id);
    let target_file = dir.join(format!("{}.json", safe_id));
    let json_content = serde_json::to_string(&layer)?;
    write_atomic(&target_file, &json_content)?;
    Ok(layer)
}

/// Menghapus file layer dari disk JSON (kompatibilitas backward untuk ortho_source pipeline).
pub fn delete_saved_layer_disk(layer_id: &str) -> Result<bool, MapControllerError> {
    let dir = get_layers_dir();
    let safe_id = sanitize_filename(layer_id);
    let target_file = dir.join(format!("{}.json", safe_id));
    if target_file.exists() {
        fs::remove_file(target_file)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_03::schema::migrate_map_schema;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        crate::modules::module_01::init_schema(&conn).expect("init module 01 schema");
        migrate_map_schema(&conn).expect("schema migration");
        conn
    }

    #[test]
    fn test_layer_sqlite_crud() {
        let conn = memory_db();

        let layer = LayerState {
            layer_id: "layer_ortho_1".to_string(),
            layer_name: "Balikpapan Ortho".to_string(),
            category: "raster".to_string(),
            is_visible: true,
            opacity: 0.8,
            z_index: 2,
            source_type: Some("raster".to_string()),
            source_id: Some("ortho_001".to_string()),
        };

        // 1. Save
        let saved = save_layer(&conn, layer.clone()).expect("save layer");
        assert_eq!(saved.layer_id, "layer_ortho_1");

        // 2. Read
        let layers = get_saved_layers(&conn).expect("get layers");
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].layer_name, "Balikpapan Ortho");
        assert!(layers[0].is_visible);
        assert_eq!(layers[0].z_index, 2);

        // 3. Delete
        let deleted = delete_saved_layer(&conn, "layer_ortho_1").expect("delete layer");
        assert!(deleted);
        assert!(get_saved_layers(&conn)
            .expect("get layers empty")
            .is_empty());
    }
}
