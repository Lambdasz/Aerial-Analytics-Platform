#![allow(unused_variables)]

use uuid::Uuid;

use crate::models::Project;
use crate::modules::module_01 as flight_session;
use crate::plugin_manager::error::CommandError;
use crate::AppState;

fn lock_err(e: impl std::fmt::Display) -> CommandError {
    CommandError {
        code: "DB_LOCK_POISONED".to_string(),
        message: e.to_string(),
    }
}

/// Creates a new mapping project.
#[tauri::command]
pub fn create_project(
    state: tauri::State<AppState>,
    name: String,
    description: Option<String>,
) -> Result<Project, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::create_project(&conn, name, description)?)
}

/// Fetches a single project by id.
#[tauri::command]
pub fn get_project(
    state: tauri::State<AppState>,
    project_id: Uuid,
) -> Result<Project, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::get_project(&conn, project_id)?)
}

/// Lists every project.
#[tauri::command]
pub fn list_projects(state: tauri::State<AppState>) -> Result<Vec<Project>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::list_projects(&conn)?)
}

/// Updates a project's name and description.
#[tauri::command]
pub fn update_project(
    state: tauri::State<AppState>,
    project_id: Uuid,
    name: String,
    description: Option<String>,
) -> Result<Project, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::update_project(
        &conn,
        project_id,
        name,
        description,
    )?)
}

/// Deletes a project and all its sessions.
#[tauri::command]
pub fn delete_project(state: tauri::State<AppState>, project_id: Uuid) -> Result<(), CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::delete_project(&conn, project_id)?)
}

/// Counts images across all sessions in a project.
#[tauri::command]
pub fn count_images_by_project(
    state: tauri::State<AppState>,
    project_id: Uuid,
) -> Result<u32, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::count_images_by_project(&conn, project_id)?)
}
