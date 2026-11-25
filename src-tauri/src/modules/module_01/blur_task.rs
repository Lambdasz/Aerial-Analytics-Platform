//! Background blur scoring (Module 1).
//!
//! [`score_pending`] drains a session's [`super::blur_queue`] on a few worker
//! threads. It has no Tauri types: the command layer wraps it with a task
//! registry and events, and tests call it directly (like
//! `warm_imported_thumbnails`).

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use rusqlite::Connection;
use uuid::Uuid;

use super::blur_detection;
use super::blur_queue::{self, PendingImage};
use super::error::SessionError;
use super::flight_session::reclassify_session_blur;

/// Upper bound on concurrent scoring threads.
///
/// Scoring decodes a full-resolution frame (~60–100 MB for a 20 MP JPEG), so
/// the cap bounds memory as well as CPU use. The decode is the dominant
/// cost, which is why it runs outside the database lock.
const MAX_BLUR_WORKERS: usize = 4;

/// Progress after one image finished, as reported to `on_update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlurProgress {
    /// Images finished so far (scored or found unscorable).
    pub done: usize,
    /// Images this run set out to score.
    pub total: usize,
    /// Finished images that produced no score (too small or undecodable).
    pub unscorable: usize,
    /// File name of the image that just finished.
    pub current: Option<String>,
}

/// How a [`score_pending`] run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlurSummary {
    /// `true` when the run stopped because `cancel` was set. Unfinished
    /// images stay queued and resume on the next launch.
    pub cancelled: bool,
    pub done: usize,
    pub total: usize,
    pub unscorable: usize,
}

fn worker_count(total: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    cores.min(MAX_BLUR_WORKERS).min(total).max(1)
}

fn lock(db: &Mutex<Connection>) -> Result<std::sync::MutexGuard<'_, Connection>, SessionError> {
    db.lock()
        .map_err(|err| SessionError::LockPoisoned(err.to_string()))
}

/// Number of images [`score_pending`] would process for `session_id`.
///
/// Computed from the same work list `score_pending` uses, so the two cannot
/// disagree about what is pending.
pub(crate) fn pending_total(
    db: &Mutex<Connection>,
    session_id: Uuid,
) -> Result<usize, SessionError> {
    let conn = lock(db)?;
    Ok(blur_queue::pending_for_session(&conn, session_id)?.len())
}

/// Scores every queued image of `session_id`, a few at a time.
///
/// Each worker takes the next image, scores it without holding the database
/// lock, then locks only long enough to store the score and dequeue it.
/// `cancel` is checked before every image; an image already being decoded
/// finishes. After the workers stop (cancelled or not), the session's
/// `is_blurry` flags are re-derived once from the scores stored so far.
///
/// `on_update` runs on worker threads after each finished image.
///
/// # Errors
///
/// [`SessionError`] when the queue cannot be read, a score cannot be stored,
/// or the database lock is poisoned. Images that cannot be scored are not an
/// error: they are counted in [`BlurSummary::unscorable`] and dequeued.
pub(crate) fn score_pending(
    db: &Mutex<Connection>,
    session_id: Uuid,
    cancel: &AtomicBool,
    on_update: impl Fn(BlurProgress) + Sync,
) -> Result<BlurSummary, SessionError> {
    score_pending_with(
        db,
        session_id,
        cancel,
        &blur_detection::blur_score_for_copy,
        on_update,
    )
}

/// [`score_pending`] with the scoring function supplied, so a test can make
/// it panic.
///
/// A panic inside `scorer` is caught per image and counts as an unscorable
/// image: without that it would unwind out of the worker scope and leave the
/// caller's task registered as running forever. (Release builds use
/// `panic = "abort"`, where a panic ends the process regardless.)
pub(crate) fn score_pending_with(
    db: &Mutex<Connection>,
    session_id: Uuid,
    cancel: &AtomicBool,
    scorer: &(dyn Fn(&Path) -> Option<f64> + Sync),
    on_update: impl Fn(BlurProgress) + Sync,
) -> Result<BlurSummary, SessionError> {
    let work: Vec<PendingImage> = {
        let conn = lock(db)?;
        blur_queue::pending_for_session(&conn, session_id)?
    };
    let total = work.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let unscorable = AtomicUsize::new(0);
    let failure: Mutex<Option<SessionError>> = Mutex::new(None);

    std::thread::scope(|scope| {
        for _ in 0..worker_count(total) {
            scope.spawn(|| loop {
                if cancel.load(Ordering::Relaxed) || failure.lock().is_ok_and(|f| f.is_some()) {
                    return;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = work.get(index) else {
                    return;
                };
                let score =
                    catch_unwind(AssertUnwindSafe(|| scorer(&item.path))).unwrap_or_else(|_| {
                        eprintln!(
                            "warning: blur scoring panicked; treating the image as unscorable"
                        );
                        None
                    });
                let stored =
                    lock(db).and_then(|conn| blur_queue::record_score(&conn, item.image_id, score));
                if let Err(err) = stored {
                    if let Ok(mut slot) = failure.lock() {
                        slot.get_or_insert(err);
                    }
                    return;
                }
                if score.is_none() {
                    unscorable.fetch_add(1, Ordering::Relaxed);
                }
                let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                on_update(BlurProgress {
                    done: finished,
                    total,
                    unscorable: unscorable.load(Ordering::Relaxed),
                    current: item
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned()),
                });
            });
        }
    });

    // Re-derive every `is_blurry` flag from the scores stored so far — also
    // when a worker failed, so the images that were scored do not keep stale
    // flags. Stored scores stay valid if this pass fails, so it is logged
    // rather than failing the run.
    match lock(db) {
        Ok(conn) => {
            // A paired DNG was never queued (see `insert_images_and_recalculate`),
            // so its score only exists once copied from its scored JPEG sibling.
            if let Err(err) = blur_queue::copy_paired_dng_scores(&conn, session_id) {
                eprintln!("warning: could not copy paired dng blur scores: {err}");
            }
            if let Err(err) =
                reclassify_session_blur(&conn, session_id, blur_detection::classify_with_stats)
            {
                eprintln!("warning: could not reclassify session blur flags: {err}");
            }
        }
        Err(err) => eprintln!("warning: could not reclassify session blur flags: {err}"),
    }

    if let Some(err) = failure.into_inner().ok().flatten() {
        return Err(err);
    }

    Ok(BlurSummary {
        cancelled: cancel.load(Ordering::Relaxed) && done.load(Ordering::Relaxed) < total,
        done: done.load(Ordering::Relaxed),
        total,
        unscorable: unscorable.load(Ordering::Relaxed),
    })
}
