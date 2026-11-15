//! Pending-blur-score queue (Module 1).
//!
//! Importing an image no longer scores its blur inline: the row is inserted
//! with `blur_score = NULL` and its id is queued here. A background task
//! ([`super::blur_task`]) drains the queue.
//!
//! A queue table is used instead of "`blur_score IS NULL`" because NULL is
//! ambiguous in the `image` table — it also means *too small to score*,
//! *undecodable*, or *imported before scoring existed*. Only an id in this
//! table is still waiting for a score, so a restart can resume exactly the
//! unfinished work and never re-decodes images that can never be scored.
//!
//! The table is a side table (not an `image` column) so no `image`-table
//! rebuild migration is involved. Every query matches a queue row to its
//! image by id and takes the session from the **image row**, so an image
//! moved to another session carries its pending state along. Rows whose
//! image no longer exists are ignored and pruned by [`pending_sessions`].

use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::image::ImageFormat;

use super::error::SessionError;

/// One image waiting for its blur score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingImage {
    pub image_id: Uuid,
    pub path: PathBuf,
    pub format: ImageFormat,
}

/// Creates the `blur_queue` table if it does not exist.
///
/// # Purity
///
/// Impure — DDL against the database.
pub fn migrate_blur_queue_schema(conn: &Connection) -> Result<(), SessionError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blur_queue (
            image_id   TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            queued_at  TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_blur_queue_session ON blur_queue(session_id)",
        [],
    )?;
    Ok(())
}

/// Queues `image_ids` of `session_id` for background scoring.
///
/// Idempotent: an id that is already queued is left alone. The stored
/// `session_id` is informational; the image row decides which session an
/// image belongs to.
pub(crate) fn enqueue(
    conn: &Connection,
    session_id: Uuid,
    image_ids: &[Uuid],
) -> Result<(), SessionError> {
    let queued_at = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare_cached(
        "INSERT OR IGNORE INTO blur_queue (image_id, session_id, queued_at) VALUES (?1, ?2, ?3)",
    )?;
    for id in image_ids {
        stmt.execute(params![id.to_string(), session_id.to_string(), queued_at])?;
    }
    Ok(())
}

fn format_from_db(raw: Option<&str>) -> Option<ImageFormat> {
    match raw {
        Some("jpeg") => Some(ImageFormat::Jpeg),
        Some("dng") => Some(ImageFormat::Dng),
        _ => None,
    }
}

/// The images of `session_id` that are still waiting for a score, oldest
/// first.
///
/// Matched through `image`, so a queue row whose image was deleted is not
/// returned, and an image moved here from another session is. A row that can
/// never be scored — an id that is not a UUID or an unknown format — is
/// dequeued on the spot instead of being skipped, so it cannot keep a
/// session "pending" forever or make a count disagree with the work list.
pub(crate) fn pending_for_session(
    conn: &Connection,
    session_id: Uuid,
) -> Result<Vec<PendingImage>, SessionError> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.file_path, i.format
         FROM blur_queue q
         JOIN image i ON i.id = q.image_id
         WHERE i.session_id = ?1
         ORDER BY q.queued_at, i.id",
    )?;
    let rows = stmt
        .query_map(params![session_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut pending = Vec::new();
    for (id, path, format) in rows {
        match (Uuid::parse_str(&id), format_from_db(format.as_deref())) {
            (Ok(image_id), Some(format)) => pending.push(PendingImage {
                image_id,
                path: PathBuf::from(path),
                format,
            }),
            _ => {
                conn.execute("DELETE FROM blur_queue WHERE image_id = ?1", params![id])?;
            }
        }
    }
    Ok(pending)
}

/// Whether `image_id` is still waiting for a score.
pub(crate) fn is_queued(conn: &Connection, image_id: Uuid) -> Result<bool, SessionError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM blur_queue WHERE image_id = ?1",
        params![image_id.to_string()],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Sessions that still have queued images, by the session each image is in
/// now.
///
/// Prunes queue rows whose image no longer exists first, so a deleted image
/// cannot keep a session "pending" forever.
pub(crate) fn pending_sessions(conn: &Connection) -> Result<Vec<Uuid>, SessionError> {
    conn.execute(
        "DELETE FROM blur_queue
         WHERE NOT EXISTS (SELECT 1 FROM image i WHERE i.id = blur_queue.image_id)",
        [],
    )?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT i.session_id FROM blur_queue q
         JOIN image i ON i.id = q.image_id
         ORDER BY i.session_id",
    )?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(Result::ok)
        .filter_map(|raw| Uuid::parse_str(&raw).ok())
        .collect();
    Ok(ids)
}

/// Stores `score` (`None` = unscorable) for `image_id` and dequeues it.
///
/// A `None` score is final: the row leaves the queue, so an image that is too
/// small or cannot be decoded is not retried on every launch. The two
/// statements are idempotent, so no transaction is needed — if the process
/// dies between them the image is simply scored again on the next run.
pub(crate) fn record_score(
    conn: &Connection,
    image_id: Uuid,
    score: Option<f64>,
) -> Result<(), SessionError> {
    let id = image_id.to_string();
    conn.execute(
        "UPDATE image SET blur_score = ?1 WHERE id = ?2",
        params![score, id],
    )?;
    conn.execute("DELETE FROM blur_queue WHERE image_id = ?1", params![id])?;
    Ok(())
}
