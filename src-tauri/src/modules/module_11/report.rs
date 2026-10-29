//! Module 11 — self-contained HTML report generation.
//!
//! Produces one printable HTML file per project: summary figures, indicator
//! aggregates with inline SVG bars, plot indicators, plugin usage, temporal
//! comparisons, and the plugin result explorer. No external assets or network
//! requests, so the file can be archived or printed to PDF by the user.
//!
//! # Purity
//!
//! `render_report_html` and the escaping/blocks helpers are pure;
//! `write_report` reads the database and writes one file.

use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde_json::json;

use super::dashboard::{build_project_dashboard, list_plugin_results, ProjectDashboard};
use crate::modules::module_10::comparisons::{list_comparisons, ComparisonRecord};

/// Escapes text for HTML text nodes and attributes. Pure.
pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// One plot indicator row used by the report table. Pure data holder.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotIndicatorRow {
    pub plot: String,
    pub plugin: String,
    pub indicator: String,
    pub unit: Option<String>,
    pub value: Option<f64>,
    pub text_value: Option<String>,
    pub created_at: String,
}

impl PlotIndicatorRow {
    fn display(&self) -> String {
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

/// Horizontal SVG bars for indicator aggregates. Pure, no JS, no external CSS.
pub fn render_bar_chart(items: &[(String, f64)]) -> String {
    if items.is_empty() {
        return "<p class=\"muted\">No indicator data.</p>".to_string();
    }
    let max = items
        .iter()
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);
    let rows: String = items
        .iter()
        .map(|(label, value)| {
            let width = ((value / max) * 240.0).round().clamp(1.0, 240.0);
            let height = 18.0;
            format!(
                "<div class=\"bar-row\"><span class=\"bar-label\">{}</span>\
                 <svg width=\"250\" height=\"{height}\" role=\"img\" aria-label=\"{}: {}\">\
                 <rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"#0066cc\" rx=\"3\"/>\
                 </svg><span class=\"bar-value\">{:.2}</span></div>",
                html_escape(label),
                html_escape(label),
                value,
                value
            )
        })
        .collect();
    format!("<div class=\"bar-chart\">{rows}</div>")
}

fn comparison_row(comparison: &ComparisonRecord) -> String {
    format!(
        "<tr><td>{} → {}</td><td>{} d</td><td>{:.2} → {:.2}</td>\
         <td>{}{:.2}</td><td>{}</td><td>{}</td></tr>",
        html_escape(&comparison.before_date),
        html_escape(&comparison.after_date),
        comparison.interval_days,
        comparison.before_value,
        comparison.after_value,
        if comparison.delta >= 0.0 { "+" } else { "" },
        comparison.delta,
        html_escape(&comparison.magnitude_level),
        html_escape(&comparison.metric),
    )
}

/// Renders the complete report document. Pure.
pub fn render_report_html(
    project_name: &str,
    dashboard: &ProjectDashboard,
    comparisons: &[ComparisonRecord],
    plot_indicators: &[PlotIndicatorRow],
    result_rows: &[(String, String, String, String)],
    generated_at: &str,
) -> String {
    let aggregates: Vec<(String, f64)> = dashboard
        .indicator_aggregates
        .iter()
        .map(|aggregate| (aggregate.indicator.clone(), aggregate.mean))
        .collect();
    let chart = render_bar_chart(&aggregates);

    let aggregate_rows: String = dashboard
        .indicator_aggregates
        .iter()
        .map(|aggregate| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                html_escape(&aggregate.indicator),
                html_escape(aggregate.unit.as_deref().unwrap_or("")),
                aggregate.plots,
                aggregate.min,
                aggregate.mean,
                aggregate.max
            )
        })
        .collect();

    let plugin_rows: String = dashboard
        .plugins
        .iter()
        .map(|plugin| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(&plugin.plugin_id),
                plugin.images,
                html_escape(plugin.last_run_at.as_deref().unwrap_or("—"))
            )
        })
        .collect();

    let indicator_rows: String = plot_indicators
        .iter()
        .map(|row| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(&row.plot),
                html_escape(&row.plugin),
                html_escape(&row.indicator),
                html_escape(&row.display()),
                html_escape(&row.created_at)
            )
        })
        .collect();

    let comparison_rows: String = comparisons.iter().map(comparison_row).collect::<String>();

    let explorer_rows: String = result_rows
        .iter()
        .map(|(image, plugin, metric, value)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(image),
                html_escape(plugin),
                html_escape(metric),
                html_escape(value)
            )
        })
        .collect();

    let latest = dashboard
        .latest_comparison
        .as_ref()
        .map(|comparison| {
            format!(
                "{} → {} ({}{:.2} pp, {})",
                html_escape(&comparison.before_date),
                html_escape(&comparison.after_date),
                if comparison.delta >= 0.0 { "+" } else { "" },
                comparison.delta,
                html_escape(&comparison.magnitude_level)
            )
        })
        .unwrap_or_else(|| "no comparison yet".to_string());

    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>Aerial Analytics report — {title}</title>\
         <style>\
         body{{font-family:system-ui,-apple-system,'Segoe UI',sans-serif;color:#1d1d1f;margin:32px;}}\
         h1{{font-size:26px;margin:0 0 4px;}} h2{{font-size:17px;margin:28px 0 8px;}}\
         .muted{{color:#707070;font-size:12px;}} table{{border-collapse:collapse;width:100%;font-size:12px;}}\
         th,td{{border-bottom:1px solid #e6e6e8;padding:6px 8px;text-align:left;}}\
         th{{color:#707070;font-weight:600;text-transform:uppercase;letter-spacing:.04em;}}\
         .cards{{display:flex;gap:12px;flex-wrap:wrap;margin:16px 0;}}\
         .card{{border:1px solid #d6d6d6;border-radius:12px;padding:12px 16px;min-width:140px;}}\
         .card b{{display:block;font-size:22px;margin-top:4px;}}\
         .bar-row{{display:flex;align-items:center;gap:8px;margin:4px 0;font-size:12px;}}\
         .bar-label{{width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}}\
         .bar-value{{color:#707070;}}\
         footer{{margin-top:32px;border-top:1px solid #e6e6e8;padding-top:8px;}}\
         @media print{{body{{margin:12mm;}}}}\
         </style></head><body>\
         <h1>Aerial Analytics report — {title}</h1>\
         <p class=\"muted\">Project {project_id} · generated {generated_at} · read-only summary of stored results</p>\
         <div class=\"cards\">\
         <div class=\"card\">Sessions<b>{sessions}</b></div>\
         <div class=\"card\">Images<b>{images}</b><span class=\"muted\">{geotagged} geotagged</span></div>\
         <div class=\"card\">Plots<b>{plots}</b><span class=\"muted\">{area:.2} ha</span></div>\
         <div class=\"card\">Plugin results<b>{plugin_results}</b></div>\
         <div class=\"card\">Latest comparison<b>{latest}</b></div>\
         </div>\
         <h2>Indicator aggregates</h2>{chart}\
         <table><thead><tr><th>Indicator</th><th>Unit</th><th>Plots</th><th>Min</th><th>Mean</th><th>Max</th></tr></thead>\
         <tbody>{aggregate_rows}</tbody></table>\
         <h2>Plots and their indicators</h2>\
         <table><thead><tr><th>Plot</th><th>Plugin</th><th>Indicator</th><th>Value</th><th>Recorded</th></tr></thead>\
         <tbody>{indicator_rows}</tbody></table>\
         <h2>Plugin usage</h2>\
         <table><thead><tr><th>Plugin</th><th>Images</th><th>Last run</th></tr></thead>\
         <tbody>{plugin_rows}</tbody></table>\
         <h2>Temporal comparisons</h2>\
         <table><thead><tr><th>Period</th><th>Interval</th><th>Coverage</th><th>Delta</th><th>Level</th><th>Metric</th></tr></thead>\
         <tbody>{comparison_rows}</tbody></table>\
         <h2>Plugin results</h2>\
         <table><thead><tr><th>Image</th><th>Plugin</th><th>Metric</th><th>Value</th></tr></thead>\
         <tbody>{explorer_rows}</tbody></table>\
         <footer><p class=\"muted\">Generated by Aerial Analytics Platform (Module 11). \
         Figures come from Module 1 sessions, Module 4-6 plugin results, Module 9 plots and Module 10 comparisons.</p></footer>\
         </body></html>\n",
        title = html_escape(project_name),
        project_id = html_escape(&dashboard.project_id),
        generated_at = html_escape(generated_at),
        sessions = dashboard.sessions,
        images = dashboard.images,
        geotagged = dashboard.geotagged_images,
        plots = dashboard.plots,
        area = dashboard.plot_area_ha,
        plugin_results = dashboard.plugin_results,
        latest = latest,
        chart = chart,
        aggregate_rows = aggregate_rows,
        indicator_rows = indicator_rows,
        plugin_rows = plugin_rows,
        comparison_rows = comparison_rows,
        explorer_rows = explorer_rows,
    )
}

fn project_name(conn: &Connection, project_id: &str) -> String {
    conn.query_row(
        "SELECT name FROM project WHERE id = ?1",
        params![project_id],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| project_id.to_string())
}

fn plot_indicator_rows(
    conn: &Connection,
    project_id: &str,
) -> rusqlite::Result<Vec<PlotIndicatorRow>> {
    let mut statement = conn.prepare(
        "SELECT p.name, i.plugin_id, i.indicator, i.unit, i.value, i.text_value, i.created_at \
         FROM plot_indicator i JOIN plot p ON p.id = i.plot_id \
         WHERE p.project_id = ?1 OR p.project_id IS NULL \
         ORDER BY p.name, i.plugin_id, i.indicator",
    )?;
    let rows = statement
        .query_map(params![project_id], |row| {
            Ok(PlotIndicatorRow {
                plot: row.get(0)?,
                plugin: row.get(1)?,
                indicator: row.get(2)?,
                unit: row.get(3)?,
                value: row.get(4)?,
                text_value: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Writes the report for one project and returns its path.
pub fn write_report(conn: &Connection, project_id: &str, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("cannot create export dir: {error}"))?;
    let dashboard = build_project_dashboard(conn, project_id).map_err(|error| error.to_string())?;
    let comparisons =
        list_comparisons(conn, Some(project_id), 10).map_err(|error| error.to_string())?;
    let indicators = plot_indicator_rows(conn, project_id).map_err(|error| error.to_string())?;
    let results = list_plugin_results(conn, project_id, 200).map_err(|error| error.to_string())?;

    let result_rows: Vec<(String, String, String, String)> = results
        .iter()
        .map(|row| {
            let value = match (row.value, &row.text_value) {
                (Some(value), _) => {
                    let unit = row.unit.as_deref().unwrap_or("");
                    if unit.is_empty() {
                        format!("{value:.4}")
                    } else {
                        format!("{value:.4} {unit}")
                    }
                }
                (None, Some(text)) => text.clone(),
                _ => String::new(),
            };
            let image = row
                .file_path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&row.file_path)
                .to_string();
            (image, row.plugin_id.clone(), row.metric.clone(), value)
        })
        .collect();

    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let html = render_report_html(
        &project_name(conn, project_id),
        &dashboard,
        &comparisons,
        &indicators,
        &result_rows,
        &generated_at,
    );

    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("dashboard_report_{stamp}.html"));
    std::fs::write(&path, html).map_err(|error| format!("cannot write report: {error}"))?;
    Ok(path)
}

/// JSON mirror of the report data, for automated pipelines. Pure formatting of
/// already-aggregated values.
pub fn report_payload(
    project_name: &str,
    dashboard: &ProjectDashboard,
    comparisons: &[ComparisonRecord],
    plot_indicators: &[PlotIndicatorRow],
) -> serde_json::Value {
    json!({
        "project": project_name,
        "dashboard": dashboard,
        "comparisons": comparisons,
        "plot_indicators": plot_indicators
            .iter()
            .map(|row| json!({
                "plot": row.plot,
                "plugin": row.plugin,
                "indicator": row.indicator,
                "unit": row.unit,
                "value": row.value,
                "text_value": row.text_value,
                "display": row.display(),
                "created_at": row.created_at,
            }))
            .collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_09::indicators::{save_indicators, IndicatorInput};
    use crate::modules::module_09::plots::{
        insert_plot, migrate_plot_schema, polygon_from_geometry,
    };

    #[test]
    fn escapes_html_metacharacters() {
        assert_eq!(
            html_escape("<b> & \"x\"</b>"),
            "&lt;b&gt; &amp; &quot;x&quot;&lt;/b&gt;"
        );
    }

    #[test]
    fn chart_renders_labels_and_values() {
        let svg = render_bar_chart(&[
            ("coverage_percentage".to_string(), 40.0),
            ("mean_vari".to_string(), 0.2),
        ]);
        assert!(svg.contains("coverage_percentage"));
        assert!(svg.contains("40.00"));
        assert!(svg.contains("<svg"));
        assert!(render_bar_chart(&[]).contains("No indicator data"));
    }

    fn seeded_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::modules::module_01::flight_session::migrate_image_schema(&conn).unwrap();
        migrate_plot_schema(&conn).unwrap();
        crate::modules::module_09::indicators::migrate_indicator_schema(&conn).unwrap();
        crate::modules::module_10::comparisons::migrate_comparison_schema(&conn).unwrap();
        crate::plugin_manager::result_store::migrate_result_schema(&conn).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS project (id TEXT PRIMARY KEY, name TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project (id, name) VALUES ('p1', 'Demo & <Test>')",
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
            "INSERT INTO session VALUES ('s1','p1','S','2026-01-01T00:00:00','2026-01-01T00:00:00','active','2026-01-01T00:00:00','2026-01-01T00:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at, heading_deg)
             VALUES ('img1','s1','/tmp/a.jpg', 1.0, 2.0, '2026-01-01T00:00:00', 0.0)",
            [],
        )
        .unwrap();
        let polygon = polygon_from_geometry(&json!({
            "type": "Polygon",
            "coordinates": [[[0.0, 0.0], [0.002, 0.0], [0.002, 0.002], [0.0, 0.002]]]
        }))
        .unwrap();
        let plot = insert_plot(&conn, Some("p1".into()), None, "Plot A", &polygon, None).unwrap();
        save_indicators(
            &conn,
            &plot.id,
            "vegetation-coverage-analytics",
            None,
            &[IndicatorInput {
                key: "coverage_percentage".into(),
                unit: Some("%".into()),
                value: json!(40.0),
            }],
        )
        .unwrap();
        conn
    }

    #[test]
    fn report_contains_sections_and_escaped_title() {
        let conn = seeded_db();
        let dir = tempfile::tempdir().unwrap();
        let path = write_report(&conn, "p1", dir.path()).unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(html.contains("Aerial Analytics report"));
        assert!(
            html.contains("Demo &amp; &lt;Test&gt;"),
            "title must be escaped"
        );
        assert!(html.contains("Indicator aggregates"));
        assert!(html.contains("Temporal comparisons"));
        assert!(html.contains("Plugin results"));
        assert!(!html.contains("<script"), "report must stay asset-free");
    }

    #[test]
    fn payload_carries_indicators() {
        let conn = seeded_db();
        let dashboard = build_project_dashboard(&conn, "p1").unwrap();
        let report = report_payload("Demo", &dashboard, &[], &[]);
        assert_eq!(report["dashboard"]["plots"], 1);
        assert_eq!(report["project"], "Demo");
    }
}
