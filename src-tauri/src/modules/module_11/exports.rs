//! Module 11 — export writers (CSV, GeoJSON, JSON snapshot).
//!
//! Every writer takes the destination directory explicitly so the command
//! layer decides where exports live (`map_storage/exports/`), keeping these
//! functions testable with a tempdir.
//!
//! # Purity
//!
//! Impure: they read the database and write files.

use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::{json, Map, Value};

use super::dashboard::{build_project_dashboard, list_plugin_results};

/// Escapes one CSV cell (quotes doubled, wrapped when needed). Pure.
pub fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn timestamped(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    dir.join(format!("{stem}_{stamp}.{extension}"))
}

/// Writes plot indicators as CSV. Returns the written path.
pub fn write_plot_indicators_csv(
    conn: &Connection,
    project_id: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot create export dir: {error}"))?;
    let mut statement = conn
        .prepare(
            "SELECT p.name, i.plugin_id, i.indicator, i.unit, i.value, i.text_value, i.created_at \
             FROM plot_indicator i JOIN plot p ON p.id = i.plot_id \
             WHERE p.project_id = ?1 OR p.project_id IS NULL \
             ORDER BY p.name, i.plugin_id, i.indicator",
        )
        .map_err(|error| error.to_string())?;

    let mut csv = String::from("plot,plugin,indicator,unit,value,text_value,created_at\n");
    let rows = statement
        .query_map(rusqlite::params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<f64>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|error| error.to_string())?;

    for row in rows {
        let (plot, plugin, indicator, unit, value, text, created_at) =
            row.map_err(|error| error.to_string())?;
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            csv_cell(&plot),
            csv_cell(&plugin),
            csv_cell(&indicator),
            csv_cell(unit.as_deref().unwrap_or("")),
            value.map(|value| value.to_string()).unwrap_or_default(),
            csv_cell(text.as_deref().unwrap_or("")),
            csv_cell(&created_at),
        ));
    }

    let path = timestamped(dir, "plot_indicators", "csv");
    std::fs::write(&path, csv).map_err(|error| format!("cannot write CSV: {error}"))?;
    Ok(path)
}

/// Writes the project's plots as GeoJSON. Returns the written path.
pub fn write_plots_geojson(
    conn: &Connection,
    project_id: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot create export dir: {error}"))?;
    let mut statement = conn
        .prepare(
            "SELECT name, geometry, area_m2, perimeter_m FROM plot \
             WHERE project_id = ?1 OR project_id IS NULL ORDER BY name",
        )
        .map_err(|error| error.to_string())?;

    let features = statement
        .query_map(rusqlite::params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|(name, geometry, area_m2, perimeter_m)| {
            let geometry: Value = serde_json::from_str(&geometry).ok()?;
            let mut properties = Map::new();
            properties.insert("name".into(), json!(name));
            properties.insert("area_m2".into(), json!(area_m2));
            properties.insert("area_ha".into(), json!(area_m2 / 10_000.0));
            properties.insert("perimeter_m".into(), json!(perimeter_m));
            Some(json!({
                "type": "Feature",
                "geometry": geometry,
                "properties": properties,
            }))
        })
        .collect::<Vec<_>>();

    let collection = json!({ "type": "FeatureCollection", "features": features });
    let path = timestamped(dir, "plots", "geojson");
    let text = serde_json::to_string_pretty(&collection).map_err(|error| error.to_string())?;
    std::fs::write(&path, text).map_err(|error| format!("cannot write GeoJSON: {error}"))?;
    Ok(path)
}

/// Writes the dashboard plus explorer rows as a JSON snapshot.
pub fn write_dashboard_json(
    conn: &Connection,
    project_id: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot create export dir: {error}"))?;
    let dashboard = build_project_dashboard(conn, project_id).map_err(|error| error.to_string())?;
    let results = list_plugin_results(conn, project_id, 500).map_err(|error| error.to_string())?;
    let snapshot = json!({
        "exported_at": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "dashboard": dashboard,
        "plugin_results": results,
    });
    let path = timestamped(dir, "dashboard", "json");
    let text = serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())?;
    std::fs::write(&path, text).map_err(|error| format!("cannot write snapshot: {error}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_cells_escape_quotes_and_commas() {
        assert_eq!(csv_cell("plain"), "plain");
        assert_eq!(csv_cell("a,b"), "\"a,b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("two\nlines"), "\"two\nlines\"");
    }

    #[test]
    fn writers_produce_files() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::modules::module_09::plots::migrate_plot_schema(&conn).unwrap();
        crate::modules::module_09::indicators::migrate_indicator_schema(&conn).unwrap();
        crate::modules::module_01::flight_session::migrate_image_schema(&conn).unwrap();
        crate::modules::module_10::comparisons::migrate_comparison_schema(&conn).unwrap();
        crate::plugin_manager::result_store::migrate_result_schema(&conn).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS session (
                id TEXT PRIMARY KEY, project_id TEXT, name TEXT, date_start TEXT, date_end TEXT,
                status TEXT, created_at TEXT, updated_at TEXT)",
            [],
        )
        .unwrap();

        let csv = write_plot_indicators_csv(&conn, "p1", dir.path()).unwrap();
        assert!(csv.exists());
        assert!(std::fs::read_to_string(&csv)
            .unwrap()
            .starts_with("plot,plugin,"));

        let geojson = write_plots_geojson(&conn, "p1", dir.path()).unwrap();
        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&geojson).unwrap()).unwrap();
        assert_eq!(parsed["type"], "FeatureCollection");

        let snapshot = write_dashboard_json(&conn, "p1", dir.path()).unwrap();
        let parsed: Value =
            serde_json::from_str(&std::fs::read_to_string(&snapshot).unwrap()).unwrap();
        assert_eq!(parsed["dashboard"]["projectId"], "p1");
    }
}
