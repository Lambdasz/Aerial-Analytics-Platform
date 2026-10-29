//! Per-plot indicator storage (Module 9).
//!
//! Every plugin run executed for a plot stores its numeric (and text) metrics
//! here, keyed by `(plot_id, plugin_id, indicator)`. Keeping the latest value
//! per key makes ranking and comparison a plain query, and re-running a plugin
//! after changing thresholds updates the row instead of duplicating it.
//!
//! # Purity
//!
//! `normalise_indicators` is pure; the read/write helpers are impure and take
//! an open SQLite connection.

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::plots::PlotError;

/// DDL for the indicator table. Additive migration.
const INDICATOR_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS plot_indicator (
    id           TEXT PRIMARY KEY,
    plot_id      TEXT NOT NULL,
    plugin_id    TEXT NOT NULL,
    execution_id TEXT,
    indicator    TEXT NOT NULL,
    unit         TEXT,
    value        REAL,
    text_value   TEXT,
    created_at   TEXT NOT NULL,
    UNIQUE(plot_id, plugin_id, indicator)
)";

const INDICATOR_COLUMNS: &str =
    "plot_id, plugin_id, execution_id, indicator, unit, value, text_value, created_at";

/// One indicator value to store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorInput {
    pub key: String,
    #[serde(default)]
    pub unit: Option<String>,
    pub value: Value,
}

/// A stored indicator row (latest value per plot/plugin/key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorRecord {
    pub plot_id: String,
    pub plugin_id: String,
    pub execution_id: Option<String>,
    pub indicator: String,
    pub unit: Option<String>,
    pub value: Option<f64>,
    pub text_value: Option<String>,
    pub created_at: String,
}

impl IndicatorRecord {
    /// Display value: the number when present, otherwise the text. Pure.
    pub fn display(&self) -> String {
        match (self.value, &self.text_value) {
            (Some(value), _) => {
                let unit = self.unit.as_deref().unwrap_or("");
                if unit.is_empty() {
                    format!("{value:.4}")
                } else {
                    format!("{value:.4} {unit}")
                }
            }
            (None, Some(text)) => text.clone(),
            _ => String::new(),
        }
    }
}

/// Creates the indicator table when missing. Additive migration.
pub fn migrate_indicator_schema(conn: &Connection) -> Result<(), PlotError> {
    conn.execute(INDICATOR_TABLE_DDL, [])?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_plot_indicator_plot ON plot_indicator(plot_id)",
        [],
    )?;
    Ok(())
}

/// Flattens a plugin `metrics` object into indicator inputs, keeping numbers
/// and strings and dropping nulls/objects/arrays (they have no ranking value).
/// Unknown units stay `None`. Pure.
pub fn normalise_indicators(metrics: &Value) -> Vec<IndicatorInput> {
    let Some(map) = metrics.as_object() else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(key, value)| {
            let unit = default_unit(key).map(str::to_string);
            match value {
                Value::Number(number) => number.as_f64().map(|value| IndicatorInput {
                    key: key.clone(),
                    unit,
                    value: Value::from(value),
                }),
                Value::String(text) => Some(IndicatorInput {
                    key: key.clone(),
                    unit: None,
                    value: Value::from(text.clone()),
                }),
                Value::Bool(flag) => Some(IndicatorInput {
                    key: key.clone(),
                    unit: None,
                    value: Value::from(*flag),
                }),
                _ => None,
            }
        })
        .collect()
}

/// Known units for metrics the vegetation plugins emit. Pure.
pub fn default_unit(key: &str) -> Option<&'static str> {
    match key {
        "coverage_percentage"
        | "vigorous_pct"
        | "moderate_stress_pct"
        | "severe_stress_pct"
        | "yellowing_pct"
        | "browning_pct"
        | "vegetation_coverage_pct"
        | "canopy_pct"
        | "dominant_pct" => Some("%"),
        "vegetation_area_sqm" | "total_area_sqm" => Some("m²"),
        "area_m2" => Some("m²"),
        "perimeter_m" => Some("m"),
        "gsd_m" => Some("m/px"),
        "mean_hue_deg" => Some("°"),
        "total_pixels" | "vegetation_pixels" | "canopy_pixels" | "green_coverage" => Some("px"),
        "tree_count" | "grid_cells" | "plots_analyzed" => Some("items"),
        _ => None,
    }
}

fn upsert_indicator(
    conn: &Connection,
    plot_id: &str,
    plugin_id: &str,
    execution_id: Option<&str>,
    input: &IndicatorInput,
    created_at: &str,
) -> Result<(), PlotError> {
    let (value, text_value): (Option<f64>, Option<String>) = match &input.value {
        Value::Number(number) => (number.as_f64(), None),
        Value::String(text) => (None, Some(text.clone())),
        Value::Bool(flag) => (Some(f64::from(u8::from(*flag))), Some(flag.to_string())),
        _ => (None, None),
    };
    conn.execute(
        "INSERT INTO plot_indicator (id, plot_id, plugin_id, execution_id, indicator, unit, value, text_value, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(plot_id, plugin_id, indicator) DO UPDATE SET
            execution_id = excluded.execution_id,
            unit = excluded.unit,
            value = excluded.value,
            text_value = excluded.text_value,
            created_at = excluded.created_at",
        params![
            Uuid::new_v4().to_string(),
            plot_id,
            plugin_id,
            execution_id,
            input.key,
            input.unit,
            value,
            text_value,
            created_at,
        ],
    )?;
    Ok(())
}

/// Stores (or refreshes) the indicators of one plugin run for one plot. Impure.
pub fn save_indicators(
    conn: &Connection,
    plot_id: &str,
    plugin_id: &str,
    execution_id: Option<&str>,
    indicators: &[IndicatorInput],
) -> Result<usize, PlotError> {
    if indicators.is_empty() {
        return Ok(0);
    }
    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    for input in indicators {
        upsert_indicator(conn, plot_id, plugin_id, execution_id, input, &created_at)?;
    }
    Ok(indicators.len())
}

fn row_to_indicator(row: &Row) -> rusqlite::Result<IndicatorRecord> {
    Ok(IndicatorRecord {
        plot_id: row.get(0)?,
        plugin_id: row.get(1)?,
        execution_id: row.get(2)?,
        indicator: row.get(3)?,
        unit: row.get(4)?,
        value: row.get(5)?,
        text_value: row.get(6)?,
        created_at: row.get(7)?,
    })
}

/// Lists indicators, optionally narrowed to one plot or one project. Impure.
pub fn list_indicators(
    conn: &Connection,
    plot_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<Vec<IndicatorRecord>, PlotError> {
    let base = format!(
        "SELECT {} FROM plot_indicator i JOIN plot p ON p.id = i.plot_id",
        INDICATOR_COLUMNS
            .split(',')
            .map(|column| format!("i.{}", column.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let (sql, args): (String, Vec<String>) = match (plot_id, project_id) {
        (Some(id), _) => (
            format!("{base} WHERE i.plot_id = ?1 ORDER BY i.indicator"),
            vec![id.to_string()],
        ),
        (None, Some(id)) => (
            format!("{base} WHERE p.project_id = ?1 ORDER BY i.plot_id, i.indicator"),
            vec![id.to_string()],
        ),
        (None, None) => (
            format!("{base} ORDER BY i.plot_id, i.indicator"),
            Vec::new(),
        ),
    };
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(args.iter()), row_to_indicator)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Latest value of one indicator for one plot. Impure.
pub fn latest_indicator(
    conn: &Connection,
    plot_id: &str,
    indicator: &str,
) -> Result<Option<IndicatorRecord>, PlotError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {} FROM plot_indicator WHERE plot_id = ?1 AND indicator = ?2",
                INDICATOR_COLUMNS
            ),
            params![plot_id, indicator],
            row_to_indicator,
        )
        .optional()?)
}

/// Deletes every indicator of a plot (called when the plot is removed). Impure.
pub fn delete_plot_indicators(conn: &Connection, plot_id: &str) -> Result<usize, PlotError> {
    Ok(conn.execute(
        "DELETE FROM plot_indicator WHERE plot_id = ?1",
        params![plot_id],
    )?)
}

/// Empty-safe map helper used by tests.
#[cfg(test)]
fn metrics_from(pairs: &[(&str, Value)]) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_string(), value.clone());
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::modules::module_09::plots::migrate_plot_schema(&conn).unwrap();
        migrate_indicator_schema(&conn).unwrap();
        let polygon = crate::modules::module_09::plots::polygon_from_geometry(&json!({
            "type": "Polygon",
            "coordinates": [[[0.0, 0.0], [0.002, 0.0], [0.002, 0.002], [0.0, 0.002]]]
        }))
        .unwrap();
        crate::modules::module_09::plots::insert_plot(
            &conn,
            Some("project-1".into()),
            None,
            "Plot A",
            &polygon,
        )
        .unwrap();
        conn
    }

    #[test]
    fn normalisation_keeps_scalars_and_drops_nulls() {
        let metrics = metrics_from(&[
            ("coverage_percentage", json!(43.1)),
            ("classification", json!("medium")),
            ("dominant_condition", json!("vigorous")),
            ("nothing", json!(null)),
            ("nested", json!({"a": 1})),
            ("flags", json!([1, 2])),
        ]);
        let inputs = normalise_indicators(&metrics);
        let keys: Vec<&str> = inputs.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"coverage_percentage"));
        let coverage = inputs
            .iter()
            .find(|input| input.key == "coverage_percentage")
            .unwrap();
        assert_eq!(coverage.unit.as_deref(), Some("%"));
    }

    #[test]
    fn save_upserts_latest_value_per_key() {
        let conn = memory_db();
        let plot_id: String = conn
            .query_row("SELECT id FROM plot LIMIT 1", [], |row| row.get(0))
            .unwrap();

        save_indicators(
            &conn,
            &plot_id,
            "vegetation-coverage-analytics",
            Some("exec-1"),
            &[
                IndicatorInput {
                    key: "coverage_percentage".into(),
                    unit: Some("%".into()),
                    value: json!(40.0),
                },
                IndicatorInput {
                    key: "classification".into(),
                    unit: None,
                    value: json!("medium"),
                },
            ],
        )
        .unwrap();

        let first = latest_indicator(&conn, &plot_id, "coverage_percentage")
            .unwrap()
            .unwrap();
        assert_eq!(first.value, Some(40.0));
        assert_eq!(first.display(), "40.0000 %");

        // Re-running replaces instead of duplicating.
        save_indicators(
            &conn,
            &plot_id,
            "vegetation-coverage-analytics",
            Some("exec-2"),
            &[IndicatorInput {
                key: "coverage_percentage".into(),
                unit: Some("%".into()),
                value: json!(52.5),
            }],
        )
        .unwrap();
        let listed = list_indicators(&conn, Some(&plot_id), None).unwrap();
        assert_eq!(listed.len(), 2);
        let latest = latest_indicator(&conn, &plot_id, "coverage_percentage")
            .unwrap()
            .unwrap();
        assert_eq!(latest.value, Some(52.5));
        assert_eq!(latest.execution_id.as_deref(), Some("exec-2"));
    }

    #[test]
    fn project_filter_and_plot_deletion() {
        let conn = memory_db();
        let plot_id: String = conn
            .query_row("SELECT id FROM plot LIMIT 1", [], |row| row.get(0))
            .unwrap();
        save_indicators(
            &conn,
            &plot_id,
            "plugin-a",
            None,
            &[IndicatorInput {
                key: "vigorous_pct".into(),
                unit: Some("%".into()),
                value: json!(19.7),
            }],
        )
        .unwrap();

        assert_eq!(
            list_indicators(&conn, None, Some("project-1"))
                .unwrap()
                .len(),
            1
        );
        assert!(list_indicators(&conn, None, Some("other"))
            .unwrap()
            .is_empty());

        delete_plot_indicators(&conn, &plot_id).unwrap();
        assert!(list_indicators(&conn, Some(&plot_id), None)
            .unwrap()
            .is_empty());
    }
}
