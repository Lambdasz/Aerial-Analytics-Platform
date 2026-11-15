//! Layanan persistensi SQLite untuk entitas Area of Interest (AOI).

use rusqlite::{params, Connection};

use super::types::{AoiFeature, AoiGeometry, AoiProperties, Polygon};
use crate::modules::module_03::error::MapControllerError;

/// Menyimpan atau memperbarui entitas AOI ke dalam database SQLite.
pub fn save_aoi(conn: &Connection, aoi: AoiFeature) -> Result<AoiFeature, MapControllerError> {
    crate::modules::module_03::geometry::validate_polygon(&aoi.geometry.coordinates)?;

    let geom_str = serde_json::to_string(&aoi.geometry.coordinates)
        .map_err(|e| MapControllerError::InvalidGeometry(e.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO aoi (
            id, project_id, session_id, name, aoi_type,
            calculated_area_sq_m, perimeter_m, geometry, created_at, updated_at
        ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            aoi_type = excluded.aoi_type,
            calculated_area_sq_m = excluded.calculated_area_sq_m,
            perimeter_m = excluded.perimeter_m,
            geometry = excluded.geometry,
            updated_at = excluded.updated_at",
        params![
            aoi.properties.aoi_id,
            aoi.properties.name,
            aoi.properties.aoi_type,
            aoi.properties.calculated_area_sq_m,
            aoi.properties.perimeter_m,
            geom_str,
            aoi.properties.created_at,
            now,
        ],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    Ok(aoi)
}

/// Mengambil seluruh data AOI yang tersimpan di SQLite, terurut berdasarkan waktu pembuatan terbaru.
pub fn get_saved_aois(conn: &Connection) -> Result<Vec<AoiFeature>, MapControllerError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, aoi_type, calculated_area_sq_m, perimeter_m, geometry, created_at
             FROM aoi ORDER BY created_at DESC",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            let aoi_id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let aoi_type: Option<String> = row.get(2)?;
            let calculated_area_sq_m: f64 = row.get(3)?;
            let perimeter_m: f64 = row.get(4)?;
            let geom_str: String = row.get(5)?;
            let created_at: String = row.get(6)?;

            let coordinates: Polygon = serde_json::from_str(&geom_str).unwrap_or_else(|_| vec![]);

            Ok(AoiFeature {
                r#type: "Feature".to_string(),
                properties: AoiProperties {
                    aoi_id,
                    name,
                    aoi_type,
                    calculated_area_sq_m,
                    perimeter_m,
                    created_at,
                },
                geometry: AoiGeometry {
                    r#type: "Polygon".to_string(),
                    coordinates,
                },
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let aois = rows.flatten().collect();
    Ok(aois)
}

/// Menghapus AOI berdasarkan ID dari tabel SQLite `aoi`.
pub fn delete_saved_aoi(conn: &Connection, aoi_id: &str) -> Result<bool, MapControllerError> {
    let changes = conn
        .execute("DELETE FROM aoi WHERE id = ?1", params![aoi_id])
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
    fn test_aoi_sqlite_crud() {
        let conn = memory_db();

        let aoi = AoiFeature {
            r#type: "Feature".to_string(),
            properties: AoiProperties {
                aoi_id: "aoi_test_1".to_string(),
                name: "Kebun Sawit A".to_string(),
                aoi_type: Some("Kelapa Sawit".to_string()),
                calculated_area_sq_m: 1000.0,
                perimeter_m: 120.0,
                created_at: "2026-11-15T12:00:00Z".to_string(),
            },
            geometry: AoiGeometry {
                r#type: "Polygon".to_string(),
                coordinates: vec![vec![
                    [116.85, -1.22],
                    [116.86, -1.22],
                    [116.86, -1.23],
                    [116.85, -1.22],
                ]],
            },
        };

        // 1. Save
        let saved = save_aoi(&conn, aoi.clone()).expect("save aoi");
        assert_eq!(saved.properties.aoi_id, "aoi_test_1");

        // 2. Read
        let aois = get_saved_aois(&conn).expect("get aois");
        assert_eq!(aois.len(), 1);
        assert_eq!(aois[0].properties.name, "Kebun Sawit A");
        assert_eq!(aois[0].geometry.coordinates.len(), 1);

        // 3. Delete
        let deleted = delete_saved_aoi(&conn, "aoi_test_1").expect("delete aoi");
        assert!(deleted);

        // 4. Read after delete
        let aois_after = get_saved_aois(&conn).expect("get aois after delete");
        assert!(aois_after.is_empty());
    }
}
