#![allow(unused_variables)]

use uuid::Uuid;

use crate::models::{Session, SessionStatus};
use crate::modules::module_01 as flight_session;
use crate::AppState;

/// Membuat session baru, kosong (belum ada image).
#[tauri::command]
pub fn create_session(
    state: tauri::State<AppState>,
    project_id: Uuid,
    name: Option<String>,
) -> Result<Session, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::create_session(&conn, project_id, name).map_err(|e| e.to_string())
}

/// Ambil 1 session by id.
#[tauri::command]
pub fn get_session(state: tauri::State<AppState>, session_id: Uuid) -> Result<Session, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::get_session(&conn, session_id).map_err(|e| e.to_string())
}

/// Ambil semua session dalam 1 project.
#[tauri::command]
pub fn get_sessions_by_project(
    state: tauri::State<AppState>,
    project_id: Uuid,
) -> Result<Vec<Session>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::get_sessions_by_project(&conn, project_id).map_err(|e| e.to_string())
}

/// User overwrite nama session yang auto-generate.
#[tauri::command]
pub fn update_session_name(
    state: tauri::State<AppState>,
    session_id: Uuid,
    new_name: String,
) -> Result<Session, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::update_session_name(&conn, session_id, new_name).map_err(|e| e.to_string())
}

/// Ubah status active <-> archived.
#[tauri::command]
pub fn update_session_status(
    state: tauri::State<AppState>,
    session_id: Uuid,
    status: SessionStatus,
) -> Result<Session, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::update_session_status(&conn, session_id, status).map_err(|e| e.to_string())
}

/// Dipanggil saat user upload dan pilih session existing.
#[tauri::command]
pub fn assign_image_to_session(
    state: tauri::State<AppState>,
    session_id: Uuid,
    image_id: Uuid,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::assign_image_to_session(&conn, session_id, image_id).map_err(|e| e.to_string())
}

/// Hitung ulang date_start/date_end dari EXIF captured_at semua image di session ini.
/// Dipanggil setiap ada image masuk atau keluar.
#[tauri::command]
pub fn recalculate_session_date_range(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::recalculate_session_date_range(&conn, session_id).map_err(|e| e.to_string())
}

/// Perilaku penghapusan session yang masih memiliki image terkait belum ditentukan
/// dan memerlukan pembahasan lebih lanjut dengan tim.
#[tauri::command]
pub fn delete_session(state: tauri::State<AppState>, session_id: Uuid) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    flight_session::delete_session(&conn, session_id).map_err(|e| e.to_string())
}
