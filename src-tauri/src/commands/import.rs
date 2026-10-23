//! RGB image import commands (Module 1).
//!
//! [`import_images`] runs the copy-on-import pipeline: picked files are copied
//! into the app-owned library, metadata is read from the copy, and one row per
//! stored image is recorded. [`get_analysis_input`] answers what a plugin may
//! analyse for a stored image.
//!
//! The pure planning half lives in
//! [`crate::modules::module_01::image_importer`]; the I/O half in
//! [`crate::modules::module_01::import_io`]. These commands are thin adapters
//! over the latter, adding the Tauri bridge and progress events.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::models::image::{ImportError, ImportReport};
use crate::modules::module_01::error::SessionError;
use crate::modules::module_01::import_io::{analysis_input, run_import, ImportFailure};
use crate::plugin_manager::error::CommandError;
use crate::plugin_manager::payload::ImageMetadata as PluginImageMetadata;
use crate::AppState;

/// Event emitted after every imported file.
///
/// Payload: `{ session_id, done, total, path }`, where `path` is the
/// destination copy and `session_id` lets the UI ignore events from a
/// different import that happens to run at the same time.
pub const IMPORT_PROGRESS_EVENT: &str = "import-progress";

#[derive(Debug, Clone, Serialize)]
struct ImportProgress {
    session_id: String,
    done: usize,
    total: usize,
    path: String,
}

/// Imports picked files (or a picked folder) into an existing session.
///
/// # Arguments
///
/// * `session_id` — destination session UUID.
/// * `sources` — absolute paths of picked files, or of folders when
///   `from_folder` is `true`.
/// * `from_folder` — scan each source folder (non-recursive) for
///   `.jpg`/`.jpeg`/`.dng`.
///
/// While it runs, `import-progress` events report progress. Per-file problems
/// come back in [`ImportReport::rejected`]; only a failure that stops the
/// whole run is an error.
///
/// # Errors
///
/// `SESSION_NOT_FOUND` for a malformed or unknown session, `NO_IMAGES_FOUND`
/// when nothing importable was picked, `DB_ERROR` when SQLite rejects a
/// statement, `DB_LOCK_POISONED` when a previous operation crashed while
/// holding the database lock, and `STORAGE_ERROR` when the library folder
/// cannot be used.
#[tauri::command]
pub async fn import_images(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    sources: Vec<String>,
    from_folder: bool,
) -> Result<ImportReport, CommandError> {
    let session_id = Uuid::parse_str(&session_id).map_err(|_| {
        ImportFailure::Import(ImportError::SessionNotFound {
            session_id: session_id.clone(),
        })
    })?;
    let db = Arc::clone(&state.db);
    let library_root = Arc::clone(&state.library_root);
    let progress_session = session_id.to_string();

    let report = tauri::async_runtime::spawn_blocking(move || {
        run_import(
            &db,
            &library_root,
            session_id,
            &sources,
            from_folder,
            |done, total, path| {
                let _ = app.emit(
                    IMPORT_PROGRESS_EVENT,
                    ImportProgress {
                        session_id: progress_session.clone(),
                        done,
                        total,
                        path: path.to_string(),
                    },
                );
            },
        )
    })
    .await
    .map_err(|err| CommandError {
        code: "IMPORT_JOIN_ERROR".to_string(),
        message: format!("the import task stopped unexpectedly: {err}"),
    })??;

    Ok(report)
}

/// Returns the image a plugin may analyse for `image_id`.
///
/// For a JPEG this is the image itself. For a DNG it is the same-stem JPEG
/// that was imported with it; a DNG with no such JPEG is rejected with
/// `IMAGE_NOT_READY` because Module 1 does not decode raw pixels.
///
/// # Errors
///
/// `IMAGE_NOT_FOUND` when the id does not exist, `IMAGE_NOT_READY` when the
/// image cannot be analysed, `DB_ERROR` when the row cannot be read, and
/// `DB_LOCK_POISONED` when a previous operation crashed while holding the
/// database lock.
#[tauri::command]
pub fn get_analysis_input(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<PluginImageMetadata, CommandError> {
    let image_id = Uuid::parse_str(&image_id).map_err(|_| CommandError {
        code: "IMAGE_NOT_FOUND".to_string(),
        message: format!("image '{image_id}' not found"),
    })?;
    let conn = state
        .db
        .lock()
        .map_err(|err| CommandError::from(SessionError::LockPoisoned(err.to_string())))?;
    let input = analysis_input(&conn, image_id)?;
    Ok(input)
}
