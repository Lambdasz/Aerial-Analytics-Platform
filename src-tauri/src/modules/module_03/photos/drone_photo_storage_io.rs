//! Layanan persistensi SQLite untuk entitas foto drone pada tabel `photo`.

use rusqlite::{params, Connection};

use super::drone_photo_struct::PhotoMetadata;
use crate::modules::module_03::error::MapControllerError;

/// Menyimpan metadata foto drone ke tabel SQLite `photo`.
pub fn save_photo(
    conn: &Connection,
    photo: PhotoMetadata,
) -> Result<PhotoMetadata, MapControllerError> {
    conn.execute(
        "INSERT INTO photo (
            id, session_id, image_id, schema_version,
            file_name, latitude, longitude, altitude_agl,
            heading_deg, thumbnail_path, created_at
        ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(id) DO UPDATE SET
            file_name = excluded.file_name,
            latitude = excluded.latitude,
            longitude = excluded.longitude,
            altitude_agl = excluded.altitude_agl,
            heading_deg = excluded.heading_deg,
            thumbnail_path = excluded.thumbnail_path",
        params![
            photo.id,
            photo.schema_version,
            photo.file_name,
            photo.latitude,
            photo.longitude,
            photo.altitude_agl,
            photo.heading_deg,
            photo.thumbnail_path,
            photo.created_at,
        ],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    Ok(photo)
}

/// Mengambil seluruh metadata foto dari tabel SQLite `photo` (terurut waktu terbaru).
pub fn get_photos(conn: &Connection) -> Result<Vec<PhotoMetadata>, MapControllerError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, schema_version, file_name, latitude, longitude, altitude_agl, heading_deg, thumbnail_path, created_at
             FROM photo ORDER BY created_at DESC",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PhotoMetadata {
                id: row.get(0)?,
                schema_version: row.get(1)?,
                file_name: row.get(2)?,
                latitude: row.get(3)?,
                longitude: row.get(4)?,
                altitude_agl: row.get(5)?,
                heading_deg: row.get(6)?,
                thumbnail_path: row.get(7)?,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let photos = rows.flatten().collect();
    Ok(photos)
}

/// Menghapus foto berdasarkan ID dari tabel SQLite `photo`.
pub fn delete_photo(conn: &Connection, id: &str) -> Result<bool, MapControllerError> {
    let changes = conn
        .execute("DELETE FROM photo WHERE id = ?1", params![id])
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
    fn test_photo_sqlite_crud() {
        let conn = memory_db();

        let photo = PhotoMetadata {
            id: "photo_001".to_string(),
            schema_version: 1,
            file_name: "DJI_001.JPG".to_string(),
            latitude: -1.2215,
            longitude: 116.8595,
            altitude_agl: 100.0,
            heading_deg: 90.0,
            thumbnail_path: "/thumbs/dji_001.jpg".to_string(),
            created_at: "2026-11-15T10:00:00Z".to_string(),
        };

        // 1. Save
        let saved = save_photo(&conn, photo.clone()).expect("save photo");
        assert_eq!(saved.id, "photo_001");

        // 2. Read
        let photos = get_photos(&conn).expect("get photos");
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].file_name, "DJI_001.JPG");

        // 3. Delete
        let deleted = delete_photo(&conn, "photo_001").expect("delete photo");
        assert!(deleted);
        assert!(get_photos(&conn).expect("get empty photos").is_empty());
    }
}
