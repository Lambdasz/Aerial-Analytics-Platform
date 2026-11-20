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
//!
//! Only JPEGs are ever queued (see
//! `flight_session::insert_images_and_recalculate`, which excludes every
//! DNG from the `enqueue` call). A DNG row can still appear here from a
//! database written before that exclusion existed; [`pending_for_session`]
//! treats such a row the same as any other row that can never be scored —
//! it is dequeued on the spot instead of being handed to the scorer.

use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use super::error::SessionError;

/// One image waiting for its blur score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingImage {
    pub image_id: Uuid,
    pub path: PathBuf,
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

/// The images of `session_id` that are still waiting for a score, oldest
/// first.
///
/// Matched through `image`, so a queue row whose image was deleted is not
/// returned, and an image moved here from another session is. A row that can
/// never be scored — an id that is not a UUID, or a format other than
/// `"jpeg"` (a DNG queued by a database from before DNGs were excluded at
/// insert time, or an unknown/NULL format) — is dequeued on the spot instead
/// of being skipped, so it cannot keep a session "pending" forever or make a
/// count disagree with the work list.
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
        match (Uuid::parse_str(&id), format.as_deref()) {
            (Ok(image_id), Some("jpeg")) => pending.push(PendingImage {
                image_id,
                path: PathBuf::from(path),
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

/// Copies a paired DNG's `blur_score` from its JPEG sibling.
///
/// A paired DNG is never queued (see `flight_session::insert_images_and_recalculate`,
/// which excludes it from the `enqueue` call) — its preview is the same
/// capture as its JPEG sibling, so decoding it a second time would be
/// wasted work for a score that just gets mirrored here. Call once after
/// the worker pool drains, before `reclassify_session_blur`.
///
/// Idempotent: only rows still missing a score are touched, so re-running
/// this is harmless. A sibling that is not yet scored itself (still queued,
/// or its scoring failed) leaves the DNG's score `NULL`, consistent with
/// how every other not-yet-scored image is represented.
///
/// # Purity
///
/// Impure — updates the `image` table.
pub(crate) fn copy_paired_dng_scores(
    conn: &Connection,
    session_id: Uuid,
) -> Result<(), SessionError> {
    conn.execute(
        "UPDATE image SET blur_score =
             (SELECT j.blur_score FROM image j WHERE j.id = image.raw_sibling_id)
         WHERE session_id = ?1 AND format = 'dng' AND raw_sibling_id IS NOT NULL
           AND blur_score IS NULL",
        params![session_id.to_string()],
    )?;
    Ok(())
}

/// Number of standalone DNGs (no same-stem JPEG sibling) in `session_id`.
///
/// A standalone DNG is never queued and never scored — see
/// [`blur_score_for_copy`](super::blur_detection::blur_score_for_copy)'s
/// module docs — so this is the count [`super::flight_session::reclassify_session_blur`]
/// excludes from its baseline (and clears any stale flag on), reported to
/// the background-task UI as deliberately skipped rather than silently
/// dropped.
///
/// # Purity
///
/// Impure — reads the `image` table.
pub(crate) fn standalone_dng_count(
    conn: &Connection,
    session_id: Uuid,
) -> Result<usize, SessionError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM image
         WHERE session_id = ?1 AND format = 'dng' AND raw_sibling_id IS NULL",
        params![session_id.to_string()],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::session::Image;
    use crate::modules::module_01::flight_session::{create_project, create_session, insert_image};

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        crate::modules::module_01::flight_session::init_schema(&conn).expect("init schema");
        migrate_blur_queue_schema(&conn).expect("migrate blur_queue schema");
        conn
    }

    fn image_row(session_id: Uuid, file_path: &str) -> Image {
        Image {
            id: Uuid::new_v4(),
            session_id,
            file_path: file_path.to_string(),
            location_lat: None,
            location_lon: None,
            captured_at: None,
            gps_altitude_m: None,
            width: None,
            height: None,
            format: Some("jpeg".to_string()),
            make: None,
            camera_model: None,
            source_path: None,
            content_hash: None,
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: Some(true),
            blur_score: None,
            is_blurry: None,
            heading_deg: None,
        }
    }

    #[test]
    fn copy_paired_dng_scores_copies_a_scored_jpegs_score_to_its_dng() {
        let conn = memory_db();
        let project = create_project(&conn, "proj".to_string(), None).expect("project");
        let session = create_session(&conn, project.id, None).expect("session");

        let dng_id = Uuid::new_v4();
        let mut jpeg = image_row(session.id, "/photos/pair.jpg");
        jpeg.blur_score = Some(42.0);
        jpeg.raw_sibling_id = Some(dng_id);
        let jpeg_id = jpeg.id;
        insert_image(&conn, &jpeg).expect("insert jpeg");

        let mut dng = image_row(session.id, "/photos/pair.dng");
        dng.id = dng_id;
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        insert_image(&conn, &dng).expect("insert dng");

        copy_paired_dng_scores(&conn, session.id).expect("copy scores");

        let dng_row =
            crate::modules::module_01::flight_session::get_image(&conn, dng_id).expect("get dng");
        assert_eq!(dng_row.blur_score, Some(42.0));
    }

    #[test]
    fn copy_paired_dng_scores_leaves_the_dng_null_when_the_jpeg_is_unscored() {
        let conn = memory_db();
        let project = create_project(&conn, "proj".to_string(), None).expect("project");
        let session = create_session(&conn, project.id, None).expect("session");

        let dng_id = Uuid::new_v4();
        let mut jpeg = image_row(session.id, "/photos/pair.jpg");
        jpeg.raw_sibling_id = Some(dng_id);
        let jpeg_id = jpeg.id;
        insert_image(&conn, &jpeg).expect("insert jpeg");

        let mut dng = image_row(session.id, "/photos/pair.dng");
        dng.id = dng_id;
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        insert_image(&conn, &dng).expect("insert dng");

        copy_paired_dng_scores(&conn, session.id).expect("copy scores");

        let dng_row =
            crate::modules::module_01::flight_session::get_image(&conn, dng_id).expect("get dng");
        assert_eq!(dng_row.blur_score, None);
    }

    #[test]
    fn pending_for_session_dequeues_a_legacy_dng_row_instead_of_returning_it() {
        // Simulates a `blur_queue` row written before DNGs were excluded at
        // insert time (or a row left over from an older build) — it must be
        // self-healed away rather than handed to a scorer that cannot
        // produce a meaningful score from a raw file.
        let conn = memory_db();
        let project = create_project(&conn, "proj".to_string(), None).expect("project");
        let session = create_session(&conn, project.id, None).expect("session");

        let mut dng = image_row(session.id, "/photos/lone.dng");
        dng.format = Some("dng".to_string());
        let dng_id = dng.id;
        insert_image(&conn, &dng).expect("insert dng");
        enqueue(&conn, session.id, &[dng_id]).expect("enqueue");

        let pending = pending_for_session(&conn, session.id).expect("pending");
        assert!(pending.is_empty(), "the dng row must not be scored");
        assert!(
            !is_queued(&conn, dng_id).expect("is_queued"),
            "the dng row must be dequeued, not merely skipped"
        );
    }

    #[test]
    fn standalone_dng_count_counts_only_dngs_without_a_sibling() {
        let conn = memory_db();
        let project = create_project(&conn, "proj".to_string(), None).expect("project");
        let session = create_session(&conn, project.id, None).expect("session");

        let dng_id = Uuid::new_v4();
        let mut jpeg = image_row(session.id, "/photos/pair.jpg");
        jpeg.raw_sibling_id = Some(dng_id);
        let jpeg_id = jpeg.id;
        insert_image(&conn, &jpeg).expect("insert jpeg");

        let mut paired_dng = image_row(session.id, "/photos/pair.dng");
        paired_dng.id = dng_id;
        paired_dng.format = Some("dng".to_string());
        paired_dng.raw_sibling_id = Some(jpeg_id);
        insert_image(&conn, &paired_dng).expect("insert paired dng");

        let mut standalone_dng = image_row(session.id, "/photos/lone.dng");
        standalone_dng.format = Some("dng".to_string());
        insert_image(&conn, &standalone_dng).expect("insert standalone dng");

        let count = standalone_dng_count(&conn, session.id).expect("count");
        assert_eq!(
            count, 1,
            "only the standalone dng counts; jpeg and paired dng do not"
        );
    }
}
