//! Database schema definitions and idempotent startup migrations for Module 03 (Map Explorer).

use std::fs;
use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection};

use crate::modules::module_03::annotations::types::AnnotationFeature;
use crate::modules::module_03::aoi::types::AoiFeature;
use crate::modules::module_03::layers::types::LayerState;
use crate::modules::module_03::photos::types::PhotoMetadata;
use crate::modules::module_03::sp_result::types::LayerPayload;
use crate::modules::module_03::sp_result::writer::{
    get_feature_id, StoredFeatureFile, StoredLayerFile,
};

// ── DDL Constants ────────────────────────────────────────────────────────────

pub const AOI_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS aoi (
    id                   TEXT PRIMARY KEY,
    project_id           TEXT REFERENCES project(id) ON DELETE SET NULL,
    session_id           TEXT REFERENCES session(id) ON DELETE SET NULL,
    name                 TEXT NOT NULL,
    aoi_type             TEXT,
    calculated_area_sq_m REAL NOT NULL,
    perimeter_m          REAL NOT NULL,
    geometry             TEXT NOT NULL,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
)";

pub const ANNOTATION_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS annotation (
    id             TEXT PRIMARY KEY,
    project_id     TEXT REFERENCES project(id) ON DELETE SET NULL,
    session_id     TEXT REFERENCES session(id) ON DELETE SET NULL,
    image_id       TEXT REFERENCES image(id) ON DELETE SET NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    text           TEXT NOT NULL,
    geometry       TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
)";

pub const LAYER_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS layer (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES project(id) ON DELETE SET NULL,
    session_id  TEXT REFERENCES session(id) ON DELETE SET NULL,
    name        TEXT NOT NULL,
    category    TEXT NOT NULL,
    is_visible  INTEGER NOT NULL DEFAULT 1,
    opacity     REAL NOT NULL DEFAULT 1.0,
    z_index     INTEGER NOT NULL DEFAULT 0,
    source_type TEXT,
    source_id   TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
)";

pub const PHOTO_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS photo (
    id             TEXT PRIMARY KEY,
    session_id     TEXT REFERENCES session(id) ON DELETE SET NULL,
    image_id       TEXT REFERENCES image(id) ON DELETE SET NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    file_name      TEXT NOT NULL,
    latitude       REAL NOT NULL,
    longitude      REAL NOT NULL,
    altitude_agl   REAL NOT NULL,
    heading_deg    REAL NOT NULL,
    thumbnail_path TEXT NOT NULL,
    created_at     TEXT NOT NULL
)";

pub const SP_RESULT_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS sp_result (
    layer_id           TEXT PRIMARY KEY,
    project_id         TEXT REFERENCES project(id) ON DELETE SET NULL,
    session_id         TEXT REFERENCES session(id) ON DELETE SET NULL,
    layer_name         TEXT NOT NULL,
    display_preference TEXT NOT NULL,
    feature_count      INTEGER NOT NULL DEFAULT 0,
    concurrency_token  TEXT,
    extra              TEXT NOT NULL DEFAULT '{}',
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
)";

pub const SP_RESULT_FEATURE_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS sp_result_feature (
    id            TEXT PRIMARY KEY,
    layer_id      TEXT NOT NULL REFERENCES sp_result(layer_id) ON DELETE CASCADE,
    feature_index INTEGER NOT NULL,
    source        TEXT NOT NULL,
    geometry      TEXT NOT NULL,
    properties    TEXT NOT NULL,
    image_url     TEXT,
    edited_fields TEXT,
    extra         TEXT NOT NULL DEFAULT '{}'
)";

// ── Schema Migration ─────────────────────────────────────────────────────────

/// Initializes the Module 03 relational schema with additive tables and indexes.
pub fn migrate_map_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    // 1. Tables
    conn.execute(AOI_TABLE_DDL, [])?;
    conn.execute(ANNOTATION_TABLE_DDL, [])?;
    conn.execute(LAYER_TABLE_DDL, [])?;
    conn.execute(PHOTO_TABLE_DDL, [])?;
    conn.execute(SP_RESULT_TABLE_DDL, [])?;
    conn.execute(SP_RESULT_FEATURE_TABLE_DDL, [])?;

    // 2. Indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_aoi_project_id ON aoi(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_aoi_session_id ON aoi(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_annotation_session_id ON annotation(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_annotation_image_id ON annotation(image_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_layer_z_index ON layer(z_index)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_layer_project_id ON layer(project_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_photo_session_id ON photo(session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_photo_image_id ON photo(image_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sp_result_project_id ON sp_result(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sp_result_session_id ON sp_result(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sp_result_feature_layer ON sp_result_feature(layer_id, feature_index)",
        [],
    )?;

    Ok(())
}

// ── Idempotent Startup Migration from Legacy JSON ─────────────────────────────

/// Reads legacy JSON files from `map_storage/`, inserts them into SQLite idempotently,
/// and renames the source JSON files to `<file>.json.bak` to prevent data loss.
pub fn migrate_legacy_json_to_sqlite(
    conn: &Connection,
    storage_root: &Path,
) -> Result<(), rusqlite::Error> {
    if !storage_root.exists() {
        return Ok(());
    }

    let now_str = Utc::now().to_rfc3339();

    // 1. AOIs (map_storage/aois/*.json)
    let aois_dir = storage_root.join("aois");
    if aois_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&aois_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_active_json(&path) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(aoi) = serde_json::from_str::<AoiFeature>(&content) {
                            let geom_str = serde_json::to_string(&aoi.geometry.coordinates)
                                .unwrap_or_else(|_| "[]".to_string());
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
                                    now_str,
                                ],
                            )?;
                            rename_to_bak(&path);
                        }
                    }
                }
            }
        }
    }

    // 2. Annotations (map_storage/annotations/*.json)
    let anno_dir = storage_root.join("annotations");
    if anno_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&anno_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_active_json(&path) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(anno) = serde_json::from_str::<AnnotationFeature>(&content) {
                            let geom_str = serde_json::to_string(&anno.geometry)
                                .unwrap_or_else(|_| "{}".to_string());
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
                                    anno.id,
                                    anno.image_id,
                                    anno.schema_version,
                                    anno.text,
                                    geom_str,
                                    anno.created_at,
                                    now_str,
                                ],
                            )?;
                            rename_to_bak(&path);
                        }
                    }
                }
            }
        }
    }

    // 3. Layers (map_storage/layers/*.json)
    let layer_dir = storage_root.join("layers");
    if layer_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&layer_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_active_json(&path) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(layer) = serde_json::from_str::<LayerState>(&content) {
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
                                    now_str,
                                    now_str,
                                ],
                            )?;
                            rename_to_bak(&path);
                        }
                    }
                }
            }
        }
    }

    // 4. Photos (map_storage/photos/*.json)
    let photo_dir = storage_root.join("photos");
    if photo_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&photo_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_active_json(&path) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(photo) = serde_json::from_str::<PhotoMetadata>(&content) {
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
                            )?;
                            rename_to_bak(&path);
                        }
                    }
                }
            }
        }
    }

    // 5. Spatial Results (map_storage/spatial_results/)
    let sr_dir = storage_root.join("spatial_results");
    if sr_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&sr_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // 5a. Decomposed Folder: {layer_id}/layer.json + features/*.json
                if path.is_dir() {
                    let folder_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if folder_name.starts_with('.') || folder_name.ends_with(".bak") {
                        continue;
                    }

                    let layer_file = path.join("layer.json");
                    if layer_file.is_file() {
                        if let Ok(layer_raw) = fs::read_to_string(&layer_file) {
                            if let Ok(layer_meta) =
                                serde_json::from_str::<StoredLayerFile>(&layer_raw)
                            {
                                let disp_pref_str =
                                    serde_json::to_string(&layer_meta.display_preference)
                                        .unwrap_or_else(|_| "{}".to_string());
                                let updated_at = layer_meta
                                    .updated_at
                                    .clone()
                                    .unwrap_or_else(|| now_str.clone());

                                conn.execute(
                                    "INSERT INTO sp_result (
                                        layer_id, project_id, session_id, layer_name,
                                        display_preference, feature_count, concurrency_token,
                                        extra, created_at, updated_at
                                    ) VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, '{}', ?6, ?7)
                                    ON CONFLICT(layer_id) DO UPDATE SET
                                        layer_name = excluded.layer_name,
                                        display_preference = excluded.display_preference,
                                        feature_count = excluded.feature_count,
                                        concurrency_token = excluded.concurrency_token,
                                        updated_at = excluded.updated_at",
                                    params![
                                        layer_meta.id,
                                        layer_meta.layer_name,
                                        disp_pref_str,
                                        layer_meta.feature_count as i64,
                                        layer_meta.updated_at.as_deref(),
                                        updated_at,
                                        now_str,
                                    ],
                                )?;

                                let features_dir = path.join("features");
                                if features_dir.is_dir() {
                                    if let Ok(feat_entries) = fs::read_dir(&features_dir) {
                                        for (idx, feat_entry) in feat_entries.flatten().enumerate()
                                        {
                                            let feat_path = feat_entry.path();
                                            if is_active_json(&feat_path) {
                                                if let Ok(feat_raw) = fs::read_to_string(&feat_path)
                                                {
                                                    if let Ok(feat_file) =
                                                        serde_json::from_str::<StoredFeatureFile>(
                                                            &feat_raw,
                                                        )
                                                    {
                                                        let f = feat_file.result;
                                                        let geom_str =
                                                            serde_json::to_string(&f.geometry)
                                                                .unwrap_or_else(|_| {
                                                                    "{}".to_string()
                                                                });
                                                        let prop_str =
                                                            serde_json::to_string(&f.properties)
                                                                .unwrap_or_else(|_| {
                                                                    "{}".to_string()
                                                                });
                                                        let edited_str =
                                                            f.edited_fields.as_ref().and_then(
                                                                |ef| serde_json::to_string(ef).ok(),
                                                            );
                                                        let extra_str =
                                                            serde_json::to_string(&f.extra)
                                                                .unwrap_or_else(|_| {
                                                                    "{}".to_string()
                                                                });

                                                        let feature_id = format!(
                                                            "{}_{}",
                                                            layer_meta.id, feat_file.id
                                                        );

                                                        conn.execute(
                                                            "INSERT INTO sp_result_feature (
                                                                id, layer_id, feature_index, source,
                                                                geometry, properties, image_url, edited_fields, extra
                                                            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                                                            ON CONFLICT(id) DO UPDATE SET
                                                                source = excluded.source,
                                                                geometry = excluded.geometry,
                                                                properties = excluded.properties,
                                                                image_url = excluded.image_url,
                                                                edited_fields = excluded.edited_fields,
                                                                extra = excluded.extra",
                                                            params![
                                                                feature_id,
                                                                layer_meta.id,
                                                                idx as i64,
                                                                f.source,
                                                                geom_str,
                                                                prop_str,
                                                                f.image_url,
                                                                edited_str,
                                                                extra_str,
                                                            ],
                                                        )?;
                                                        rename_to_bak(&feat_path);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                rename_to_bak(&layer_file);
                            }
                        }
                    }
                }
                // 5b. Flat File: {layer_id}.json
                else if is_active_json(&path) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(sr) = serde_json::from_str::<LayerPayload>(&content) {
                            let disp_pref_str = serde_json::to_string(&sr.display_preference)
                                .unwrap_or_else(|_| "{}".to_string());
                            let updated_at =
                                sr.updated_at.clone().unwrap_or_else(|| now_str.clone());
                            let extra_str = serde_json::to_string(&sr.extra)
                                .unwrap_or_else(|_| "{}".to_string());

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
                                    sr.layer_id,
                                    sr.layer_name,
                                    disp_pref_str,
                                    sr.spatial_results.len() as i64,
                                    sr.concurrency_token.as_deref(),
                                    extra_str,
                                    updated_at,
                                    now_str,
                                ],
                            )?;

                            for (idx, feat) in sr.spatial_results.iter().enumerate() {
                                let feat_id = get_feature_id(feat, idx);
                                let scoped_id = format!("{}_{}", sr.layer_id, feat_id);
                                let geom_str = serde_json::to_string(&feat.geometry)
                                    .unwrap_or_else(|_| "{}".to_string());
                                let prop_str = serde_json::to_string(&feat.properties)
                                    .unwrap_or_else(|_| "{}".to_string());
                                let edited_str = feat
                                    .edited_fields
                                    .as_ref()
                                    .and_then(|ef| serde_json::to_string(ef).ok());
                                let extra_str = serde_json::to_string(&feat.extra)
                                    .unwrap_or_else(|_| "{}".to_string());

                                conn.execute(
                                    "INSERT INTO sp_result_feature (
                                        id, layer_id, feature_index, source,
                                        geometry, properties, image_url, edited_fields, extra
                                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                                    ON CONFLICT(id) DO UPDATE SET
                                        source = excluded.source,
                                        geometry = excluded.geometry,
                                        properties = excluded.properties,
                                        image_url = excluded.image_url,
                                        edited_fields = excluded.edited_fields,
                                        extra = excluded.extra",
                                    params![
                                        scoped_id,
                                        sr.layer_id,
                                        idx as i64,
                                        feat.source,
                                        geom_str,
                                        prop_str,
                                        feat.image_url,
                                        edited_str,
                                        extra_str,
                                    ],
                                )?;
                            }

                            rename_to_bak(&path);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn is_active_json(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if file_name.starts_with('.') || file_name.ends_with(".bak") {
        return false;
    }
    path.extension().and_then(|s| s.to_str()) == Some("json")
}

fn rename_to_bak(path: &Path) {
    let bak_path = path.with_extension("json.bak");
    let _ = fs::rename(path, bak_path);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        crate::modules::module_01::init_schema(&conn).expect("init module 01 schema");
        migrate_map_schema(&conn).expect("migrate map schema");
        conn
    }

    #[test]
    fn test_migrate_map_schema_is_idempotent() {
        let conn = memory_db();
        // Running it a second time on the same connection must not error
        assert!(migrate_map_schema(&conn).is_ok());
    }

    #[test]
    fn test_migrate_legacy_json_to_sqlite_imports_and_renames() {
        let conn = memory_db();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let aoi_dir = temp_dir.path().join("aois");
        fs::create_dir_all(&aoi_dir).expect("create aois dir");

        let sample_aoi = r#"{
            "type": "Feature",
            "properties": {
                "aoi_id": "test_aoi_001",
                "name": "Test Kebun",
                "aoi_type": "Kelapa Sawit",
                "calculated_area_sq_m": 5000.0,
                "perimeter_m": 300.0,
                "created_at": "2026-11-15T12:00:00Z"
            },
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[116.85, -1.22], [116.86, -1.22], [116.86, -1.23], [116.85, -1.22]]]
            }
        }"#;

        let aoi_file = aoi_dir.join("test_aoi_001.json");
        fs::write(&aoi_file, sample_aoi).expect("write sample aoi");

        // Run migration
        migrate_legacy_json_to_sqlite(&conn, temp_dir.path()).expect("migrate");

        // Verify inserted into SQLite
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM aoi WHERE id = 'test_aoi_001'",
                [],
                |r| r.get(0),
            )
            .expect("query aoi");
        assert_eq!(count, 1);

        // Verify renamed to .bak
        assert!(!aoi_file.exists());
        assert!(aoi_dir.join("test_aoi_001.json.bak").exists());

        // Run migration again — must remain idempotent and not error
        migrate_legacy_json_to_sqlite(&conn, temp_dir.path()).expect("re-migrate");
        let count_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM aoi WHERE id = 'test_aoi_001'",
                [],
                |r| r.get(0),
            )
            .expect("query aoi");
        assert_eq!(count_after, 1);
    }
}
