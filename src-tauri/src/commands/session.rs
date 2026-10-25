#![allow(unused_variables)]

use std::path::Path;

use uuid::Uuid;

use crate::models::{Image, Session, SessionStatus};
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

/// Lists the images stored in a session, including the metadata import recorded.
#[tauri::command]
pub fn get_images_by_session(
    state: tauri::State<AppState>,
    session_id: Uuid,
) -> Result<Vec<crate::models::Image>, CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    Ok(flight_session::get_images_by_session(&conn, session_id)?)
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

/// The result of deleting one image.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteImageOutcome {
    /// `true` when the copy could not be removed or rescued into `.trash/`
    /// and remains on disk under the session's folder.
    pub files_left: bool,
}

/// Deletes one image: its row, its pair link, and its copied file.
///
/// Destructive, so `confirm` must be `true`; the UI asks the user first and
/// the command refuses without it (`CONFIRMATION_REQUIRED`). The row goes
/// first (in one transaction, unlinking a JPEG/DNG partner and demoting a
/// surviving lone DNG); the file is then removed, or rescued into `.trash/`
/// when a direct removal fails.
///
/// A file outside the app-owned project layout — a migrated row pointing at
/// the user's original — is deliberately left untouched: rows are the app's,
/// originals are the user's.
#[tauri::command]
pub fn delete_image(
    state: tauri::State<AppState>,
    image_id: Uuid,
    confirm: bool,
) -> Result<DeleteImageOutcome, CommandError> {
    delete_image_impl(&state, image_id, confirm)
}

/// Testable core of [`delete_image`]: identical behaviour against an owned
/// [`AppState`], without Tauri plumbing.
pub(crate) fn delete_image_impl(
    state: &AppState,
    image_id: Uuid,
    confirm: bool,
) -> Result<DeleteImageOutcome, CommandError> {
    if !confirm {
        return Err(CommandError {
            code: "CONFIRMATION_REQUIRED".to_string(),
            message: "deleting an image also moves its library copy to the trash; \
                 pass confirm=true after asking the user"
                .to_string(),
        });
    }
    let file_path = {
        let conn = state.db.lock().map_err(lock_err)?;
        let image = flight_session::get_image(&conn, image_id)?;
        flight_session::delete_image_row(&conn, image_id)?;
        flight_session::recalculate_session_date_range(&conn, image.session_id)?;
        image.file_path
    };
    // Only files inside the app-owned project layout are removed.
    let owned = Path::new(&file_path).starts_with(state.library_root.join("projects"));
    let files_left =
        owned && import_io::discard_image_file(&state.library_root, Path::new(&file_path)).is_err();
    Ok(DeleteImageOutcome { files_left })
}

/// Moves one image into another session of the same project.
///
/// Both the row and the file move: the file is renamed into the target
/// session's `images/` folder first, then the row is updated to the new path
/// in one transaction. If the row update fails the file is moved back, so
/// the row and the disk cannot disagree. The moved image loses its pair
/// link; a DNG partner left behind is demoted.
///
/// # Errors
///
/// `IMAGE_NOT_FOUND`/`SESSION_NOT_FOUND` for unknown ids, `SAME_SESSION` when
/// the target is the image's own session, `CROSS_PROJECT_IMAGE` for another
/// project (an image only ever belongs to one project), and `STORAGE_ERROR`
/// when the file cannot be moved.
#[tauri::command]
pub fn move_image_to_session(
    state: tauri::State<AppState>,
    image_id: Uuid,
    target_session_id: Uuid,
) -> Result<(), CommandError> {
    move_image_impl(&state, image_id, target_session_id)
}

/// Testable core of [`move_image_to_session`].
pub(crate) fn move_image_impl(
    state: &AppState,
    image_id: Uuid,
    target_session_id: Uuid,
) -> Result<(), CommandError> {
    let (image, source, target) = load_image_and_target(state, image_id, target_session_id)?;
    let dest_dir = import_io::session_images_dir(
        &state.library_root,
        &target.project_id.to_string(),
        &target_session_id.to_string(),
    );
    let new_path = import_io::relocate_image_file(Path::new(&image.file_path), &dest_dir)?;
    let new_path_string = new_path.display().to_string();
    let row = {
        let conn = state.db.lock().map_err(lock_err)?;
        flight_session::move_image_row(&conn, image_id, target_session_id, &new_path_string)
    };
    if let Err(err) = row {
        // Roll the file back so the row and the disk cannot disagree.
        let source_dir = Path::new(&image.file_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let _ = import_io::relocate_image_file(&new_path, &source_dir);
        return Err(err.into());
    }
    let conn = state.db.lock().map_err(lock_err)?;
    flight_session::recalculate_session_date_range(&conn, source.id)?;
    flight_session::recalculate_session_date_range(&conn, target_session_id)?;
    Ok(())
}

/// Copies one image into another session of the same project.
///
/// The file is copied (byte-for-byte, suffix-named so nothing is replaced)
/// into the target session's `images/` folder, then a new row is inserted
/// with a fresh id. The copy is a lone image: no pair link, and only a JPEG
/// starts analysis-ready. This is an explicit copy of content already in the
/// project, so the import duplicate rules do not apply.
///
/// # Errors
///
/// Same as [`move_image_to_session`], with `STORAGE_ERROR` when the file
/// cannot be copied.
#[tauri::command]
pub fn copy_image_to_session(
    state: tauri::State<AppState>,
    image_id: Uuid,
    target_session_id: Uuid,
) -> Result<(), CommandError> {
    copy_image_impl(&state, image_id, target_session_id)
}

/// Testable core of [`copy_image_to_session`].
pub(crate) fn copy_image_impl(
    state: &AppState,
    image_id: Uuid,
    target_session_id: Uuid,
) -> Result<(), CommandError> {
    let (image, _source, target) = load_image_and_target(state, image_id, target_session_id)?;
    let dest_dir = import_io::session_images_dir(
        &state.library_root,
        &target.project_id.to_string(),
        &target_session_id.to_string(),
    );
    let new_path = import_io::copy_image_file(Path::new(&image.file_path), &dest_dir)?;
    let copy = Image {
        id: Uuid::new_v4(),
        session_id: target_session_id,
        file_path: new_path.display().to_string(),
        raw_sibling_id: None,
        analysis_ready: Some(image.format.as_deref() == Some("jpeg")),
        ..image
    };
    let inserted = {
        let conn = state.db.lock().map_err(lock_err)?;
        flight_session::insert_image(&conn, &copy)
    };
    if let Err(err) = inserted {
        let _ = std::fs::remove_file(&new_path);
        return Err(err.into());
    }
    let conn = state.db.lock().map_err(lock_err)?;
    flight_session::recalculate_session_date_range(&conn, target_session_id)?;
    Ok(())
}

/// Reads the image and both sessions, enforcing the rules that need no disk:
/// same project, different session.
fn load_image_and_target(
    state: &AppState,
    image_id: Uuid,
    target_session_id: Uuid,
) -> Result<(Image, Session, Session), CommandError> {
    let conn = state.db.lock().map_err(lock_err)?;
    let image = flight_session::get_image(&conn, image_id)?;
    let source = flight_session::get_session(&conn, image.session_id)?;
    let target = flight_session::get_session(&conn, target_session_id)?;
    if source.project_id != target.project_id {
        return Err(CommandError {
            code: "CROSS_PROJECT_IMAGE".to_string(),
            message: "images can only move or copy between sessions of the same project"
                .to_string(),
        });
    }
    if source.id == target_session_id {
        return Err(CommandError {
            code: "SAME_SESSION".to_string(),
            message: "the image is already in that session".to_string(),
        });
    }
    Ok((image, source, target))
}
