//! Background task commands and the blur-scoring task runner.
//!
//! Imports no longer score blur inline. Instead the images are queued
//! ([`crate::modules::module_01::blur_queue`]) and [`spawn_blur_tasks`] runs
//! [`crate::modules::module_01::blur_task::score_pending`] as a watchable task
//! in the [`BackgroundTasks`] registry, emitting
//! [`BACKGROUND_TASK_EVENT`] after every change. The frontend lists the
//! current tasks with [`list_background_tasks`] and then follows the events.
//!
//! [`run_blur_task_impl`] holds the logic against plain handles so the e2e
//! tests can drive it without a Tauri runtime.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::models::background_task::{TaskKind, TaskSnapshot, TaskState, BACKGROUND_TASK_EVENT};
use crate::modules::module_01::blur_queue;
use crate::modules::module_01::blur_task::{pending_total, score_pending};
use crate::modules::module_01::error::SessionError;
use crate::modules::module_01::flight_session::get_session;
use crate::plugin_manager::error::CommandError;
use crate::services::background_tasks::BackgroundTasks;
use crate::AppState;

/// Title shown for blur-scoring tasks.
const BLUR_TASK_TITLE: &str = "Menilai ketajaman citra";

/// One scoring pass: registers a task for the session's queued images, scores
/// them, and finishes the task. Returns `Ok(None)` when there was nothing to
/// do, or a blur task for this session is already live.
///
/// `skip_already_reported` suppresses the standalone-DNG count: pass `true`
/// once a prior pass in the same [`run_blur_task_impl`] call already carried
/// it (either in a real pass's `skipped` field or as its own skip-only
/// pass), so a drained queue that still has standalone DNGs sitting in the
/// `image` table — permanent, not something later passes consume — does not
/// register a second, redundant skip-only task that would overwrite the real
/// result as the loop's `last` snapshot.
fn run_blur_pass(
    db: &Mutex<Connection>,
    tasks: &BackgroundTasks,
    session_id: Uuid,
    emit: &(impl Fn(&TaskSnapshot) + Sync),
    skip_already_reported: bool,
) -> Result<Option<TaskSnapshot>, CommandError> {
    let total = pending_total(db, session_id)?;
    let skipped = if skip_already_reported {
        0
    } else {
        let conn = db
            .lock()
            .map_err(|err| SessionError::LockPoisoned(err.to_string()))?;
        blur_queue::standalone_dng_count(&conn, session_id)?
    };
    if total == 0 && skipped == 0 {
        return Ok(None);
    }
    let session_name = {
        let conn = db
            .lock()
            .map_err(|err| SessionError::LockPoisoned(err.to_string()))?;
        get_session(&conn, session_id)?.name
    };
    let Some(started) = tasks.try_start(
        TaskKind::BlurScoring,
        BLUR_TASK_TITLE,
        &session_id.to_string(),
        &session_name,
        total,
        skipped,
    ) else {
        return Ok(None);
    };
    let task_id = started.snapshot.id.clone();
    emit(&started.snapshot);

    // Nothing queued — the session has only standalone DNGs. There is no
    // scoring to run; report the skip and finish immediately so this pass
    // does not try to drain an empty queue.
    if total == 0 {
        let finished = tasks.finish(&task_id, TaskState::Done, None);
        if let Some(snapshot) = &finished {
            emit(snapshot);
        }
        return Ok(finished);
    }

    let outcome = score_pending(db, session_id, &started.cancel, |progress| {
        // Workers finish out of order; progress only ever moves forward.
        let snapshot = tasks.update(&task_id, |task| {
            task.done = task.done.max(progress.done);
            task.unscorable = task.unscorable.max(progress.unscorable);
            task.total = progress.total;
            task.current = progress.current;
        });
        if let Some(snapshot) = snapshot {
            emit(&snapshot);
        }
    });

    let finished = match outcome {
        Ok(summary) => {
            let state = if summary.cancelled {
                TaskState::Cancelled
            } else if summary.unscorable > 0 {
                TaskState::Warn
            } else {
                TaskState::Done
            };
            tasks.update(&task_id, |task| {
                task.done = summary.done;
                task.total = summary.total;
                task.unscorable = summary.unscorable;
            });
            tasks.finish(&task_id, state, None)
        }
        Err(err) => tasks.finish(&task_id, TaskState::Failed, Some(err.to_string())),
    };
    if let Some(snapshot) = &finished {
        emit(snapshot);
    }
    Ok(finished)
}

/// Scores a session's queued images as registered tasks and blocks until the
/// queue is empty. `emit` receives a snapshot on start, after every finished
/// image, and at the end of each pass.
///
/// Returns the last pass's final snapshot, or `Ok(None)` when there was
/// nothing to do: no queued images, or a blur task for this session is
/// already live.
///
/// It keeps going while images are queued. An import into a session whose
/// task is still running cannot start a second task (see
/// [`BackgroundTasks::try_start`]); its rows are queued before this task is
/// finished, so the check after each pass finds them. A cancelled or failed
/// pass stops the loop.
///
/// A failed pass is reported in the returned (and emitted) snapshot with
/// state `failed`; only a failure before a task could be registered is an
/// `Err`.
pub(crate) fn run_blur_task_impl(
    db: &Mutex<Connection>,
    tasks: &BackgroundTasks,
    session_id: Uuid,
    emit: impl Fn(&TaskSnapshot) + Sync,
) -> Result<Option<TaskSnapshot>, CommandError> {
    let mut last = None;
    let mut skip_already_reported = false;
    while let Some(snapshot) = run_blur_pass(db, tasks, session_id, &emit, skip_already_reported)? {
        skip_already_reported = true;
        // A pass with `total == 0` only reported standalone DNGs that were
        // skipped — there was nothing to score, so another pass would just
        // register the same skip-only task again forever.
        let more = snapshot.total > 0
            && !matches!(snapshot.state, TaskState::Cancelled | TaskState::Failed);
        last = Some(snapshot);
        if !more {
            break;
        }
    }
    Ok(last)
}

/// Runs blur-scoring tasks for `session_ids` in order, on the calling thread.
fn run_sessions(
    app: &AppHandle,
    db: &Mutex<Connection>,
    tasks: &BackgroundTasks,
    session_ids: Vec<Uuid>,
) {
    let emit = |snapshot: &TaskSnapshot| {
        let _ = app.emit(BACKGROUND_TASK_EVENT, snapshot);
    };
    for session_id in session_ids {
        if let Err(err) = run_blur_task_impl(db, tasks, session_id, emit) {
            // The code only: the message for a missing session carries the
            // session id, which must not reach the logs.
            eprintln!("warning: blur scoring failed to start ({})", err.code);
        }
    }
}

/// Runs blur-scoring tasks for `session_ids`, one session after another, on a
/// blocking thread. Returns immediately.
///
/// Sessions run in sequence so a large backlog does not multiply the worker
/// threads (each task already uses several). Errors are logged: the images
/// stay queued and are retried on the next launch.
pub fn spawn_blur_tasks(app: &AppHandle, session_ids: Vec<Uuid>) {
    if session_ids.is_empty() {
        return;
    }
    let db = Arc::clone(&app.state::<AppState>().db);
    let tasks = app.state::<BackgroundTasks>().inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || run_sessions(&app, &db, &tasks, session_ids));
}

/// Resumes blur scoring for every session that still has queued images, for
/// example after the app was closed mid-run.
///
/// Returns immediately: the queue lookup takes the shared database lock, so
/// it runs on the blocking thread rather than delaying startup.
pub fn resume_pending_blur_tasks(app: &AppHandle) {
    let db = Arc::clone(&app.state::<AppState>().db);
    let tasks = app.state::<BackgroundTasks>().inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let sessions = match db.lock() {
            Ok(conn) => match blur_queue::pending_sessions(&conn) {
                Ok(sessions) => sessions,
                Err(err) => {
                    eprintln!("warning: could not resume blur scoring: {err}");
                    return;
                }
            },
            Err(_) => {
                eprintln!("warning: could not resume blur scoring: db lock");
                return;
            }
        };
        run_sessions(&app, &db, &tasks, sessions);
    });
}

/// Lists every background task: live ones first, newest first within each
/// group.
#[tauri::command]
pub fn list_background_tasks(tasks: State<'_, BackgroundTasks>) -> Vec<TaskSnapshot> {
    tasks.list()
}

/// Asks a background task to stop after the images it is working on.
///
/// Unfinished images stay queued and resume on the next launch.
///
/// # Errors
///
/// `TASK_NOT_FOUND` when no task has this id.
#[tauri::command]
pub fn cancel_background_task(
    tasks: State<'_, BackgroundTasks>,
    task_id: String,
) -> Result<(), CommandError> {
    tasks.cancel(&task_id)
}

/// Forgets every finished task and returns the removed ids.
#[tauri::command]
pub fn clear_finished_background_tasks(tasks: State<'_, BackgroundTasks>) -> Vec<String> {
    tasks.clear_finished()
}
