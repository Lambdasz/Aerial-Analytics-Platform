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

/// Membuat session baru, kosong (belum ada image).
#[tauri::command]
pub fn create_session(
    state: tauri::State<AppState>,
    project_id: Uuid,
    name: Option<String>,
) -> Result<Session, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::create_session(&conn, project_id, name)?)
}

/// Ambil 1 session by id.
#[tauri::command]
pub fn get_session(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<Session, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::get_session(&conn, session_id)?)
}

/// Ambil semua session dalam 1 project.
#[tauri::command]
pub fn get_sessions_by_project(
    state: tauri::State<AppState>,
    project_id: Uuid,
) -> Result<Vec<Session>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::get_sessions_by_project(&conn, project_id)?)
}

/// User overwrite nama session yang auto-generate.
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

/// Ubah status active <-> archived.
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

/// Dipanggil saat user upload dan pilih session existing.
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

/// Hitung ulang date_start/date_end dari EXIF captured_at semua image di session ini.
/// Dipanggil setiap ada image masuk atau keluar.
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

/// Perilaku penghapusan session yang masih memiliki image terkait belum ditentukan
/// dan memerlukan pembahasan lebih lanjut dengan tim.
#[tauri::command]
pub fn delete_session(state: tauri::State<AppState>, session_id: Uuid) -> Result<(), CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::delete_session(&conn, session_id)?)
}
