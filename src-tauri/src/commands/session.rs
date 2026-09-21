#![allow(unused_variables)]
use uuid::Uuid;

use crate::models::{Session, SessionStatus};

#[tauri::command]
pub fn create_session(project_id: Uuid, name: Option<String>) -> Result<Session, String> {
    todo!()
}

#[tauri::command]
pub fn get_session(session_id: Uuid) -> Result<Session, String> {
    todo!()
}

#[tauri::command]
pub fn get_sessions_by_project(project_id: Uuid) -> Result<Vec<Session>, String> {
    todo!()
}

#[tauri::command]
pub fn update_session_name(session_id: Uuid, new_name: String) -> Result<Session, String> {
    todo!()
}

#[tauri::command]
pub fn update_session_status(session_id: Uuid, status: SessionStatus) -> Result<Session, String> {
    todo!()
}

#[tauri::command]
pub fn assign_image_to_session(session_id: Uuid, image_id: Uuid) -> Result<(), String> {
    todo!()
}

#[tauri::command]
pub fn recalculate_session_date_range(session_id: Uuid) -> Result<(), String> {
    todo!()
}

#[tauri::command]
pub fn delete_session(session_id: Uuid) -> Result<(), String> {
    todo!()
}
