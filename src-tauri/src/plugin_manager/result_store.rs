//! Per-image plugin result store.
//!
//! Downstream modules (Module 10 temporal change, Module 11 dashboards) must
//! read what plugins already measured instead of recomputing it. When a run is
//! published, its metrics and artifact paths are stored here keyed by
//! `(image_id, plugin_id)`, so "latest value for this image" is a single query.
//!
//! # Purity
//!
//! The read/write helpers are impure and take an open SQLite connection.

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

/// DDL for the result table. Additive migration, run on startup.
const RESULT_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS plugin_image_result (
    id           TEXT PRIMARY KEY,
    image_id     TEXT NOT NULL,
    session_id   TEXT,
    plugin_id    TEXT NOT NULL,
    execution_id TEXT,
    metrics      TEXT NOT NULL,
    artifacts    TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    UNIQUE(image_id, plugin_id)
)";

const RESULT_COLUMNS: &str =
    "id, image_id, session_id, plugin_id, execution_id, metrics, artifacts, created_at";

/// One stored plugin run for one image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImagePluginResult {
    pub id: String,
    pub image_id: String,
    pub session_id: Option<String>,
    pub plugin_id: String,
    pub execution_id: Option<String>,
    /// Raw plugin metrics object.
    pub metrics: Value,
    /// Artifact name -> artifact object (`file_path`, `format`, `bounds?`).
    pub artifacts: Value,
    pub created_at: String,
}

/// Creates the table when missing. Additive migration.
pub fn migrate_result_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(RESULT_TABLE_DDL, [])?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_plugin_image_result_image ON plugin_image_result(image_id)",
        [],
    )?;
    Ok(())
}

fn row_to_result(row: &Row) -> rusqlite::Result<ImagePluginResult> {
    let metrics: String = row.get(5)?;
    let artifacts: String = row.get(6)?;
    Ok(ImagePluginResult {
        id: row.get(0)?,
        image_id: row.get(1)?,
        session_id: row.get(2)?,
        plugin_id: row.get(3)?,
        execution_id: row.get(4)?,
        metrics: serde_json::from_str(&metrics).unwrap_or(Value::Object(Map::new())),
        artifacts: serde_json::from_str(&artifacts).unwrap_or(Value::Object(Map::new())),
        created_at: row.get(7)?,
    })
}

/// Stores (or refreshes) the latest run of one plugin for one image. Impure.
pub fn upsert_result(
    conn: &Connection,
    image_id: &str,
    session_id: Option<&str>,
    plugin_id: &str,
    execution_id: Option<&str>,
    metrics: &Value,
    artifacts: &Value,
) -> Result<ImagePluginResult, rusqlite::Error> {
    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let record = ImagePluginResult {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.to_string(),
        session_id: session_id.map(str::to_string),
        plugin_id: plugin_id.to_string(),
        execution_id: execution_id.map(str::to_string),
        metrics: metrics.clone(),
        artifacts: artifacts.clone(),
        created_at,
    };
    conn.execute(
        "INSERT INTO plugin_image_result (id, image_id, session_id, plugin_id, execution_id, metrics, artifacts, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(image_id, plugin_id) DO UPDATE SET
            session_id = excluded.session_id,
            execution_id = excluded.execution_id,
            metrics = excluded.metrics,
            artifacts = excluded.artifacts,
            created_at = excluded.created_at",
        params![
            record.id,
            record.image_id,
            record.session_id,
            record.plugin_id,
            record.execution_id,
            serde_json::to_string(&record.metrics).unwrap_or_else(|_| "{}".to_string()),
            serde_json::to_string(&record.artifacts).unwrap_or_else(|_| "{}".to_string()),
            record.created_at,
        ],
    )?;
    latest_for_image(conn, image_id)?
        .into_iter()
        .next()
        .ok_or(rusqlite::Error::QueryReturnedNoRows)
}

/// Every stored result for one image, newest first. Impure.
pub fn latest_for_image(
    conn: &Connection,
    image_id: &str,
) -> Result<Vec<ImagePluginResult>, rusqlite::Error> {
    let mut statement = conn.prepare(&format!(
        "SELECT {RESULT_COLUMNS} FROM plugin_image_result WHERE image_id = ?1 \
         ORDER BY created_at DESC"
    ))?;
    let rows = statement
        .query_map(params![image_id], row_to_result)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Latest stored metric value for one image, searched across plugins by key
/// priority. Returns `(value, plugin_id)`. Impure.
pub fn latest_metric(
    conn: &Connection,
    image_id: &str,
    keys: &[&str],
) -> Result<Option<(f64, String)>, rusqlite::Error> {
    for result in latest_for_image(conn, image_id)? {
        for key in keys {
            if let Some(value) = result.metrics.get(*key).and_then(Value::as_f64) {
                return Ok(Some((value, result.plugin_id)));
            }
        }
    }
    Ok(None)
}

/// Latest artifact path for one image whose key and format match. Impure.
pub fn latest_artifact(
    conn: &Connection,
    image_id: &str,
    keys: &[&str],
    format_prefix: &str,
) -> Result<Option<String>, rusqlite::Error> {
    for result in latest_for_image(conn, image_id)? {
        for key in keys {
            let Some(artifact) = result.artifacts.get(*key) else {
                continue;
            };
            let format = artifact
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !format.starts_with(format_prefix) {
                continue;
            }
            if let Some(path) = artifact.get("file_path").and_then(Value::as_str) {
                return Ok(Some(path.to_string()));
            }
        }
    }
    Ok(None)
}

/// Deletes results for an image (used when an image is removed). Impure.
pub fn delete_for_image(conn: &Connection, image_id: &str) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM plugin_image_result WHERE image_id = ?1",
        params![image_id],
    )
}

/// Optional lookup used by tests.
#[cfg(test)]
fn first(conn: &Connection, image_id: &str) -> Option<ImagePluginResult> {
    latest_for_image(conn, image_id).ok()?.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate_result_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn upsert_replaces_previous_run_metrics() {
        let conn = memory_db();
        upsert_result(
            &conn,
            "img-1",
            Some("s-1"),
            "vegetation-coverage-analytics",
            Some("exec-1"),
            &json!({ "coverage_percentage": 43.1, "classification": "medium" }),
            &json!({ "vegetation_mask": { "file_path": "/tmp/m.png", "format": "image/png" } }),
        )
        .unwrap();
        let stored = first(&conn, "img-1").unwrap();
        assert_eq!(stored.metrics["classification"], "medium");

        upsert_result(
            &conn,
            "img-1",
            Some("s-1"),
            "vegetation-coverage-analytics",
            Some("exec-2"),
            &json!({ "coverage_percentage": 51.7 }),
            &json!({}),
        )
        .unwrap();
        let listed = latest_for_image(&conn, "img-1").unwrap();
        assert_eq!(listed.len(), 1, "one row per image+plugin");
        assert_eq!(listed[0].execution_id.as_deref(), Some("exec-2"));
        assert_eq!(listed[0].metrics["coverage_percentage"], 51.7);
    }

    #[test]
    fn metric_lookup_prefers_earlier_keys() {
        let conn = memory_db();
        upsert_result(
            &conn,
            "img-1",
            None,
            "vegetation-coverage-analytics",
            None,
            &json!({ "coverage_percentage": 40.0 }),
            &json!({}),
        )
        .unwrap();
        upsert_result(
            &conn,
            "img-1",
            None,
            "vegetation-condition-analysis",
            None,
            &json!({ "vegetation_coverage_pct": 44.0 }),
            &json!({}),
        )
        .unwrap();

        let found = latest_metric(
            &conn,
            "img-1",
            &["coverage_percentage", "vegetation_coverage_pct"],
        )
        .unwrap()
        .unwrap();
        assert!((found.0 - 40.0).abs() < 1e-9);
        assert_eq!(found.1, "vegetation-coverage-analytics");

        let fallback = latest_metric(&conn, "img-1", &["missing_key"]).unwrap();
        assert!(fallback.is_none());
    }

    #[test]
    fn artifact_lookup_filters_by_format_and_key() {
        let conn = memory_db();
        upsert_result(
            &conn,
            "img-1",
            None,
            "rgb-vegetation-detection",
            None,
            &json!({}),
            &json!({
                "vegetation_mask": { "file_path": "/tmp/mask.png", "format": "image/png" },
                "detection_overlay": { "file_path": "/tmp/overlay.png", "format": "image/png" },
            }),
        )
        .unwrap();
        let mask = latest_artifact(&conn, "img-1", &["vegetation_mask"], "image/")
            .unwrap()
            .unwrap();
        assert_eq!(mask, "/tmp/mask.png");
        assert!(
            latest_artifact(&conn, "img-1", &["vegetation_mask"], "application/")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn delete_removes_rows() {
        let conn = memory_db();
        upsert_result(&conn, "img-1", None, "p", None, &json!({}), &json!({})).unwrap();
        assert_eq!(delete_for_image(&conn, "img-1").unwrap(), 1);
        assert!(latest_for_image(&conn, "img-1").unwrap().is_empty());
    }
}
