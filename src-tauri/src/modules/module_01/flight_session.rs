#![allow(dead_code)]

//! Flight Session Management (Module 1.4).
//!
//! Backs the `commands::session` Tauri command stubs with real logic:
//! persisting sessions to SQLite, and deriving a session's date range from
//! the EXIF `captured_at` of the images assigned to it.
//!
//! ## Purity Boundary
//!
//! Every function that touches the database (i.e. takes a `&Connection`) is
//! impure. [`compute_date_range`] and [`default_session_name`] are the only
//! pure functions in this module — they can be unit-tested with in-memory
//! fixtures, no database required.
//!
//! ## Open integration question
//!
//! [`assign_image_to_session`] updates the `session_id` column on the
//! `image` table. This module assumes that table already exists (created
//! either here or by Module 1's Image Import work) with at minimum an
//! `id TEXT PRIMARY KEY` and `session_id TEXT` column. Needs confirming
//! with whoever owns Image persistence.

use chrono::{NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use super::error::SessionError;
use crate::models::{Image, Session, SessionStatus};

/// Creates the `session` and `image` tables if they do not already exist.
///
/// The `image` table is defined here (rather than by Module 1's Image
/// Import work) per team decision on 2026-09-26: Adit is building the
/// import function against this schema as the source of truth.
///
/// # Purity
///
/// Impure — DDL against the database.
pub fn init_schema(conn: &Connection) -> Result<(), SessionError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS session (
            id          TEXT PRIMARY KEY,
            project_id  TEXT NOT NULL,
            name        TEXT NOT NULL,
            date_start  TEXT,
            date_end    TEXT,
            status      TEXT NOT NULL CHECK (status IN ('active', 'archived')) DEFAULT 'active',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS image (
            id            TEXT PRIMARY KEY,
            session_id    TEXT NOT NULL REFERENCES session(id),
            file_path     TEXT NOT NULL UNIQUE,
            location_lat  REAL NOT NULL,
            location_lon  REAL NOT NULL,
            captured_at   TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

/// Creates a new, empty session (no images assigned yet).
///
/// # Purity
///
/// Impure — writes a new row.
///
/// # Arguments
///
/// * `conn` — open SQLite connection.
/// * `project_id` — the parent project this session belongs to.
/// * `name` — explicit session name; when `None`, a default name is
///   generated from the current time via [`default_session_name`].
///
/// # Errors
///
/// [`SessionError::Db`] if the insert fails.
pub fn create_session(
    conn: &Connection,
    project_id: Uuid,
    name: Option<String>,
) -> Result<Session, SessionError> {
    let now = Utc::now();
    let session = Session {
        id: Uuid::new_v4(),
        project_id,
        name: name.unwrap_or_else(|| default_session_name(now.naive_utc())),
        date_start: now.naive_utc(),
        date_end: now.naive_utc(),
        status: SessionStatus::Active,
        created_at: now,
        updated_at: now,
    };

    conn.execute(
        "INSERT INTO session (id, project_id, name, date_start, date_end, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            session.id.to_string(),
            session.project_id.to_string(),
            session.name,
            naive_to_iso(&session.date_start),
            naive_to_iso(&session.date_end),
            status_to_str(session.status),
            session.created_at.to_rfc3339(),
            session.updated_at.to_rfc3339(),
        ],
    )?;

    Ok(session)
}

/// Fetches a single session by id.
///
/// # Purity
///
/// Impure — reads from the database.
///
/// # Errors
///
/// [`SessionError::NotFound`] if no session with that id exists.
/// [`SessionError::Db`] on any other database error.
pub fn get_session(conn: &Connection, session_id: Uuid) -> Result<Session, SessionError> {
    conn.query_row(
        "SELECT id, project_id, name, date_start, date_end, status, created_at, updated_at
         FROM session WHERE id = ?1",
        params![session_id.to_string()],
        row_to_session,
    )
    .optional()?
    .ok_or(SessionError::NotFound { session_id })
}

/// Lists every session belonging to a project.
///
/// # Purity
///
/// Impure.
pub fn get_sessions_by_project(
    conn: &Connection,
    project_id: Uuid,
) -> Result<Vec<Session>, SessionError> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, date_start, date_end, status, created_at, updated_at
         FROM session WHERE project_id = ?1 ORDER BY date_start",
    )?;
    let sessions = stmt
        .query_map(params![project_id.to_string()], row_to_session)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

/// Overwrites a session's auto-generated name with a user-provided one.
///
/// # Purity
///
/// Impure.
pub fn update_session_name(
    conn: &Connection,
    session_id: Uuid,
    new_name: String,
) -> Result<Session, SessionError> {
    let now = Utc::now();
    let rows = conn.execute(
        "UPDATE session SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_name, now.to_rfc3339(), session_id.to_string()],
    )?;
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    get_session(conn, session_id)
}

/// Transitions a session between `active` and `archived`.
///
/// # Purity
///
/// Impure.
pub fn update_session_status(
    conn: &Connection,
    session_id: Uuid,
    status: SessionStatus,
) -> Result<Session, SessionError> {
    let now = Utc::now();
    let rows = conn.execute(
        "UPDATE session SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![
            status_to_str(status),
            now.to_rfc3339(),
            session_id.to_string()
        ],
    )?;
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    get_session(conn, session_id)
}

/// Associates an already-imported image with a session (manual assignment
/// at upload time — see project decision log: images are never dangling).
///
/// # Purity
///
/// Impure. Also triggers [`recalculate_session_date_range`] for the target
/// session, since adding an image can shift `date_start`/`date_end`.
///
/// # Open question
///
/// Assumes an `image` table with a `session_id` column already exists.
/// See module-level docs.
pub fn assign_image_to_session(
    conn: &Connection,
    session_id: Uuid,
    image_id: Uuid,
) -> Result<(), SessionError> {
    let previous_session_id: Option<String> = conn
        .query_row(
            "SELECT session_id FROM image WHERE id = ?1",
            params![image_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(previous_session_id) = previous_session_id else {
        return Err(SessionError::ImageNotFound { image_id });
    };

    conn.execute(
        "UPDATE image SET session_id = ?1 WHERE id = ?2",
        params![session_id.to_string(), image_id.to_string()],
    )?;

    recalculate_session_date_range(conn, session_id)?;

    if previous_session_id != session_id.to_string() {
        if let Ok(previous_session_id) = Uuid::parse_str(&previous_session_id) {
            recalculate_session_date_range(conn, previous_session_id)?;
        }
    }
    Ok(())
}

/// Recomputes `date_start`/`date_end` for a session from the `captured_at`
/// of every image currently assigned to it.
///
/// # Purity
///
/// Impure (reads images, writes the session) — but delegates the actual
/// min/max computation to the pure [`compute_date_range`].
///
/// # Errors
///
/// [`SessionError::NotFound`] if the session doesn't exist. A session with
/// zero images assigned is left with its previous date range unchanged
/// (not an error — see project decision log: `image_ids` may be empty on a
/// freshly created session).
pub fn recalculate_session_date_range(
    conn: &Connection,
    session_id: Uuid,
) -> Result<(), SessionError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM session WHERE id = ?1",
            params![session_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(SessionError::NotFound { session_id });
    }

    let mut stmt = conn.prepare(
        "SELECT id, session_id, file_path, location_lat, location_lon, captured_at
         FROM image WHERE session_id = ?1",
    )?;
    let images = stmt
        .query_map(params![session_id.to_string()], row_to_image)?
        .collect::<Result<Vec<_>, _>>()?;

    let Some((start, end)) = compute_date_range(&images) else {
        return Ok(());
    };

    let rows = conn.execute(
        "UPDATE session SET date_start = ?1, date_end = ?2, updated_at = ?3 WHERE id = ?4",
        params![
            naive_to_iso(&start),
            naive_to_iso(&end),
            Utc::now().to_rfc3339(),
            session_id.to_string(),
        ],
    )?;
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    Ok(())
}

/// Deletes a session.
///
/// # Status
///
/// **Open item** — behaviour when the session still has images assigned to
/// it has not yet been decided by the team (block deletion vs. cascade
/// vs. unassign). Currently deletes unconditionally; revisit once decided.
///
/// # Purity
///
/// Impure.
pub fn delete_session(conn: &Connection, session_id: Uuid) -> Result<(), SessionError> {
    let rows = conn.execute(
        "DELETE FROM session WHERE id = ?1",
        params![session_id.to_string()],
    )?;
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    Ok(())
}

/// Derives a session's `(date_start, date_end)` as the min/max `captured_at`
/// across a set of images.
///
/// # Purity
///
/// Pure, total (never fails).
///
/// # Returns
///
/// `None` when `images` is empty (nothing to derive a range from).
fn compute_date_range(images: &[Image]) -> Option<(NaiveDateTime, NaiveDateTime)> {
    let mut iter = images.iter().map(|img| img.captured_at);
    let first = iter.next()?;
    let (min, max) = iter.fold((first, first), |(min, max), t| (min.min(t), max.max(t)));
    Some((min, max))
}

/// Generates a default session name from a timestamp, e.g. `"Session -
/// 2026-09-18 07:12"`. User-overwritable per project decision log.
///
/// # Purity
///
/// Pure, total (never fails).
fn default_session_name(at: NaiveDateTime) -> String {
    format!("Session - {}", at.format("%Y-%m-%d %H:%M"))
}

fn status_to_str(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::Archived => "archived",
    }
}

/// Formats a `NaiveDateTime` as ISO 8601 with `T` separator so that
/// `NaiveDateTime::parse_from_str` (and its `FromStr` impl) can round-trip it.
///
/// `NaiveDateTime::to_string()` uses a space separator (`2026-10-01 02:30:18`),
/// which `NaiveDateTime::from_str` rejects — hence the explicit format here.
fn naive_to_iso(dt: &NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string()
}

/// Parses a `NaiveDateTime` from a DB string, tolerating both `T` and space
/// separators (guards against legacy rows written with `to_string()`).
fn parse_naive(s: &str, col: usize) -> rusqlite::Result<NaiveDateTime> {
    // Try the standard FromStr first (expects `T` separator).
    s.parse::<NaiveDateTime>()
        // Fall back to the space-separated format produced by Display.
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f"))
        .map_err(|e| conversion_err(col, rusqlite::types::Type::Text, e))
}

fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let date_start: String = row.get(3)?;
    let date_end: String = row.get(4)?;
    let status: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;

    Ok(Session {
        id: Uuid::parse_str(&id).map_err(|e| conversion_err(0, rusqlite::types::Type::Text, e))?,
        project_id: Uuid::parse_str(&project_id)
            .map_err(|e| conversion_err(1, rusqlite::types::Type::Text, e))?,
        name,
        date_start: parse_naive(&date_start, 3)?,
        date_end: parse_naive(&date_end, 4)?,
        status: match status.as_str() {
            "archived" => SessionStatus::Archived,
            _ => SessionStatus::Active,
        },
        created_at: created_at
            .parse()
            .map_err(|e| conversion_err(6, rusqlite::types::Type::Text, e))?,
        updated_at: updated_at
            .parse()
            .map_err(|e| conversion_err(7, rusqlite::types::Type::Text, e))?,
    })
}

fn row_to_image(row: &Row) -> rusqlite::Result<Image> {
    let id: String = row.get(0)?;
    let session_id: String = row.get(1)?;
    let file_path: String = row.get(2)?;
    let location_lat: f64 = row.get(3)?;
    let location_lon: f64 = row.get(4)?;
    let captured_at: String = row.get(5)?;

    Ok(Image {
        id: Uuid::parse_str(&id).map_err(|e| conversion_err(0, rusqlite::types::Type::Text, e))?,
        session_id: Uuid::parse_str(&session_id)
            .map_err(|e| conversion_err(1, rusqlite::types::Type::Text, e))?,
        file_path,
        location_lat,
        location_lon,
        captured_at: captured_at
            .parse()
            .map_err(|e| conversion_err(5, rusqlite::types::Type::Text, e))?,
    })
}

/// Wraps a parse error (UUID or `NaiveDateTime`) as a proper
/// `rusqlite::Error::FromSqlConversionFailure` for a given column, so
/// callers get a real `rusqlite::Error` instead of a made-up variant.
fn conversion_err(
    col: usize,
    ty: rusqlite::types::Type,
    err: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(col, ty, Box::new(err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(captured_at: &str) -> Image {
        Image {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            file_path: "test.jpg".to_string(),
            location_lat: 0.0,
            location_lon: 0.0,
            captured_at: captured_at.parse().unwrap(),
        }
    }

    #[test]
    fn compute_date_range_empty_is_none() {
        assert_eq!(compute_date_range(&[]), None);
    }

    #[test]
    fn compute_date_range_picks_min_and_max() {
        let images = vec![
            img("2026-09-20T10:00:00"),
            img("2026-09-18T07:15:00"),
            img("2026-09-25T09:40:00"),
        ];
        let (start, end) = compute_date_range(&images).unwrap();
        assert_eq!(
            start,
            "2026-09-18T07:15:00".parse::<NaiveDateTime>().unwrap()
        );
        assert_eq!(end, "2026-09-25T09:40:00".parse::<NaiveDateTime>().unwrap());
    }

    #[test]
    fn default_session_name_formats_timestamp() {
        let at: NaiveDateTime = "2026-09-18T07:12:00".parse().unwrap();
        assert_eq!(default_session_name(at), "Session - 2026-09-18 07:12");
    }
}
