#![allow(unused_variables)]

use uuid::Uuid;

use crate::models::{Session, SessionStatus};
use crate::modules::module_01 as flight_session;
use crate::plugin_manager::error::CommandError;
use crate::AppState;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DB_LOCK_POISONED".to_string(),
        message: e.to_string(),
    }
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

/// The behavior for deleting a session that still has images assigned to it
/// has not been decided yet and needs further discussion with the team.
#[tauri::command]
pub fn delete_session(state: tauri::State<AppState>, session_id: Uuid) -> Result<(), CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::delete_session(&conn, session_id)?)
}
