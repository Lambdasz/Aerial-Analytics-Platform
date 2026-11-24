//! Layanan persistensi SQLite untuk entitas catatan anotasi inspeksi peta.

use rusqlite::{params, Connection};

use super::annotation_entity_struct::AnnotationFeature;
use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::types::geometry_type::SpatialGeometry;

/// Menyimpan data anotasi ke tabel SQLite `annotation`.
pub fn save_annotation(
    conn: &Connection,
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, MapControllerError> {
    let geom_str = serde_json::to_string(&annotation.geometry)
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO annotation (
            id, project_id, session_id, image_id, schema_version,
            text, geometry, created_at, updated_at
        ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(id) DO UPDATE SET
            image_id = excluded.image_id,
            schema_version = excluded.schema_version,
            text = excluded.text,
            geometry = excluded.geometry,
            updated_at = excluded.updated_at",
        params![
            annotation.id,
            annotation.image_id,
            annotation.schema_version,
            annotation.text,
            geom_str,
            annotation.created_at,
            now,
        ],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    Ok(annotation)
}

/// Mengambil seluruh data anotasi dari tabel SQLite `annotation` (terurut waktu terbaru).
pub fn get_saved_annotations(
    conn: &Connection,
) -> Result<Vec<AnnotationFeature>, MapControllerError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, schema_version, text, geometry, created_at, image_id
             FROM annotation ORDER BY created_at DESC",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let schema_version: u32 = row.get(1)?;
            let text: String = row.get(2)?;
            let geom_str: String = row.get(3)?;
            let created_at: String = row.get(4)?;
            let image_id: Option<String> = row.get(5)?;

            let geometry: SpatialGeometry =
                serde_json::from_str(&geom_str).unwrap_or_else(|_| SpatialGeometry::Point {
                    coordinates: vec![0.0, 0.0],
                });

            Ok(AnnotationFeature {
                id,
                schema_version,
                text,
                geometry,
                created_at,
                image_id,
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let list = rows.flatten().collect();
    Ok(list)
}

/// Mengambil satu anotasi spesifik berdasarkan ID dari SQLite.
pub fn get_annotation_by_id(
    conn: &Connection,
    id: &str,
) -> Result<AnnotationFeature, MapControllerError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, schema_version, text, geometry, created_at, image_id
             FROM annotation WHERE id = ?1",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let mut rows = stmt
        .query_map(params![id], |row| {
            let id: String = row.get(0)?;
            let schema_version: u32 = row.get(1)?;
            let text: String = row.get(2)?;
            let geom_str: String = row.get(3)?;
            let created_at: String = row.get(4)?;
            let image_id: Option<String> = row.get(5)?;

            let geometry: SpatialGeometry =
                serde_json::from_str(&geom_str).unwrap_or_else(|_| SpatialGeometry::Point {
                    coordinates: vec![0.0, 0.0],
                });

            Ok(AnnotationFeature {
                id,
                schema_version,
                text,
                geometry,
                created_at,
                image_id,
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    if let Some(row) = rows.next() {
        row.map_err(|e| MapControllerError::StorageError(e.to_string()))
    } else {
        Err(MapControllerError::NotFound(format!(
            "Anotasi '{}' tidak ditemukan",
            id
        )))
    }
}

/// Mengupdate data anotasi yang sudah ada di database SQLite.
pub fn update_annotation(
    conn: &Connection,
    annotation: AnnotationFeature,
) -> Result<AnnotationFeature, MapControllerError> {
    // Validasi keberadaan terlebih dahulu
    let _ = get_annotation_by_id(conn, &annotation.id)?;
    save_annotation(conn, annotation)
}

/// Menghapus anotasi dari tabel SQLite `annotation` berdasarkan ID.
pub fn delete_saved_annotation(conn: &Connection, id: &str) -> Result<bool, MapControllerError> {
    let changes = conn
        .execute("DELETE FROM annotation WHERE id = ?1", params![id])
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
    Ok(changes > 0)
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
    fn test_annotation_sqlite_crud() {
        let conn = memory_db();

        let anno = AnnotationFeature {
            id: "anno_001".to_string(),
            schema_version: 1,
            text: "Pohon tumbang menutup jalan".to_string(),
            geometry: SpatialGeometry::Point {
                coordinates: vec![116.85, -1.22],
            },
            created_at: "2026-11-15T10:00:00Z".to_string(),
            image_id: None,
        };

        // 1. Save
        let saved = save_annotation(&conn, anno.clone()).expect("save");
        assert_eq!(saved.id, "anno_001");

        // 2. Read by ID
        let found = get_annotation_by_id(&conn, "anno_001").expect("find");
        assert_eq!(found.text, "Pohon tumbang menutup jalan");
        assert_eq!(found.image_id, None);

        // 3. Update
        let updated = AnnotationFeature {
            text: "Pohon sudah dibersihkan".to_string(),
            ..found
        };
        update_annotation(&conn, updated).expect("update");
        let found_updated = get_annotation_by_id(&conn, "anno_001").expect("find updated");
        assert_eq!(found_updated.text, "Pohon sudah dibersihkan");

        // 4. List
        let list = get_saved_annotations(&conn).expect("list");
        assert_eq!(list.len(), 1);

        // 5. Delete
        let deleted = delete_saved_annotation(&conn, "anno_001").expect("delete");
        assert!(deleted);
        assert!(get_annotation_by_id(&conn, "anno_001").is_err());
    }
}
