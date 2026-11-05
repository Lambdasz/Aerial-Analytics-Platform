//! Module 11 — aggregation queries for the analytics dashboard.
//!
//! Everything here is **read-only**: the dashboard summarises what Modules 1,
//! 4-6, 9 and 10 already stored. Nothing is recomputed and no table is written.
//!
//! # Purity
//!
//! All functions take an open connection and only run `SELECT`s.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::modules::module_09::indicators::default_unit;
use crate::modules::module_10::comparisons::{list_comparisons, ComparisonRecord};

/// Aggregate of one indicator across a project's plots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorAggregate {
    pub indicator: String,
    pub unit: Option<String>,
    pub plots: usize,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}

/// One plugin and how many images it has results for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginUsage {
    pub plugin_id: String,
    pub images: usize,
    pub last_run_at: Option<String>,
}

/// Everything the project dashboard shows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDashboard {
    pub project_id: String,
    pub sessions: usize,
    pub images: usize,
    pub geotagged_images: usize,
    pub plots: usize,
    pub plot_area_ha: f64,
    pub plugin_results: usize,
    pub indicator_aggregates: Vec<IndicatorAggregate>,
    pub plugins: Vec<PluginUsage>,
    pub latest_comparison: Option<ComparisonRecord>,
}

/// One metric of one plugin run on one image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginResultRow {
    pub image_id: String,
    pub file_path: String,
    pub plugin_id: String,
    pub metric: String,
    pub unit: Option<String>,
    pub value: Option<f64>,
    pub text_value: Option<String>,
    pub created_at: String,
}

fn count(conn: &Connection, sql: &str, project_id: &str) -> rusqlite::Result<usize> {
    conn.query_row(sql, params![project_id], |row| {
        let value: i64 = row.get(0)?;
        Ok(value.max(0) as usize)
    })
}

/// Builds the project dashboard from stored data. Impure (reads).
pub fn build_project_dashboard(
    conn: &Connection,
    project_id: &str,
) -> rusqlite::Result<ProjectDashboard> {
    let sessions = count(
        conn,
        "SELECT COUNT(*) FROM session WHERE project_id = ?1",
        project_id,
    )?;
    let images = count(
        conn,
        "SELECT COUNT(*) FROM image i JOIN session s ON s.id = i.session_id \
         WHERE s.project_id = ?1",
        project_id,
    )?;
    let geotagged_images = count(
        conn,
        "SELECT COUNT(*) FROM image i JOIN session s ON s.id = i.session_id \
         WHERE s.project_id = ?1 AND i.location_lat IS NOT NULL AND i.location_lon IS NOT NULL",
        project_id,
    )?;

    // Plots keep the Module 9 rule: project matches or is unset.
    let (plots, plot_area_m2): (usize, f64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(area_m2), 0) FROM plot \
         WHERE project_id = ?1 OR project_id IS NULL",
        params![project_id],
        |row| {
            let plots: i64 = row.get(0)?;
            Ok((plots.max(0) as usize, row.get::<_, f64>(1)?))
        },
    )?;

    let plugin_results = count(
        conn,
        "SELECT COUNT(*) FROM plugin_image_result r \
         JOIN image i ON i.id = r.image_id \
         JOIN session s ON s.id = i.session_id \
         WHERE s.project_id = ?1",
        project_id,
    )?;

    let mut indicator_statement = conn.prepare(
        "SELECT i.indicator, i.unit, COUNT(DISTINCT i.plot_id), AVG(i.value), MIN(i.value), \
                MAX(i.value) \
         FROM plot_indicator i JOIN plot p ON p.id = i.plot_id \
         WHERE (p.project_id = ?1 OR p.project_id IS NULL) AND i.value IS NOT NULL \
         GROUP BY i.indicator, i.unit ORDER BY i.indicator",
    )?;
    let indicator_aggregates = indicator_statement
        .query_map(params![project_id], |row| {
            Ok(IndicatorAggregate {
                indicator: row.get(0)?,
                unit: row.get(1)?,
                plots: row.get::<_, i64>(2)?.max(0) as usize,
                mean: row.get(3)?,
                min: row.get(4)?,
                max: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut plugin_statement = conn.prepare(
        "SELECT r.plugin_id, COUNT(*), MAX(r.created_at) FROM plugin_image_result r \
         JOIN image i ON i.id = r.image_id \
         JOIN session s ON s.id = i.session_id \
         WHERE s.project_id = ?1 \
         GROUP BY r.plugin_id ORDER BY r.plugin_id",
    )?;
    let plugins = plugin_statement
        .query_map(params![project_id], |row| {
            Ok(PluginUsage {
                plugin_id: row.get(0)?,
                images: row.get::<_, i64>(1)?.max(0) as usize,
                last_run_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let latest_comparison = list_comparisons(conn, Some(project_id), 1)?
        .into_iter()
        .next();

    Ok(ProjectDashboard {
        project_id: project_id.to_string(),
        sessions,
        images,
        geotagged_images,
        plots,
        plot_area_ha: plot_area_m2 / 10_000.0,
        plugin_results,
        indicator_aggregates,
        plugins,
        latest_comparison,
    })
}

/// Flattens the stored metric objects into explorer rows, newest first. Impure.
pub fn list_plugin_results(
    conn: &Connection,
    project_id: &str,
    limit: usize,
) -> rusqlite::Result<Vec<PluginResultRow>> {
    let limit = limit.clamp(1, 500) as i64;
    let mut statement = conn.prepare(
        "SELECT r.image_id, i.file_path, r.plugin_id, r.metrics, r.created_at \
         FROM plugin_image_result r \
         JOIN image i ON i.id = r.image_id \
         JOIN session s ON s.id = i.session_id \
         WHERE s.project_id = ?1 \
         ORDER BY r.created_at DESC LIMIT ?2",
    )?;
    let runs = statement
        .query_map(params![project_id, limit], |row| {
            let metrics: String = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                metrics,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut rows = Vec::new();
    for (image_id, file_path, plugin_id, metrics, created_at) in runs {
        let parsed: Value = serde_json::from_str(&metrics).unwrap_or(Value::Null);
        let Some(map) = parsed.as_object() else {
            continue;
        };
        for (metric, value) in map {
            let (number, text) = match value {
                Value::Number(number) => (number.as_f64(), None),
                Value::String(text) => (None, Some(text.clone())),
                Value::Bool(flag) => (Some(f64::from(u8::from(*flag))), Some(flag.to_string())),
                _ => continue,
            };
            rows.push(PluginResultRow {
                image_id: image_id.clone(),
                file_path: file_path.clone(),
                plugin_id: plugin_id.clone(),
                metric: metric.clone(),
                unit: default_unit(metric).map(str::to_string),
                value: number,
                text_value: text,
                created_at: created_at.clone(),
            });
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::modules::module_01::flight_session::migrate_image_schema(&conn).unwrap();
        crate::modules::module_09::plots::migrate_plot_schema(&conn).unwrap();
        crate::modules::module_09::indicators::migrate_indicator_schema(&conn).unwrap();
        crate::modules::module_10::comparisons::migrate_comparison_schema(&conn).unwrap();
        crate::plugin_manager::result_store::migrate_result_schema(&conn).unwrap();

        // Minimal session + image rows without touching Module 1's schema.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS project (id TEXT PRIMARY KEY, name TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project (id, name) VALUES ('p1', 'Project')",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS session (
                id TEXT PRIMARY KEY, project_id TEXT, name TEXT, date_start TEXT, date_end TEXT,
                status TEXT, created_at TEXT, updated_at TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session VALUES ('s1','p1','Session','2026-01-01T00:00:00','2026-01-01T00:00:00','active','2026-01-01T00:00:00','2026-01-01T00:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at, heading_deg)
             VALUES ('img1','s1','/tmp/a.jpg', 1.0, 2.0, '2026-01-01T00:00:00', 0.0)",
            [],
        )
        .unwrap();
        let polygon = crate::modules::module_09::plots::polygon_from_geometry(&json!({
            "type": "Polygon",
            "coordinates": [[[0.0, 0.0], [0.002, 0.0], [0.002, 0.002], [0.0, 0.002]]]
        }))
        .unwrap();
        let plot = crate::modules::module_09::plots::insert_plot(
            &conn,
            Some("p1".into()),
            None,
            "Plot A",
            &polygon,
            None,
        )
        .unwrap();
        crate::modules::module_09::indicators::save_indicators(
            &conn,
            &plot.id,
            "vegetation-coverage-analytics",
            None,
            &[
                crate::modules::module_09::indicators::IndicatorInput {
                    key: "coverage_percentage".into(),
                    unit: Some("%".into()),
                    value: json!(40.0),
                },
                crate::modules::module_09::indicators::IndicatorInput {
                    key: "classification".into(),
                    unit: None,
                    value: json!("medium"),
                },
            ],
        )
        .unwrap();
        crate::plugin_manager::result_store::upsert_result(
            &conn,
            "img1",
            Some("s1"),
            "vegetation-coverage-analytics",
            Some("exec-1"),
            &json!({ "coverage_percentage": 43.1, "classification": "medium" }),
            &json!({}),
        )
        .unwrap();
        conn
    }

    #[test]
    fn dashboard_totals_and_aggregates() {
        let conn = memory_db();
        let dashboard = build_project_dashboard(&conn, "p1").unwrap();
        assert_eq!(dashboard.sessions, 1);
        assert_eq!(dashboard.images, 1);
        assert_eq!(dashboard.geotagged_images, 1);
        assert_eq!(dashboard.plots, 1);
        assert!(dashboard.plot_area_ha > 0.0);
        assert_eq!(dashboard.plugin_results, 1);
        // Only the numeric indicator aggregates; text values are skipped.
        assert_eq!(dashboard.indicator_aggregates.len(), 1);
        let coverage = &dashboard.indicator_aggregates[0];
        assert_eq!(coverage.indicator, "coverage_percentage");
        assert!((coverage.mean - 40.0).abs() < 1e-9);
        assert_eq!(dashboard.plugins.len(), 1);
        assert_eq!(dashboard.plugins[0].images, 1);
        assert!(dashboard.latest_comparison.is_none());
    }

    #[test]
    fn explorer_rows_flatten_metrics_with_units() {
        let conn = memory_db();
        let rows = list_plugin_results(&conn, "p1", 50).unwrap();
        assert_eq!(rows.len(), 2);
        let coverage = rows
            .iter()
            .find(|row| row.metric == "coverage_percentage")
            .unwrap();
        assert_eq!(coverage.unit.as_deref(), Some("%"));
        assert_eq!(coverage.value, Some(43.1));
        assert_eq!(coverage.image_id, "img1");
        let classification = rows
            .iter()
            .find(|row| row.metric == "classification")
            .unwrap();
        assert_eq!(classification.text_value.as_deref(), Some("medium"));
        assert!(list_plugin_results(&conn, "other", 50).unwrap().is_empty());
    }
}
