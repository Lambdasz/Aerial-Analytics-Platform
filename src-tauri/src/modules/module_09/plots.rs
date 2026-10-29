//! Plot persistence and geodesic metrics (Module 9).
//!
//! A plot is a named polygon boundary inside a project (optionally tied to a
//! flight session) that analysis results are aggregated into. Area and
//! perimeter are computed once on write with the Module 3 geodesic helpers, so
//! ranking and comparison never recompute geometry.
//!
//! # Purity
//!
//! Validation and metric helpers are pure; the `load`/`insert`/`update`/
//! `delete` functions are impure and take an open SQLite connection.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::modules::module_03::geometry::spherical_excess::{calculate_area, calculate_perimeter};
use crate::modules::module_03::types::aoi_type::Polygon;

/// DDL for the `plot` table. Additive: existing databases gain it on startup.
const PLOT_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS plot (
    id            TEXT PRIMARY KEY,
    project_id    TEXT,
    session_id    TEXT,
    name          TEXT NOT NULL,
    geometry      TEXT NOT NULL,
    area_m2       REAL NOT NULL,
    perimeter_m   REAL NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
)";

const PLOT_COLUMNS: &str =
    "id, project_id, session_id, name, geometry, area_m2, perimeter_m, created_at, updated_at";

/// A stored plot boundary with its geodesic metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlotRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub name: String,
    /// GeoJSON `geometry` object of type `Polygon` (WGS 84, lon/lat order).
    pub geometry: Value,
    pub area_m2: f64,
    pub area_ha: f64,
    pub perimeter_m: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// Why a plot could not be stored.
#[derive(Debug)]
pub struct PlotError(pub String);

impl std::fmt::Display for PlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlotError {}

impl From<rusqlite::Error> for PlotError {
    fn from(error: rusqlite::Error) -> Self {
        PlotError(error.to_string())
    }
}

impl From<crate::plugin_manager::error::CommandError> for PlotError {
    fn from(error: crate::plugin_manager::error::CommandError) -> Self {
        PlotError(error.message)
    }
}

/// Creates the `plot` table when it is missing. Additive migration: never
/// rewrites existing tables.
pub fn migrate_plot_schema(conn: &Connection) -> Result<(), PlotError> {
    conn.execute(PLOT_TABLE_DDL, [])?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_plot_project ON plot(project_id)",
        [],
    )?;
    Ok(())
}

/// Extracts a WGS 84 polygon from a GeoJSON `geometry` value and validates it.
///
/// Accepts `{"type":"Polygon","coordinates":[[[lon,lat],…]]}` and (for
/// convenience) a bare coordinate array. Rings are closed automatically. Pure.
pub fn polygon_from_geometry(geometry: &Value) -> Result<Polygon, PlotError> {
    let coordinates = if geometry.get("type").and_then(Value::as_str) == Some("Polygon") {
        geometry
            .get("coordinates")
            .ok_or_else(|| PlotError("Polygon geometry has no coordinates".into()))?
    } else if geometry.is_array() {
        geometry
    } else {
        let kind = geometry
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(PlotError(format!(
            "plot geometry must be a Polygon, got {kind}"
        )));
    };

    let rings = coordinates
        .as_array()
        .ok_or_else(|| PlotError("polygon coordinates must be an array of rings".into()))?;
    if rings.is_empty() {
        return Err(PlotError("polygon has no rings".into()));
    }

    let mut polygon: Polygon = Vec::with_capacity(rings.len());
    for (index, ring_value) in rings.iter().enumerate() {
        let ring = ring_value
            .as_array()
            .ok_or_else(|| PlotError("polygon ring must be an array of positions".into()))?;
        let mut positions: Vec<[f64; 2]> = Vec::with_capacity(ring.len() + 1);
        for position in ring {
            let pair = position
                .as_array()
                .ok_or_else(|| PlotError("polygon position must be [lon, lat]".into()))?;
            let lon = pair.first().and_then(Value::as_f64);
            let lat = pair.get(1).and_then(Value::as_f64);
            let (Some(lon), Some(lat)) = (lon, lat) else {
                return Err(PlotError("polygon position must be two numbers".into()));
            };
            if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
                return Err(PlotError(format!(
                    "position [{lon}, {lat}] is outside WGS 84 bounds"
                )));
            }
            positions.push([lon, lat]);
        }
        if positions.len() < 3 {
            return Err(PlotError(format!(
                "ring {index} needs at least 3 positions"
            )));
        }
        let first = positions[0];
        let last = positions[positions.len() - 1];
        if first != last {
            positions.push(first);
        }
        polygon.push(positions);
    }

    let area = calculate_area(&polygon);
    if !area.is_finite() || area <= 0.0 {
        return Err(PlotError(
            "polygon encloses no area (check ring winding and vertex order)".into(),
        ));
    }
    Ok(polygon)
}

/// Area in m² and perimeter in m for a validated polygon. Pure.
pub fn metrics(polygon: &Polygon) -> (f64, f64) {
    (calculate_area(polygon), calculate_perimeter(polygon))
}

/// Serialises a polygon back into a GeoJSON `geometry` value. Pure.
pub fn geometry_value(polygon: &Polygon) -> Value {
    json!({ "type": "Polygon", "coordinates": polygon })
}

fn row_to_plot(row: &Row) -> rusqlite::Result<PlotRecord> {
    let geometry_text: String = row.get(4)?;
    let geometry: Value = serde_json::from_str(&geometry_text)
        .unwrap_or_else(|_| json!({"type": "Polygon", "coordinates": []}));
    let area_m2: f64 = row.get(5)?;
    Ok(PlotRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        name: row.get(3)?,
        geometry,
        area_m2,
        area_ha: area_m2 / 10_000.0,
        perimeter_m: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn validate_name(name: &str) -> Result<String, PlotError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(PlotError("plot name must not be empty".into()));
    }
    if trimmed.chars().count() > 100 {
        return Err(PlotError("plot name must be at most 100 characters".into()));
    }
    Ok(trimmed.to_string())
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Inserts a new plot. Impure — writes a row.
pub fn insert_plot(
    conn: &Connection,
    project_id: Option<String>,
    session_id: Option<String>,
    name: &str,
    polygon: &Polygon,
) -> Result<PlotRecord, PlotError> {
    let name = validate_name(name)?;
    let (area_m2, perimeter_m) = metrics(polygon);
    let timestamp = now();
    let record = PlotRecord {
        id: Uuid::new_v4().to_string(),
        project_id,
        session_id,
        name,
        geometry: geometry_value(polygon),
        area_m2,
        area_ha: area_m2 / 10_000.0,
        perimeter_m,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    conn.execute(
        &format!("INSERT INTO plot ({PLOT_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
        params![
            record.id,
            record.project_id,
            record.session_id,
            record.name,
            serde_json::to_string(&record.geometry).map_err(|e| PlotError(e.to_string()))?,
            record.area_m2,
            record.perimeter_m,
            record.created_at,
            record.updated_at,
        ],
    )?;
    Ok(record)
}

/// Lists plots, newest first, optionally filtered to one project. Impure.
pub fn list_plots(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<PlotRecord>, PlotError> {
    let sql = match project_id {
        Some(_) => format!(
            "SELECT {PLOT_COLUMNS} FROM plot WHERE project_id = ?1 ORDER BY created_at DESC"
        ),
        None => format!("SELECT {PLOT_COLUMNS} FROM plot ORDER BY created_at DESC"),
    };
    let mut statement = conn.prepare(&sql)?;
    let rows = match project_id {
        Some(id) => statement
            .query_map(params![id], row_to_plot)?
            .collect::<Result<Vec<_>, _>>()?,
        None => statement
            .query_map([], row_to_plot)?
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(rows)
}

/// Fetches one plot by id. Impure.
pub fn get_plot(conn: &Connection, plot_id: &str) -> Result<Option<PlotRecord>, PlotError> {
    Ok(conn
        .query_row(
            &format!("SELECT {PLOT_COLUMNS} FROM plot WHERE id = ?1"),
            params![plot_id],
            row_to_plot,
        )
        .optional()?)
}

/// Renames a plot. Impure.
pub fn rename_plot(conn: &Connection, plot_id: &str, name: &str) -> Result<PlotRecord, PlotError> {
    let name = validate_name(name)?;
    let updated = now();
    let changed = conn.execute(
        "UPDATE plot SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, updated, plot_id],
    )?;
    if changed == 0 {
        return Err(PlotError(format!("plot '{plot_id}' not found")));
    }
    get_plot(conn, plot_id)?.ok_or_else(|| PlotError(format!("plot '{plot_id}' not found")))
}

/// Deletes a plot. Impure.
pub fn delete_plot(conn: &Connection, plot_id: &str) -> Result<(), PlotError> {
    let changed = conn.execute("DELETE FROM plot WHERE id = ?1", params![plot_id])?;
    if changed == 0 {
        return Err(PlotError(format!("plot '{plot_id}' not found")));
    }
    Ok(())
}

/// Formats a stored timestamp for display, falling back to the raw value. Pure.
pub fn display_timestamp(raw: &str) -> String {
    raw.parse::<DateTime<Utc>>()
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_else(|_| raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(lon: f64, lat: f64, side: f64) -> Value {
        json!({
            "type": "Polygon",
            "coordinates": [[
                [lon, lat],
                [lon + side, lat],
                [lon + side, lat + side],
                [lon, lat + side]
            ]]
        })
    }

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate_plot_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn polygon_is_closed_and_measured() {
        let polygon = polygon_from_geometry(&square(116.833, -1.270, 0.001)).unwrap();
        let ring = &polygon[0];
        assert_eq!(ring.first(), ring.last());
        let (area, perimeter) = metrics(&polygon);
        // ~111 m sides: 0.001 deg lon at 1.27 S is ~111 m.
        assert!((area - 12_300.0).abs() < 500.0, "area {area}");
        assert!((perimeter - 444.0).abs() < 10.0, "perimeter {perimeter}");
    }

    #[test]
    fn invalid_geometry_is_rejected() {
        assert!(polygon_from_geometry(&json!({ "type": "Point", "coordinates": [0, 0] })).is_err());
        assert!(polygon_from_geometry(&json!({ "type": "Polygon", "coordinates": [] })).is_err());
        // Two positions cannot enclose area.
        assert!(polygon_from_geometry(&json!({
            "type": "Polygon",
            "coordinates": [[[0, 0], [1, 1]]]
        }))
        .is_err());
        // Out of WGS 84 bounds.
        assert!(polygon_from_geometry(&json!({
            "type": "Polygon",
            "coordinates": [[[0, 0], [500, 0], [500, 1]]]
        }))
        .is_err());
    }

    #[test]
    fn crud_roundtrip() {
        let conn = memory_db();
        let polygon = polygon_from_geometry(&square(116.833, -1.270, 0.002)).unwrap();
        let created =
            insert_plot(&conn, Some("project-1".into()), None, " Plot A ", &polygon).unwrap();
        assert_eq!(created.name, "Plot A");
        assert!(created.area_ha > 0.0);

        let listed = list_plots(&conn, Some("project-1")).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(list_plots(&conn, Some("other")).unwrap().is_empty());

        let renamed = rename_plot(&conn, &created.id, "North field").unwrap();
        assert_eq!(renamed.name, "North field");

        delete_plot(&conn, &created.id).unwrap();
        assert!(get_plot(&conn, &created.id).unwrap().is_none());
        assert!(delete_plot(&conn, &created.id).is_err());
    }

    #[test]
    fn empty_names_are_rejected() {
        let conn = memory_db();
        let polygon = polygon_from_geometry(&square(0.0, 0.0, 0.001)).unwrap();
        assert!(insert_plot(&conn, None, None, "   ", &polygon).is_err());
        assert!(insert_plot(&conn, None, None, &"x".repeat(101), &polygon).is_err());
    }
}
