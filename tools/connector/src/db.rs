//! Read-only access to the app's SQLite database.
//!
//! Path resolution mirrors `plan_db_location` in `src-tauri/src/lib.rs`:
//! `AAP_DB_PATH` wins, otherwise the Linux app-data location
//! (`~/.local/share/<identifier>/aerial.db`). Read-only open so the
//! running app keeps its writer lock.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};

use crate::models::{ImageRow, ProjectRow, SessionRow};

const APP_IDENTIFIER: &str = "com.lambdasz.aerial-analytics-platform";

pub fn resolve_db_path(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("AAP_DB_PATH") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    let db = PathBuf::from(home)
        .join(".local")
        .join("share")
        .join(APP_IDENTIFIER)
        .join("aerial.db");
    if !db.exists() {
        bail!(
            "database not found at {} (set AAP_DB_PATH to override)",
            db.display()
        );
    }
    Ok(db)
}

pub fn open_readonly(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open database {}", path.display()))
}

pub fn list_projects(conn: &Connection) -> Result<Vec<ProjectRow>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.description,
                (SELECT COUNT(*) FROM session s WHERE s.project_id = p.id),
                (SELECT COUNT(*) FROM image i
                   JOIN session s ON i.session_id = s.id
                  WHERE s.project_id = p.id)
           FROM project p
          ORDER BY p.created_at",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ProjectRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                session_count: row.get(3)?,
                image_count: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_sessions(conn: &Connection, project_id: &str) -> Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.project_id, s.name, s.status, s.date_start, s.date_end,
                (SELECT COUNT(*) FROM image i WHERE i.session_id = s.id),
                (SELECT COUNT(*) FROM image i
                  WHERE i.session_id = s.id
                    AND i.location_lat IS NOT NULL
                    AND i.location_lon IS NOT NULL)
           FROM session s
          WHERE s.project_id = ?1
          ORDER BY s.created_at",
    )?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
                status: row.get(3)?,
                date_start: row.get(4)?,
                date_end: row.get(5)?,
                image_count: row.get(6)?,
                geotagged_count: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_images(conn: &Connection, session_id: &str) -> Result<Vec<ImageRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, file_path, location_lat, location_lon, captured_at,
                gps_altitude_m, width, height, format, analysis_ready, is_blurry
           FROM image
          WHERE session_id = ?1
          ORDER BY file_path",
    )?;
    let rows = stmt
        .query_map([session_id], |row| {
            let analysis_ready: Option<i64> = row.get(10)?;
            let is_blurry: Option<i64> = row.get(11)?;
            Ok(ImageRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                file_path: row.get(2)?,
                location_lat: row.get(3)?,
                location_lon: row.get(4)?,
                captured_at: row.get(5)?,
                gps_altitude_m: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                format: row.get(9)?,
                analysis_ready: analysis_ready.map(|v| v != 0),
                is_blurry: is_blurry.map(|v| v != 0),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
