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

/// Scores a session's queued images as a registered task and blocks until it
/// ends. `emit` receives a snapshot on start, after every finished image, and
/// at the end.
///
/// Returns `Ok(None)` when there was nothing to do: no queued images, or a
/// blur task for this session is already live.
///
/// A failed run is reported in the returned (and emitted) snapshot with state
/// `failed`; only a failure before the task could be registered is an `Err`.
pub(crate) fn run_blur_task_impl(
    db: &Mutex<Connection>,
    tasks: &BackgroundTasks,
    session_id: Uuid,
    emit: impl Fn(&TaskSnapshot) + Sync,
) -> Result<Option<TaskSnapshot>, CommandError> {
    let total = pending_total(db, session_id)?;
    if total == 0 {
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
    ) else {
        return Ok(None);
    };
    let task_id = started.snapshot.id.clone();
    emit(&started.snapshot);

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
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |snapshot: &TaskSnapshot| {
            let _ = app.emit(BACKGROUND_TASK_EVENT, snapshot);
        };
        for session_id in session_ids {
            if let Err(err) = run_blur_task_impl(&db, &tasks, session_id, emit) {
                eprintln!("warning: blur scoring for session {session_id} failed to start: {err}");
            }
        }
    });
}

/// Resumes blur scoring for every session that still has queued images, for
/// example after the app was closed mid-run. Safe to call at startup.
pub fn resume_pending_blur_tasks(app: &AppHandle) {
    let sessions = {
        let state = app.state::<AppState>();
        let Ok(conn) = state.db.lock() else {
            eprintln!("warning: could not resume blur scoring: db lock");
            return;
        };
        match blur_queue::pending_sessions(&conn) {
            Ok(sessions) => sessions,
            Err(err) => {
                eprintln!("warning: could not resume blur scoring: {err}");
                return;
            }
        }
    };
    spawn_blur_tasks(app, sessions);
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
