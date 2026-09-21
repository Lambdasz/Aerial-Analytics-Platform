#![allow(dead_code)]
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub date_start: NaiveDateTime,
    pub date_end: NaiveDateTime,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub id: Uuid,
    pub session_id: Uuid,
    pub file_path: String,
    pub location_lat: f64,
    pub location_lon: f64,
    pub captured_at: NaiveDateTime,
}
