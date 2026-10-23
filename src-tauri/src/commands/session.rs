#![allow(unused_variables)]

use uuid::Uuid;

use crate::models::{Session, SessionStatus};
use crate::modules::module_01::error::SessionError;
use crate::modules::module_01::{self as flight_session, import_io};
use crate::plugin_manager::error::CommandError;
use crate::AppState;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    SessionError::LockPoisoned(e.to_string()).into()
}

/// Creates a new, empty session (no images assigned yet).
#[tauri::command]
pub fn create_session(
    state: tauri::State<AppState>,
    project_id: Uuid,
    name: Option<String>,
) -> Result<Session, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::create_session(&conn, project_id, name)?)
}

/// Fetches a single session by id.
#[tauri::command]
pub fn get_session(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<Session, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::get_session(&conn, session_id)?)
}

/// Lists every session belonging to a project.
#[tauri::command]
pub fn get_sessions_by_project(
    state: tauri::State<AppState>,
    project_id: Uuid,
) -> Result<Vec<Session>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::get_sessions_by_project(&conn, project_id)?)
}

/// Overwrites a session's auto-generated name with a user-provided one.
#[tauri::command]
pub fn update_session_name(
    state: tauri::State<AppState>,
    session_id: Uuid,
    new_name: String,
) -> Result<Session, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::update_session_name(
        &conn, session_id, new_name,
    )?)
}

/// Transitions a session between `active` and `archived`.
#[tauri::command]
pub fn update_session_status(
    state: tauri::State<AppState>,
    session_id: Uuid,
    status: SessionStatus,
) -> Result<Session, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::update_session_status(
        &conn, session_id, status,
    )?)
}

/// Called when the user uploads an image and picks an existing session.
#[tauri::command]
pub fn assign_image_to_session(
    state: tauri::State<AppState>,
    session_id: Uuid,
    image_id: Uuid,
) -> Result<(), CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::assign_image_to_session(
        &conn, session_id, image_id,
    )?)
}

/// Recomputes date_start/date_end from the EXIF captured_at of every image
/// in this session. Called whenever an image is added to or removed from it.
#[tauri::command]
pub fn recalculate_session_date_range(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<(), CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::recalculate_session_date_range(
        &conn, session_id,
    )?)
}

/// Deletes a session, its image rows, and the copies it imported.
///
/// Cascade is the decided behaviour: a session only owns the copies it
/// imported. Because that is destructive, `confirm` must be `true`; the UI
/// asks the user first, and the command refuses without it
/// (`CONFIRMATION_REQUIRED`) rather than deleting on a stray call.
///
/// The rows are deleted first: if removing the folder fails, the library
/// holds an orphan rather than the UI holding a session that points at
/// deleted files. Unremovable files are then moved into `.trash/` so the user
/// can still undo; only when even that fails does the UI need to tell the
/// user a few copies remain on disk unreachable by the app.
#[tauri::command]
pub fn delete_session(
    state: tauri::State<AppState>,
    session_id: Uuid,
    confirm: bool,
) -> Result<DeleteOutcome, CommandError> {
    delete_session_impl(&state, session_id, confirm)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    /// `true` when some copied files could not be removed or rescued into
    /// `.trash/` and remain on disk under the session's folder. The UI
    /// should surface this to the user.
    pub files_left: bool,
}

/// Testable core of [`delete_session`]: identical behaviour against an owned
/// [`AppState`], without Tauri plumbing.
pub(crate) fn delete_session_impl(
    state: &AppState,
    session_id: Uuid,
    confirm: bool,
) -> Result<DeleteOutcome, CommandError> {
    if !confirm {
        return Err(CommandError {
            code: "CONFIRMATION_REQUIRED".to_string(),
            message: "deleting a session also deletes its imported images; pass confirm=true after asking the user".to_string(),
        });
    }
    let project_id = {
        let conn = state.db.lock().map_err(lock_err)?;
        let session = flight_session::get_session(&conn, session_id)?;
        flight_session::delete_session(&conn, session_id)?;
        session.project_id
    };
    let project = project_id.to_string();
    let session = session_id.to_string();
    if import_io::remove_session_files(&state.library_root, &project, &session) {
        return Ok(DeleteOutcome { files_left: false });
    }
    // Removing failed part-way. The rows are gone, so the background sweep
    // will never collect these files: rescue them into `.trash/`, and only
    // warn the UI if even that is impossible.
    if import_io::trash_session_folder(&state.library_root, &project, &session) {
        return Ok(DeleteOutcome { files_left: false });
    }
    eprintln!(
        "warning: some copies of a deleted session stay on disk outside \
         .trash; remove them manually or retry"
    );
    Ok(DeleteOutcome { files_left: true })
}
