//! End-to-end tests for the whole command surface of Module 1.
//!
//! There is a test-side seam around the Tauri runtime. The `*_impl` functions
//! behind the `#[tauri::command]` wrappers run against a plain [`AppState`] —
//! the same database, library root, and session folder movement the real
//! process has — so these tests can answer "does deleting a session actually
//! leave no dangling copies?" without patching the frontend or launching an
//! app instance.
//!
//! Deliberately not unit-level: each test drives the full command path with a
//! real SQLite file, then verifies data-level truth (rows gone, files gone,
//! copies in `.trash`) rather than looking at one function in isolation.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::modules::module_01::error::SessionError;
use crate::modules::module_01::flight_session;
use crate::modules::module_01::import_io::session_images_dir;
use crate::plugin_manager::executor::ActiveJobTracker;
use crate::AppState;

use super::import::{get_analysis_input_impl, import_images_impl};
use super::session::{delete_session_impl, DeleteOutcome};

/// Full app state over real tempdirs: on-disk SQLite, real library root.
struct TestEnv {
    state: AppState,
    /// Owns the database file and the library root (dropped last).
    #[allow(dead_code)]
    _root: TempDir,
    /// Where synthetic JPEG/DNG sources are written per test.
    source_dir: TempDir,
}

/// Fresh app state with the current schema, over real dirs and a real file.
fn test_env() -> TestEnv {
    test_env_with_db(flight_session::init_schema)
}

/// Each e2e test starts from a real file, not an in-memory connection: the
/// command path opens schema-bearing databases in production too.
fn test_env_with_db<F>(init: F) -> TestEnv
where
    F: FnOnce(&rusqlite::Connection) -> Result<(), SessionError>,
{
    let root = tempfile::tempdir().expect("test root");
    let conn = rusqlite::Connection::open(root.path().join("aerial.db")).expect("scratch db");
    init(&conn).expect("scratch schema");
    let state = AppState {
        plugins: Arc::new(RwLock::new(
            crate::plugin_manager::registry::PluginRegistry::new(),
        )),
        jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
        state_path: None,
        db: Arc::new(Mutex::new(conn)),
        library_root: Arc::new(root.path().join("library")),
    };
    let source_dir = tempfile::tempdir().expect("source dir");
    TestEnv {
        state,
        _root: root,
        source_dir,
    }
}

fn env_session(env: &TestEnv) -> Uuid {
    let conn = env.state.db.lock().expect("db lock");
    flight_session::create_session(
        &conn,
        crate::modules::module_01::create_project(&conn, "Test Project".to_string(), None)
            .expect("project")
            .id,
        Some("e2e".to_string()),
    )
    .expect("session")
    .id
}

/// A synthetic-but-real JPEG the import pipeline can read back.
fn write_jpeg(dir: &std::path::Path, name: &str, seed: u8) -> std::path::PathBuf {
    let path = dir.join(name);
    let image = image::RgbImage::from_fn(8, 8, |x, y| {
        image::Rgb([seed.wrapping_add((x * 3) as u8), (y * 5) as u8, 64])
    });
    let file = std::fs::File::create(&path).expect("create jpeg");
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
    encoder.encode_image(&image).expect("encode synthetic jpeg");
    path
}

/// A synthetic DNG the metadata reader accepts: a bare TIFF IFD.
fn write_dng(dir: &std::path::Path, name: &str, width: u32, height: u32) -> std::path::PathBuf {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00]);
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    for (tag, value) in [(256u16, width), (257u16, height)] {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes()); // LONG
        bytes.extend_from_slice(&1u32.to_le_bytes()); // count
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&0u32.to_le_bytes()); // next IFD
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write synthetic dng");
    path
}

fn sources(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// All files below `dir`, sorted.
fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in match std::fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return files,
            Err(err) => panic!("read {}: {err}", current.display()),
        } {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn e2e_import_folder_copies_records_and_emits_progress() {
    let env = test_env();
    let session_id = env_session(&env);
    write_jpeg(env.source_dir.path(), "DJI_0001.JPG", 1);
    write_jpeg(env.source_dir.path(), "DJI_0002.JPG", 2);
    let sources = sources(&[env.source_dir.path().to_path_buf()]);

    let mut events: Vec<(usize, usize, String)> = Vec::new();
    let report = import_images_impl(
        &env.state,
        session_id,
        &sources,
        true,
        |done, total, path| {
            events.push((done, total, path.to_string()));
        },
    )
    .expect("folder import");

    assert_eq!(report.imported_image_ids.len(), 2);
    assert!(report.duplicates.is_empty());
    assert!(report.rejected.is_empty());
    assert!(report.not_analysis_ready.is_empty());

    // Copy-on-import: the library holds the bytes after the source dies.
    let conn = env.state.db.lock().expect("db lock");
    let mut images = flight_session::get_images_by_session(&conn, session_id).expect("rows");
    drop(conn);
    images.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    assert_eq!(images.len(), 2);
    let mut stored_files: Vec<String> = images.iter().map(|i| i.file_path.clone()).collect();
    stored_files.sort();
    for event in &events {
        let (done, total, path) = event;
        assert_eq!(*total, 2, "total stable across events");
        let _ = (done, total, path);
    }
    let mut event_paths: Vec<String> = events.iter().map(|e| e.2.clone()).collect();
    event_paths.sort();
    assert_eq!(event_paths, stored_files);
    // done climbs to total exactly once per file, on the last event.
    assert_eq!(events[1].0, events[1].1);
}

#[test]
fn e2e_reimport_reports_duplicates_and_copies_nothing() {
    let env = test_env();
    let session_id = env_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0001.JPG", 1);
    let _ = jpeg;
    let sources = sources(&[env.source_dir.path().to_path_buf()]);

    let first = import_images_impl(&env.state, session_id, &sources, true, |_, _, _| {})
        .expect("first import");
    assert_eq!(first.imported_image_ids.len(), 1);

    let second = import_images_impl(&env.state, session_id, &sources, true, |_, _, _| {})
        .expect("second import");
    assert!(second.imported_image_ids.is_empty(), "nothing re-copied");
    assert_eq!(second.duplicates.len(), 1, "reported as duplicate");
    assert!(second.rejected.is_empty());

    // The library still holds exactly one copy for the session.
    let project_id = env_session_project(&env, session_id);
    let session_dir = session_images_dir(
        &env.state.library_root,
        &project_id,
        &session_id.to_string(),
    );
    assert_eq!(
        files_under(&session_dir).len(),
        1,
        "no additional copy was written"
    );
}

fn env_session_project(env: &TestEnv, session_id: Uuid) -> String {
    let conn = env.state.db.lock().expect("db lock");
    flight_session::get_session(&conn, session_id)
        .expect("session")
        .project_id
        .to_string()
}

#[test]
fn e2e_get_analysis_input_uses_jpeg_for_paired_dng() {
    let env = test_env();
    let session_id = env_session(&env);
    let _jpeg = write_jpeg(env.source_dir.path(), "DJI_0003.JPG", 3);
    let _dng = write_dng(env.source_dir.path(), "DJI_0003.DNG", 5, 4);
    let sources = sources(&[env.source_dir.path().to_path_buf()]);
    import_images_impl(&env.state, session_id, &sources, true, |_, _, _| {}).expect("pair import");

    let conn = env.state.db.lock().expect("db lock");
    let images = flight_session::get_images_by_session(&conn, session_id).expect("rows");
    drop(conn);
    let dng_row = images
        .iter()
        .find(|row| row.file_path.ends_with("DJI_0003.DNG"))
        .expect("dng row");
    let jpeg_row = images
        .iter()
        .find(|row| row.file_path.ends_with("DJI_0003.JPG"))
        .expect("jpeg row");
    // The pair link is what makes the DNG analysable at all.
    assert_eq!(
        dng_row.raw_sibling_id.map(|id| id == jpeg_row.id),
        Some(true),
        "dng points at its jpeg pair"
    );

    let metadata =
        get_analysis_input_impl(&env.state, dng_row.id).expect("analysis input for the paired dng");
    assert_eq!(
        std::path::Path::new(&metadata.path),
        std::path::Path::new(&jpeg_row.file_path),
        "the analysis input is the jpeg copy, not the dng"
    );
    assert_eq!(metadata.width, 8);
}

#[test]
fn e2e_delete_session_requires_confirm_then_removes_rows_and_files() {
    let env = test_env();
    let session_id = env_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0001.JPG", 1);
    let _ = jpeg;
    let sources = sources(&[env.source_dir.path().to_path_buf()]);
    import_images_impl(&env.state, session_id, &sources, true, |_, _, _| {}).expect("import");

    // Without `confirm` the command must not move anything.
    let refusal = delete_session_impl(&env.state, session_id, false)
        .expect_err("deletion without confirmation is refused");
    assert_eq!(refusal.code, "CONFIRMATION_REQUIRED");
    {
        let conn = env.state.db.lock().expect("db lock");
        flight_session::get_session(&conn, session_id)
            .expect("session still present before confirmation");
    }
    assert!(
        !files_under(&env.state.library_root).is_empty(),
        "copies untouched before confirmation"
    );

    let outcome = delete_session_impl(&env.state, session_id, true).expect("delete");
    assert!(!outcome.files_left, "nothing left behind");
    {
        let conn = env.state.db.lock().expect("db lock");
        let err = flight_session::get_session(&conn, session_id)
            .expect_err("session deleted from database");
        assert!(matches!(err, SessionError::NotFound { .. }), "{err:?}",);
        assert!(
            flight_session::get_images_by_session(&conn, session_id)
                .expect("query after delete")
                .is_empty(),
            "image rows deleted too"
        );
    }
    assert!(
        files_under(&env.state.library_root).is_empty(),
        "library has no stray copies after deletion"
    );
    assert!(
        files_under(&env.state.library_root.join(".trash")).is_empty(),
        "nothing needed rescuing in this happy path"
    );
}

#[test]
fn e2e_sweep_trashes_orphan_in_known_session_only() {
    let env = test_env();
    let session_id = env_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0001.JPG", 1);
    let _ = jpeg;
    let sources = sources(&[env.source_dir.path().to_path_buf()]);
    import_images_impl(&env.state, session_id, &sources, true, |_, _, _| {}).expect("import");

    // A session the sweep does not know must never be touched.
    let unknown_dir = env
        .state
        .library_root
        .join("projects")
        .join(Uuid::new_v4().to_string())
        .join("sessions")
        .join(Uuid::new_v4().to_string())
        .join("images");
    std::fs::create_dir_all(&unknown_dir).expect("create unknown scope");
    let other_library_file = unknown_dir.join("stray_from_other_library.JPG");
    std::fs::write(&other_library_file, b"not ours").expect("stray");

    // First run: nothing unreferenced within known scopes yet.
    let trashed = crate::run_library_sweep(&env.state.db, &env.state.library_root, Duration::ZERO);
    assert_eq!(trashed, 0, "nothing unreferenced in known scopes");

    // Now stage a real orphan in the known session and re-run.
    let project_id = env_session_project(&env, session_id);
    let known_images = session_images_dir(
        &env.state.library_root,
        &project_id,
        &session_id.to_string(),
    );
    let orphan = known_images.join("orphan.JPG");
    std::fs::write(&orphan, b"orphaned").expect("orphan file");
    let trashed = crate::run_library_sweep(&env.state.db, &env.state.library_root, Duration::ZERO);
    assert_eq!(trashed, 1, "the real orphan moved to .trash");

    let referenced = {
        let conn = env.state.db.lock().expect("db lock");
        flight_session::get_images_by_session(&conn, session_id).expect("rows")[0]
            .file_path
            .clone()
    };
    assert!(
        std::path::Path::new(&referenced).is_file(),
        "the referenced copy survives the sweep"
    );
    assert!(
        other_library_file.is_file(),
        "an unknown session's files are never swept"
    );
    let trashed_files = files_under(&env.state.library_root.join(".trash"));
    assert_eq!(trashed_files.len(), 1);
    assert!(
        trashed_files[0]
            .to_string_lossy()
            .ends_with("images/orphan.JPG"),
        "trash keeps the session-relative layout: {trashed_files:?}"
    );
    assert_eq!(
        std::fs::read(&trashed_files[0]).expect("orphan"),
        b"orphaned"
    );
}

#[test]
fn e2e_get_analysis_input_rejects_unknown_image() {
    let env = test_env();
    let error =
        get_analysis_input_impl(&env.state, Uuid::new_v4()).expect_err("unknown id is rejected");
    assert_eq!(error.code, "IMAGE_NOT_FOUND");
}

#[test]
fn e2e_import_rejects_unknown_session() {
    let env = test_env();
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0001.JPG", 1);
    let error = import_images_impl(
        &env.state,
        Uuid::new_v4(),
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect_err("unknown session is rejected");
    assert_eq!(error.code, "SESSION_NOT_FOUND");
}

#[test]
fn e2e_delete_outcome_serializes_camel_case() {
    let outcome = DeleteOutcome { files_left: true };
    let json = serde_json::to_string(&outcome).expect("serialize");
    assert_eq!(json, r#"{"filesLeft":true}"#);
}
