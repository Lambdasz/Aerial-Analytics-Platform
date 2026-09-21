#![allow(unused_variables)]

use uuid::Uuid;

use crate::models::{Session, SessionStatus};

#[tauri::command]
pub fn create_session(project_id: Uuid, name: Option<String>) -> Result<Session, String> {
    Err("create_session: not implemented".to_string())
}

#[tauri::command]
pub fn get_session(session_id: Uuid) -> Result<Session, String> {
    Err("get_session: not implemented".to_string())
}

#[tauri::command]
pub fn get_sessions_by_project(project_id: Uuid) -> Result<Vec<Session>, String> {
    Err("get_sessions_by_project: not implemented".to_string())
}

#[tauri::command]
pub fn update_session_name(session_id: Uuid, new_name: String) -> Result<Session, String> {
    Err("update_session_name: not implemented".to_string())
}

#[tauri::command]
pub fn update_session_status(session_id: Uuid, status: SessionStatus) -> Result<Session, String> {
    Err("update_session_status: not implemented".to_string())
}

#[tauri::command]
pub fn assign_image_to_session(session_id: Uuid, image_id: Uuid) -> Result<(), String> {
    Err("assign_image_to_session: not implemented".to_string())
}

#[tauri::command]
pub fn recalculate_session_date_range(session_id: Uuid) -> Result<(), String> {
    Err("recalculate_session_date_range: not implemented".to_string())
}

#[tauri::command]
pub fn delete_session(session_id: Uuid) -> Result<(), String> {
    Err("delete_session: not implemented".to_string())
}
