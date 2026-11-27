//! Layanan persistensi SQLite untuk hasil analitik spasial (Spatial Results).

use super::types::{
    DeleteSpatialResultSummary, LayerDisplayMode, LayerPayload, ManualEditsCheckResult,
    SpatialProperties, SpatialResult, SpatialResultSummary, UpdateSpatialResultPayload,
};
use super::writer::get_feature_id;
use crate::modules::module_03::error::MapControllerError;
use chrono::Utc;
use rusqlite::{params, Connection};

/// Menyimpan atau memperbarui satu kesatuan layer Spatial Result dan seluruh fiturnya ke SQLite.
pub fn save_spatial_result(
    conn: &Connection,
    payload: LayerPayload,
) -> Result<LayerPayload, MapControllerError> {
    payload.validate()?;

    let disp_pref_str = serde_json::to_string(&payload.display_preference)
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
    let extra_str = serde_json::to_string(&payload.extra).unwrap_or_else(|_| "{}".to_string());
    let now = Utc::now().to_rfc3339();
    let updated_at = payload.updated_at.clone().unwrap_or_else(|| now.clone());

    // 1. Simpan header layer sp_result
    conn.execute(
        "INSERT INTO sp_result (
            layer_id, project_id, session_id, layer_name,
            display_preference, feature_count, concurrency_token,
            extra, created_at, updated_at
        ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(layer_id) DO UPDATE SET
            layer_name = excluded.layer_name,
            display_preference = excluded.display_preference,
            feature_count = excluded.feature_count,
            concurrency_token = excluded.concurrency_token,
            extra = excluded.extra,
            updated_at = excluded.updated_at",
        params![
            payload.layer_id,
            payload.layer_name,
            disp_pref_str,
            payload.spatial_results.len() as i64,
            payload.concurrency_token.as_deref(),
            extra_str,
            updated_at,
            now,
        ],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    // 2. Sinkronkan ke tabel layer peta
    let (opacity, category) = match &payload.display_preference {
        LayerDisplayMode::Polygon { opacity, .. } => (*opacity, "vector"),
        LayerDisplayMode::Point { .. } => (1.0, "vector"),
        LayerDisplayMode::Line { .. } => (1.0, "vector"),
        LayerDisplayMode::ImageOverlay { opacity, .. } => (*opacity, "raster"),
    };

    conn.execute(
        "INSERT INTO layer (
            id, project_id, session_id, name, category,
            is_visible, opacity, z_index, source_type, source_id,
            created_at, updated_at
        ) VALUES (?1, NULL, NULL, ?2, ?3, 1, ?4, 5, 'spatial_result', ?1, ?5, ?5)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            category = excluded.category,
            opacity = excluded.opacity,
            updated_at = excluded.updated_at",
        params![payload.layer_id, payload.layer_name, category, opacity, now],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    // 3. Bersihkan fitur lama lalu masukkan fitur terkini
    conn.execute(
        "DELETE FROM sp_result_feature WHERE layer_id = ?1",
        params![payload.layer_id],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    for (idx, feat) in payload.spatial_results.iter().enumerate() {
        let feat_id = get_feature_id(feat, idx);
        let scoped_id = format!("{}_{}", payload.layer_id, feat_id);
        let geom_str = serde_json::to_string(&feat.geometry)
            .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
        let prop_str = serde_json::to_string(&feat.properties)
            .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
        let edited_str = feat
            .edited_fields
            .as_ref()
            .and_then(|ef| serde_json::to_string(ef).ok());
        let extra_str = serde_json::to_string(&feat.extra).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO sp_result_feature (
                id, layer_id, feature_index, source,
                geometry, properties, image_url, edited_fields, extra
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                scoped_id,
                payload.layer_id,
                idx as i64,
                feat.source,
                geom_str,
                prop_str,
                feat.image_url,
                edited_str,
                extra_str,
            ],
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;
    }

    Ok(payload)
}

/// Mengambil satu data Spatial Result lengkap berdasarkan layer ID dari database SQLite.
pub fn get_saved_spatial_result(
    conn: &Connection,
    layer_id: &str,
) -> Result<LayerPayload, MapControllerError> {
    let mut header_stmt = conn
        .prepare(
            "SELECT layer_id, layer_name, display_preference, concurrency_token, extra, updated_at
             FROM sp_result WHERE layer_id = ?1",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let header = header_stmt
        .query_row(params![layer_id], |row| {
            let layer_id: String = row.get(0)?;
            let layer_name: String = row.get(1)?;
            let disp_pref_str: String = row.get(2)?;
            let concurrency_token: Option<String> = row.get(3)?;
            let extra_str: String = row.get(4)?;
            let updated_at: Option<String> = row.get(5)?;

            let display_preference: LayerDisplayMode = serde_json::from_str(&disp_pref_str)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let extra: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&extra_str).unwrap_or_default();

            Ok((
                layer_id,
                layer_name,
                display_preference,
                concurrency_token,
                extra,
                updated_at,
            ))
        })
        .map_err(|_| {
            MapControllerError::NotFound(format!("Spatial result '{}' tidak ditemukan", layer_id))
        })?;

    let mut feat_stmt = conn
        .prepare(
            "SELECT source, geometry, properties, image_url, edited_fields, extra
             FROM sp_result_feature WHERE layer_id = ?1 ORDER BY feature_index ASC",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let feat_rows = feat_stmt
        .query_map(params![layer_id], |row| {
            let source: String = row.get(0)?;
            let geom_str: String = row.get(1)?;
            let prop_str: String = row.get(2)?;
            let image_url: Option<String> = row.get(3)?;
            let edited_str: Option<String> = row.get(4)?;
            let extra_str: String = row.get(5)?;

            let geometry = serde_json::from_str(&geom_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let properties: SpatialProperties = serde_json::from_str(&prop_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let edited_fields: Option<Vec<String>> =
                edited_str.and_then(|s| serde_json::from_str(&s).ok());
            let extra: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&extra_str).unwrap_or_default();

            Ok(SpatialResult {
                source,
                geometry,
                properties,
                image_url,
                edited_fields,
                extra,
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let spatial_results = feat_rows.flatten().collect();

    Ok(LayerPayload {
        layer_id: header.0,
        layer_name: header.1,
        display_preference: header.2,
        spatial_results,
        updated_at: header.5,
        concurrency_token: header.3,
        extra: header.4,
    })
}

/// Mengambil seluruh data Spatial Result yang tersimpan di SQLite.
pub fn get_saved_spatial_results(
    conn: &Connection,
) -> Result<Vec<LayerPayload>, MapControllerError> {
    let mut stmt = conn
        .prepare("SELECT layer_id FROM sp_result ORDER BY created_at DESC")
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let mut results = Vec::new();
    for id in ids.flatten() {
        if let Ok(layer) = get_saved_spatial_result(conn, &id) {
            results.push(layer);
        }
    }
    Ok(results)
}

/// Mengambil daftar ringkasan Spatial Result (ringan) untuk visualisasi daftar/kartu UI.
pub fn get_saved_spatial_results_summary(
    conn: &Connection,
) -> Result<Vec<SpatialResultSummary>, MapControllerError> {
    let mut stmt = conn
        .prepare(
            "SELECT layer_id, layer_name, display_preference, feature_count, concurrency_token, updated_at
             FROM sp_result ORDER BY created_at DESC",
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let layer_name: String = row.get(1)?;
            let disp_pref_str: String = row.get(2)?;
            let feature_count: i64 = row.get(3)?;
            let concurrency_token: Option<String> = row.get(4)?;
            let updated_at: Option<String> = row.get(5)?;

            let disp_pref: Result<LayerDisplayMode, _> = serde_json::from_str(&disp_pref_str);

            let (geom_type, display_color, icon, thumbnail_url) = if let Ok(ref dp) = disp_pref {
                let color = match dp {
                    LayerDisplayMode::Polygon { color, .. } => color.clone(),
                    LayerDisplayMode::Point { color, .. } => color.clone(),
                    LayerDisplayMode::Line { color, .. } => color.clone(),
                    LayerDisplayMode::ImageOverlay { .. } => "#2ECC71".to_string(),
                };
                let icon_str = match dp {
                    LayerDisplayMode::Point { icon, .. } => Some(icon.clone()),
                    _ => None,
                };
                let gtype = match dp {
                    LayerDisplayMode::Polygon { .. } => "Polygon",
                    LayerDisplayMode::Point { .. } => "Point",
                    LayerDisplayMode::Line { .. } => "Line",
                    LayerDisplayMode::ImageOverlay { .. } => "ImageOverlay",
                };
                let thumb = match dp {
                    LayerDisplayMode::Polygon { image_url, .. }
                    | LayerDisplayMode::Point { image_url, .. }
                    | LayerDisplayMode::Line { image_url, .. } => image_url.clone(),
                    LayerDisplayMode::ImageOverlay { image_url, .. } => Some(image_url.clone()),
                };
                (gtype.to_string(), color, icon_str, thumb)
            } else {
                ("Mixed".to_string(), "#2ECC71".to_string(), None, None)
            };

            Ok(SpatialResultSummary {
                id,
                layer_name,
                feature_count: feature_count as usize,
                geom_type,
                display_color,
                icon,
                thumbnail_url,
                format_type: "sqlite".to_string(),
                is_corrupted: false,
                updated_at,
                has_manual_edits: false,
                concurrency_token: concurrency_token.unwrap_or_else(|| "token".to_string()),
            })
        })
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let mut summaries = Vec::new();
    for row in rows.flatten() {
        summaries.push(row);
    }
    Ok(summaries)
}

/// Memperbarui field aman Spatial Result (nama layer, display_color, icon, fields, edit fitur) di SQLite.
pub fn update_saved_spatial_result(
    conn: &Connection,
    payload: UpdateSpatialResultPayload,
) -> Result<LayerPayload, MapControllerError> {
    let mut current = get_saved_spatial_result(conn, &payload.id)?;

    if let Some(ref name) = payload.name {
        current.layer_name = name.clone();
    }

    if let Some(ref color) = payload.display_color {
        match &mut current.display_preference {
            LayerDisplayMode::Polygon { color: c, .. }
            | LayerDisplayMode::Point { color: c, .. }
            | LayerDisplayMode::Line { color: c, .. } => *c = color.clone(),
            LayerDisplayMode::ImageOverlay { .. } => {}
        }
    }

    if let Some(ref icon) = payload.icon {
        if let LayerDisplayMode::Point { icon: i, .. } = &mut current.display_preference {
            *i = icon.clone();
        }
    }

    if let Some(fields) = payload.fields {
        match &mut current.display_preference {
            LayerDisplayMode::Polygon { fields: f, .. }
            | LayerDisplayMode::Point { fields: f, .. }
            | LayerDisplayMode::Line { fields: f, .. }
            | LayerDisplayMode::ImageOverlay { fields: f, .. } => *f = Some(fields),
        }
    }

    // Terapkan feature edits jika ada
    if let Some(edits) = payload.feature_edits {
        for edit in edits {
            if let Some(feat) = current.spatial_results.get_mut(edit.index) {
                let mut edited_list = feat.edited_fields.clone().unwrap_or_default();
                for (k, v) in edit.set {
                    if let SpatialProperties::Generic(ref mut map) = feat.properties {
                        map.insert(k.clone(), v);
                    }
                    if !edited_list.contains(&k) {
                        edited_list.push(k);
                    }
                }
                for k in edit.remove {
                    if let SpatialProperties::Generic(ref mut map) = feat.properties {
                        map.remove(&k);
                    }
                    if !edited_list.contains(&k) {
                        edited_list.push(k);
                    }
                }
                feat.edited_fields = Some(edited_list);
            }
        }
    }

    current.updated_at = Some(Utc::now().to_rfc3339());
    save_spatial_result(conn, current)
}

/// Menghitung dependensi sebelum penghapusan spatial result di SQLite.
pub fn count_spatial_result_dependents(
    conn: &Connection,
    layer_id: &str,
) -> Result<DeleteSpatialResultSummary, MapControllerError> {
    let feature_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sp_result_feature WHERE layer_id = ?1",
            params![layer_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let layer_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM layer WHERE id = ?1",
            params![layer_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    Ok(DeleteSpatialResultSummary {
        id: layer_id.to_string(),
        deleted_from_disk: true,
        layers_removed: layer_exists as usize,
        feature_count: feature_count as usize,
        photos_count: 0,
    })
}

/// Menghapus Spatial Result dari SQLite beserta seluruh fiturnya dan layer terkait.
pub fn delete_saved_spatial_result(
    conn: &Connection,
    layer_id: &str,
) -> Result<bool, MapControllerError> {
    conn.execute(
        "DELETE FROM sp_result_feature WHERE layer_id = ?1",
        params![layer_id],
    )
    .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let changes = conn
        .execute(
            "DELETE FROM sp_result WHERE layer_id = ?1",
            params![layer_id],
        )
        .map_err(|e| MapControllerError::StorageError(e.to_string()))?;

    let _ = conn.execute("DELETE FROM layer WHERE id = ?1", params![layer_id]);

    Ok(changes > 0)
}

/// Memeriksa keberadaan edit manual pada layer spasial di SQLite.
pub fn check_spatial_result_manual_edits(
    conn: &Connection,
    layer_id: &str,
) -> Result<ManualEditsCheckResult, MapControllerError> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sp_result WHERE layer_id = ?1",
            params![layer_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if exists == 0 {
        return Ok(ManualEditsCheckResult {
            layer_id: layer_id.to_string(),
            exists: false,
            has_manual_edits: false,
            edited_features_count: 0,
            custom_fields_count: 0,
        });
    }

    let edited_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sp_result_feature WHERE layer_id = ?1 AND edited_fields IS NOT NULL AND edited_fields != '[]'",
            params![layer_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    Ok(ManualEditsCheckResult {
        layer_id: layer_id.to_string(),
        exists: true,
        has_manual_edits: edited_count > 0,
        edited_features_count: edited_count as usize,
        custom_fields_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_03::schema::migrate_map_schema;
    use crate::modules::module_03::types::geometry_type::SpatialGeometry;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        crate::modules::module_01::init_schema(&conn).expect("init module 01 schema");
        migrate_map_schema(&conn).expect("schema migration");
        conn
    }

    #[test]
    fn test_spatial_result_sqlite_crud() {
        let conn = memory_db();

        let payload = LayerPayload {
            layer_id: "res_trees_01".to_string(),
            layer_name: "Deteksi Pohon Kelapa Sawit".to_string(),
            display_preference: LayerDisplayMode::Point {
                color: "#2ecc40".to_string(),
                icon: "tree_marker".to_string(),
                image_url: None,
                fields: None,
            },
            spatial_results: vec![
                SpatialResult {
                    source: "plugin_tree".to_string(),
                    geometry: SpatialGeometry::Point {
                        coordinates: vec![116.85, -1.22],
                    },
                    properties: SpatialProperties::Generic(serde_json::Map::new()),
                    image_url: None,
                    edited_fields: None,
                    extra: serde_json::Map::new(),
                },
                SpatialResult {
                    source: "plugin_tree".to_string(),
                    geometry: SpatialGeometry::Point {
                        coordinates: vec![116.86, -1.23],
                    },
                    properties: SpatialProperties::Generic(serde_json::Map::new()),
                    image_url: None,
                    edited_fields: None,
                    extra: serde_json::Map::new(),
                },
            ],
            updated_at: Some("2026-11-15T12:00:00Z".to_string()),
            concurrency_token: Some("token_1".to_string()),
            extra: serde_json::Map::new(),
        };

        // 1. Save
        let saved = save_spatial_result(&conn, payload).expect("save");
        assert_eq!(saved.layer_id, "res_trees_01");
        assert_eq!(saved.spatial_results.len(), 2);

        // 2. Read
        let fetched = get_saved_spatial_result(&conn, "res_trees_01").expect("fetch");
        assert_eq!(fetched.layer_name, "Deteksi Pohon Kelapa Sawit");
        assert_eq!(fetched.spatial_results.len(), 2);

        // 3. Summary
        let summaries = get_saved_spatial_results_summary(&conn).expect("summaries");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].feature_count, 2);
        assert_eq!(summaries[0].geom_type, "Point");

        // 4. Update
        let update_req = UpdateSpatialResultPayload {
            id: "res_trees_01".to_string(),
            name: Some("Deteksi Pohon Blok B".to_string()),
            ..Default::default()
        };
        let updated = update_saved_spatial_result(&conn, update_req).expect("update");
        assert_eq!(updated.layer_name, "Deteksi Pohon Blok B");

        // 5. Delete
        let deleted = delete_saved_spatial_result(&conn, "res_trees_01").expect("delete");
        assert!(deleted);
        assert!(get_saved_spatial_result(&conn, "res_trees_01").is_err());
    }
}
