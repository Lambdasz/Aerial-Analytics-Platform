//! Persisted before/after comparisons (Module 10).
//!
//! Each successful session comparison stores one row so the page can list
//! earlier results and the dashboard can read the latest one without
//! recomputing pixel coverage. Additive table, migrated on startup.
//!
//! # Purity
//!
//! `magnitude_level_name` is pure; the read/write helpers are impure and take
//! an open SQLite connection.

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::analysis::MagnitudeLevel;

/// DDL for the comparison history table.
const COMPARISON_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS temporal_comparison (
    id                 TEXT PRIMARY KEY,
    project_id         TEXT,
    before_session_id  TEXT NOT NULL,
    after_session_id   TEXT NOT NULL,
    before_image_id    TEXT NOT NULL,
    after_image_id     TEXT NOT NULL,
    before_date        TEXT NOT NULL,
    after_date         TEXT NOT NULL,
    interval_days      INTEGER NOT NULL,
    metric             TEXT NOT NULL,
    before_value       REAL NOT NULL,
    after_value        REAL NOT NULL,
    delta              REAL NOT NULL,
    magnitude          REAL NOT NULL,
    magnitude_level    TEXT NOT NULL,
    created_at         TEXT NOT NULL
)";

const COMPARISON_COLUMNS: &str = "id, project_id, before_session_id, after_session_id, \
    before_image_id, after_image_id, before_date, after_date, interval_days, metric, \
    before_value, after_value, delta, magnitude, magnitude_level, created_at";

/// One stored comparison row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub before_session_id: String,
    pub after_session_id: String,
    pub before_image_id: String,
    pub after_image_id: String,
    pub before_date: String,
    pub after_date: String,
    pub interval_days: i64,
    /// Metric key, e.g. `vegetation_coverage_percent`.
    pub metric: String,
    pub before_value: f64,
    pub after_value: f64,
    pub delta: f64,
    pub magnitude: f64,
    /// `rendah` | `sedang` | `tinggi`.
    pub magnitude_level: String,
    pub created_at: String,
}

/// Creates the comparison table when missing. Additive migration.
pub fn migrate_comparison_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(COMPARISON_TABLE_DDL, [])?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_temporal_comparison_project ON temporal_comparison(project_id)",
        [],
    )?;
    Ok(())
}

/// Contract spelling of a magnitude level. Pure.
pub fn magnitude_level_name(level: MagnitudeLevel) -> &'static str {
    match level {
        MagnitudeLevel::Low => "rendah",
        MagnitudeLevel::Medium => "sedang",
        MagnitudeLevel::High => "tinggi",
    }
}

/// Stored comparison fields for one metric of one session pair.
#[derive(Debug, Clone)]
pub struct NewComparison {
    pub project_id: Option<String>,
    pub before_session_id: String,
    pub after_session_id: String,
    pub before_image_id: String,
    pub after_image_id: String,
    pub before_date: String,
    pub after_date: String,
    pub interval_days: i64,
    pub metric: String,
    pub before_value: f64,
    pub after_value: f64,
    pub delta: f64,
    pub magnitude: f64,
    pub magnitude_level: String,
}

fn row_to_comparison(row: &Row) -> rusqlite::Result<ComparisonRecord> {
    Ok(ComparisonRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        before_session_id: row.get(2)?,
        after_session_id: row.get(3)?,
        before_image_id: row.get(4)?,
        after_image_id: row.get(5)?,
        before_date: row.get(6)?,
        after_date: row.get(7)?,
        interval_days: row.get(8)?,
        metric: row.get(9)?,
        before_value: row.get(10)?,
        after_value: row.get(11)?,
        delta: row.get(12)?,
        magnitude: row.get(13)?,
        magnitude_level: row.get(14)?,
        created_at: row.get(15)?,
    })
}

/// Inserts one comparison row. Impure.
pub fn insert_comparison(
    conn: &Connection,
    entry: &NewComparison,
) -> Result<ComparisonRecord, rusqlite::Error> {
    let record = ComparisonRecord {
        id: Uuid::new_v4().to_string(),
        project_id: entry.project_id.clone(),
        before_session_id: entry.before_session_id.clone(),
        after_session_id: entry.after_session_id.clone(),
        before_image_id: entry.before_image_id.clone(),
        after_image_id: entry.after_image_id.clone(),
        before_date: entry.before_date.clone(),
        after_date: entry.after_date.clone(),
        interval_days: entry.interval_days,
        metric: entry.metric.clone(),
        before_value: entry.before_value,
        after_value: entry.after_value,
        delta: entry.delta,
        magnitude: entry.magnitude,
        magnitude_level: entry.magnitude_level.clone(),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    };
    conn.execute(
        &format!(
            "INSERT INTO temporal_comparison ({COMPARISON_COLUMNS}) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"
        ),
        params![
            record.id,
            record.project_id,
            record.before_session_id,
            record.after_session_id,
            record.before_image_id,
            record.after_image_id,
            record.before_date,
            record.after_date,
            record.interval_days,
            record.metric,
            record.before_value,
            record.after_value,
            record.delta,
            record.magnitude,
            record.magnitude_level,
            record.created_at,
        ],
    )?;
    Ok(record)
}

/// Lists comparisons newest first, optionally for one project. Impure.
pub fn list_comparisons(
    conn: &Connection,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<ComparisonRecord>, rusqlite::Error> {
    let limit = limit.max(1) as i64;
    let mut statement = match project_id {
        Some(_) => conn.prepare(&format!(
            "SELECT {COMPARISON_COLUMNS} FROM temporal_comparison \
             WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2"
        ))?,
        None => conn.prepare(&format!(
            "SELECT {COMPARISON_COLUMNS} FROM temporal_comparison \
             ORDER BY created_at DESC LIMIT ?1"
        ))?,
    };
    let rows = match project_id {
        Some(id) => statement
            .query_map(params![id, limit], row_to_comparison)?
            .collect::<Result<Vec<_>, _>>()?,
        None => statement
            .query_map(params![limit], row_to_comparison)?
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(rows)
}

/// Fetches one comparison by id. Impure.
pub fn get_comparison(
    conn: &Connection,
    comparison_id: &str,
) -> Result<Option<ComparisonRecord>, rusqlite::Error> {
    conn.query_row(
        &format!("SELECT {COMPARISON_COLUMNS} FROM temporal_comparison WHERE id = ?1"),
        params![comparison_id],
        row_to_comparison,
    )
    .optional()
}

/// Deletes comparisons that reference a session. Impure.
pub fn delete_for_session(conn: &Connection, session_id: &str) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM temporal_comparison WHERE before_session_id = ?1 OR after_session_id = ?1",
        params![session_id],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate_comparison_schema(&conn).unwrap();
        conn
    }

    fn sample(project: Option<&str>, delta: f64) -> NewComparison {
        NewComparison {
            project_id: project.map(str::to_string),
            before_session_id: "s-before".into(),
            after_session_id: "s-after".into(),
            before_image_id: "img-b".into(),
            after_image_id: "img-a".into(),
            before_date: "2026-01-10".into(),
            after_date: "2026-02-10".into(),
            interval_days: 31,
            metric: "vegetation_coverage_percent".into(),
            before_value: 60.0,
            after_value: 60.0 + delta,
            delta,
            magnitude: delta.abs(),
            magnitude_level: magnitude_level_name(MagnitudeLevel::Medium).to_string(),
        }
    }

    #[test]
    fn insert_and_list_roundtrip() {
        let conn = memory_db();
        let first = insert_comparison(&conn, &sample(Some("p1"), 12.0)).unwrap();
        insert_comparison(&conn, &sample(Some("p1"), -3.0)).unwrap();
        insert_comparison(&conn, &sample(Some("p2"), 1.0)).unwrap();

        let listed = list_comparisons(&conn, Some("p1"), 10).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed[0].created_at >= listed[1].created_at);
        assert_eq!(first.metric, "vegetation_coverage_percent");
        assert_eq!(first.interval_days, 31);

        // Limit applies.
        assert_eq!(list_comparisons(&conn, Some("p1"), 1).unwrap().len(), 1);
        // Project filter applies.
        assert_eq!(list_comparisons(&conn, Some("nope"), 10).unwrap().len(), 0);
        assert_eq!(list_comparisons(&conn, None, 10).unwrap().len(), 3);
    }

    #[test]
    fn delete_for_session_removes_both_sides() {
        let conn = memory_db();
        insert_comparison(&conn, &sample(Some("p1"), 5.0)).unwrap();
        assert_eq!(delete_for_session(&conn, "s-after").unwrap(), 1);
        assert!(list_comparisons(&conn, None, 10).unwrap().is_empty());
        assert_eq!(delete_for_session(&conn, "s-before").unwrap(), 0);
    }

    #[test]
    fn magnitude_levels_spell_out_for_the_contract() {
        assert_eq!(magnitude_level_name(MagnitudeLevel::Low), "rendah");
        assert_eq!(magnitude_level_name(MagnitudeLevel::Medium), "sedang");
        assert_eq!(magnitude_level_name(MagnitudeLevel::High), "tinggi");
    }
}
