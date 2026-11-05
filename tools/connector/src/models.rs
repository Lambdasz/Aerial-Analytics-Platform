//! Row mirrors of the app's SQLite schema (`modules/module_01/flight_session.rs`).
//! Columns not consumed by the connector are omitted; names match the DB exactly.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub image_count: i64,
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub image_count: i64,
    pub geotagged_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageRow {
    pub id: String,
    pub session_id: String,
    pub file_path: String,
    pub location_lat: Option<f64>,
    pub location_lon: Option<f64>,
    pub captured_at: Option<String>,
    pub gps_altitude_m: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub format: Option<String>,
    pub analysis_ready: Option<bool>,
    pub is_blurry: Option<bool>,
}

impl ImageRow {
    pub fn file_name(&self) -> &str {
        std::path::Path::new(&self.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.file_path)
    }

    pub fn has_gps(&self) -> bool {
        self.location_lat.is_some() && self.location_lon.is_some()
    }
}
