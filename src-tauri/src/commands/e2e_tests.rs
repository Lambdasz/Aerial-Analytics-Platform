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
use super::session::{
    copy_image_impl, delete_image_impl, delete_session_impl, get_image_metadata_impl,
    move_image_impl, DeleteOutcome,
};

/// Resolves a path when the OS will, so assertions survive a library root that
/// is only a symlink (macOS temp folders).
fn resolved(path: &std::path::Path) -> std::path::PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

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
        jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
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
        !files_under(&env.state.library_root.join("projects")).is_empty(),
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
    // Copies live under `projects/`; `.thumbs` thumbnails are derived cache,
    // not copies, and are deliberately not part of this assertion.
    assert!(
        files_under(&env.state.library_root.join("projects")).is_empty(),
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

/// A second session in the same project as `session_id`.
fn env_sibling_session(env: &TestEnv, session_id: Uuid) -> Uuid {
    let conn = env.state.db.lock().expect("db lock");
    let project_id = flight_session::get_session(&conn, session_id)
        .expect("session")
        .project_id;
    flight_session::create_session(&conn, project_id, Some("tujuan".to_string()))
        .expect("sibling session")
        .id
}

/// A session in a brand-new project.
fn env_other_project_session(env: &TestEnv) -> Uuid {
    let conn = env.state.db.lock().expect("db lock");
    let project =
        crate::modules::module_01::create_project(&conn, "Other Project".to_string(), None)
            .expect("project");
    flight_session::create_session(&conn, project.id, Some("lain".to_string()))
        .expect("session")
        .id
}

/// The only image row of a session.
fn only_image(env: &TestEnv, session_id: Uuid) -> crate::models::Image {
    let conn = env.state.db.lock().expect("db lock");
    let mut rows = flight_session::get_images_by_session(&conn, session_id).expect("rows");
    assert_eq!(rows.len(), 1, "one image expected");
    rows.remove(0)
}

#[test]
fn e2e_delete_image_requires_confirm_then_removes_row_and_copy() {
    let env = test_env();
    let session_id = env_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0010.JPG", 10);
    import_images_impl(
        &env.state,
        session_id,
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect("import");
    let image = only_image(&env, session_id);
    let project = env_session_project(&env, session_id);
    let session_dir =
        session_images_dir(&env.state.library_root, &project, &session_id.to_string());

    let refusal = delete_image_impl(&env.state, image.id, false).expect_err("confirm is required");
    assert_eq!(refusal.code, "CONFIRMATION_REQUIRED");
    {
        let conn = env.state.db.lock().expect("db lock");
        flight_session::get_image(&conn, image.id).expect("row untouched before confirmation");
    }
    assert_eq!(files_under(&session_dir).len(), 1, "copy untouched too");

    let outcome = delete_image_impl(&env.state, image.id, true).expect("delete");
    assert!(!outcome.files_left, "nothing left behind");
    {
        let conn = env.state.db.lock().expect("db lock");
        assert!(matches!(
            flight_session::get_image(&conn, image.id),
            Err(SessionError::ImageNotFound { .. })
        ));
        assert!(flight_session::get_images_by_session(&conn, session_id)
            .expect("rows")
            .is_empty());
    }
    assert!(
        files_under(&session_dir).is_empty(),
        "copy removed from disk"
    );
}

#[test]
fn e2e_delete_image_demotes_the_dng_left_behind() {
    let env = test_env();
    let session_id = env_session(&env);
    write_jpeg(env.source_dir.path(), "DJI_0011.JPG", 11);
    write_dng(env.source_dir.path(), "DJI_0011.DNG", 8, 6);
    let sources = sources(&[env.source_dir.path().to_path_buf()]);
    import_images_impl(&env.state, session_id, &sources, true, |_, _, _| {}).expect("pair import");

    let (jpeg, dng_id) = {
        let conn = env.state.db.lock().expect("db lock");
        let rows = flight_session::get_images_by_session(&conn, session_id).expect("rows");
        let jpeg = rows
            .iter()
            .find(|row| row.file_path.ends_with(".JPG"))
            .expect("jpeg row")
            .clone();
        let dng_id = rows
            .iter()
            .find(|row| row.file_path.ends_with(".DNG"))
            .expect("dng row")
            .id;
        (jpeg, dng_id)
    };

    delete_image_impl(&env.state, jpeg.id, true).expect("delete jpeg");

    let conn = env.state.db.lock().expect("db lock");
    let dng = flight_session::get_image(&conn, dng_id).expect("dng survives");
    assert_eq!(dng.raw_sibling_id, None, "the pair link is cleared");
    assert_eq!(
        dng.analysis_ready,
        Some(false),
        "a lone dng is not analysable"
    );
}

#[test]
fn e2e_move_image_between_sessions_relocates_row_and_file() {
    let env = test_env();
    let source_id = env_session(&env);
    let target_id = env_sibling_session(&env, source_id);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0012.JPG", 12);
    import_images_impl(
        &env.state,
        source_id,
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect("import");
    let image = only_image(&env, source_id);
    let project = env_session_project(&env, source_id);
    let source_dir = session_images_dir(&env.state.library_root, &project, &source_id.to_string());
    let target_dir = session_images_dir(&env.state.library_root, &project, &target_id.to_string());

    move_image_impl(&env.state, image.id, target_id).expect("move");

    {
        let conn = env.state.db.lock().expect("db lock");
        let moved = flight_session::get_image(&conn, image.id).expect("row survives the move");
        assert_eq!(moved.session_id, target_id);
        assert!(
            resolved(std::path::Path::new(&moved.file_path)).starts_with(resolved(&target_dir)),
            "row points at the target folder: {}",
            moved.file_path
        );
        assert!(
            flight_session::get_images_by_session(&conn, source_id)
                .expect("rows")
                .is_empty(),
            "the image left the source session"
        );
    }
    assert!(files_under(&source_dir).is_empty(), "no file left behind");
    assert_eq!(files_under(&target_dir).len(), 1, "the file moved");
}

#[test]
fn e2e_copy_image_between_sessions_duplicates_row_and_file() {
    let env = test_env();
    let source_id = env_session(&env);
    let target_id = env_sibling_session(&env, source_id);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0013.JPG", 13);
    import_images_impl(
        &env.state,
        source_id,
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect("import");
    let image = only_image(&env, source_id);
    let project = env_session_project(&env, source_id);
    let source_dir = session_images_dir(&env.state.library_root, &project, &source_id.to_string());
    let target_dir = session_images_dir(&env.state.library_root, &project, &target_id.to_string());

    copy_image_impl(&env.state, image.id, target_id).expect("copy");

    {
        let conn = env.state.db.lock().expect("db lock");
        let original = flight_session::get_image(&conn, image.id).expect("original survives");
        assert_eq!(original.session_id, source_id);
        let copied = flight_session::get_images_by_session(&conn, target_id).expect("rows");
        assert_eq!(copied.len(), 1, "one copied row in the target session");
        let copy = &copied[0];
        assert_ne!(copy.id, original.id, "the copy is its own image");
        assert_eq!(copy.raw_sibling_id, None, "a copy starts unpaired");
        assert_eq!(copy.analysis_ready, Some(true), "a lone jpeg is analysable");
        let expected = original
            .file_path
            .replace(&source_id.to_string(), &target_id.to_string());
        assert_eq!(
            resolved(std::path::Path::new(&copy.file_path)),
            resolved(std::path::Path::new(&expected)),
            "the copy lands beside the original, symlink prefix or not"
        );
    }
    assert_eq!(files_under(&source_dir).len(), 1, "the original copy stays");
    assert_eq!(
        files_under(&target_dir).len(),
        1,
        "the duplicated file landed"
    );
}

#[test]
fn e2e_image_actions_refuse_other_projects() {
    let env = test_env();
    let source_id = env_session(&env);
    let other_id = env_other_project_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0014.JPG", 14);
    import_images_impl(
        &env.state,
        source_id,
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect("import");
    let image = only_image(&env, source_id);

    let moved = move_image_impl(&env.state, image.id, other_id).expect_err("cross-project move");
    assert_eq!(moved.code, "CROSS_PROJECT_IMAGE");
    let copied = copy_image_impl(&env.state, image.id, other_id).expect_err("cross-project copy");
    assert_eq!(copied.code, "CROSS_PROJECT_IMAGE");
    let same = move_image_impl(&env.state, image.id, source_id).expect_err("same session");
    assert_eq!(same.code, "SAME_SESSION");
}

/// The macOS shape: `library_root` reaches the files through a symlink while
/// the import stores the resolved path, so a text `starts_with` check can
/// miss the app's own copy and leave it on disk after a delete.
#[cfg(unix)]
#[test]
fn e2e_delete_image_works_when_the_library_root_is_a_symlink() {
    let root = tempfile::tempdir().expect("test root");
    let conn = rusqlite::Connection::open(root.path().join("aerial.db")).expect("scratch db");
    flight_session::init_schema(&conn).expect("scratch schema");
    let real_library = root.path().join("library-real");
    std::fs::create_dir_all(&real_library).expect("real library");
    let link_library = root.path().join("library");
    std::os::unix::fs::symlink(&real_library, &link_library).expect("symlink library");
    let env = TestEnv {
        state: AppState {
            jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
            db: Arc::new(Mutex::new(conn)),
            library_root: Arc::new(link_library),
        },
        _root: root,
        source_dir: tempfile::tempdir().expect("source dir"),
    };

    let session_id = env_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0015.JPG", 15);
    import_images_impl(
        &env.state,
        session_id,
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect("import");
    let image = only_image(&env, session_id);
    assert!(
        image
            .file_path
            .starts_with(&resolved(&real_library).display().to_string()),
        "the import stored the resolved path"
    );

    let outcome = delete_image_impl(&env.state, image.id, true).expect("delete");
    assert!(
        !outcome.files_left,
        "the copy must be recognized and removed"
    );
    let project = env_session_project(&env, session_id);
    let session_dir =
        session_images_dir(&env.state.library_root, &project, &session_id.to_string());
    assert!(
        files_under(&session_dir).is_empty(),
        "copy removed from disk through the symlink"
    );
}

#[test]
fn e2e_move_refuses_a_row_pointing_at_the_users_original() {
    let env = test_env();
    let source_id = env_session(&env);
    let target_id = env_sibling_session(&env, source_id);
    let original = write_jpeg(env.source_dir.path(), "DJI_0016.JPG", 16);

    let image_id = Uuid::new_v4();
    {
        let conn = env.state.db.lock().expect("db lock");
        let metadata = crate::modules::module_01::metadata_extractor::extract_metadata(&original)
            .expect("metadata");
        let image = flight_session::image_from_metadata(
            image_id,
            source_id,
            original.display().to_string(),
            &metadata,
        );
        flight_session::insert_image(&conn, &image).expect("row");
    }

    let refusal = move_image_impl(&env.state, image_id, target_id).expect_err("refused");
    assert_eq!(refusal.code, "NOT_A_LIBRARY_COPY");
    assert!(original.exists(), "the user's file stays where it is");
    let conn = env.state.db.lock().expect("db lock");
    let row = flight_session::get_image(&conn, image_id).expect("row stays");
    assert_eq!(row.session_id, source_id, "the row did not move");
}

#[test]
fn e2e_get_image_metadata_reads_the_file_not_just_the_row() {
    let env = test_env();
    let session_id = env_session(&env);
    let jpeg = write_jpeg(env.source_dir.path(), "DJI_0017.JPG", 17);
    import_images_impl(
        &env.state,
        session_id,
        &sources(&[jpeg]),
        false,
        |_, _, _| {},
    )
    .expect("import");
    let image = only_image(&env, session_id);

    let metadata = get_image_metadata_impl(&env.state, image.id).expect("metadata");
    assert_eq!(metadata.width, Some(8), "the extractor read the file");
    assert_eq!(metadata.height, Some(8));

    let json = serde_json::to_value(&metadata).expect("json");
    assert!(json.get("gpsLatitude").is_some(), "camelCase for the panel");
    assert!(json.get("gps_latitude").is_none());
}

#[test]
fn e2e_copy_refuses_a_row_pointing_at_the_users_original() {
    let env = test_env();
    let source_id = env_session(&env);
    let target_id = env_sibling_session(&env, source_id);
    let original = write_jpeg(env.source_dir.path(), "DJI_0018.JPG", 18);

    let image_id = Uuid::new_v4();
    {
        let conn = env.state.db.lock().expect("db lock");
        let metadata = crate::modules::module_01::metadata_extractor::extract_metadata(&original)
            .expect("metadata");
        let image = flight_session::image_from_metadata(
            image_id,
            source_id,
            original.display().to_string(),
            &metadata,
        );
        flight_session::insert_image(&conn, &image).expect("row");
    }

    let refusal = copy_image_impl(&env.state, image_id, target_id).expect_err("refused");
    assert_eq!(refusal.code, "NOT_A_LIBRARY_COPY");
}

#[test]
fn e2e_delete_image_reclassifies_session_blur() {
    let env = test_env();
    let session_id = env_session(&env);
    let mut image_ids = Vec::new();
    let real_library = env.state.library_root.join("projects");
    std::fs::create_dir_all(&real_library).expect("projects dir");

    {
        let conn = env.state.db.lock().expect("db lock");
        // 1 mild outlier (score 50.0, above hard floor 20.0)
        let outlier_id = Uuid::new_v4();
        let outlier_path = real_library.join("outlier.jpg");
        std::fs::write(&outlier_path, b"test").expect("write file");
        let mut outlier = flight_session::image_from_metadata(
            outlier_id,
            session_id,
            outlier_path.display().to_string(),
            &crate::models::ImageMetadata::default(),
        );
        outlier.blur_score = Some(50.0);
        flight_session::insert_image(&conn, &outlier).expect("row");
        image_ids.push(outlier_id);

        // 5 sharp images with natural variance so stddev > 0
        let sharp_scores = [100.0, 120.0, 80.0, 110.0, 90.0];
        for (i, &score) in sharp_scores.iter().enumerate() {
            let id = Uuid::new_v4();
            let sharp_path = real_library.join(format!("sharp_{i}.jpg"));
            std::fs::write(&sharp_path, b"test").expect("write file");
            let mut sharp = flight_session::image_from_metadata(
                id,
                session_id,
                sharp_path.display().to_string(),
                &crate::models::ImageMetadata::default(),
            );
            sharp.blur_score = Some(score);
            flight_session::insert_image(&conn, &sharp).expect("row");
            image_ids.push(id);
        }
        flight_session::reclassify_session_blur(
            &conn,
            session_id,
            crate::modules::module_01::blur_detection::classify_with_stats,
        )
        .expect("initial blur classification");

        let row = flight_session::get_image(&conn, outlier_id).expect("row");
        assert_eq!(
            row.is_blurry,
            Some(true),
            "outlier is blurry against 5 sharp images"
        );
    }

    // Delete the 5 sharp images
    for &id in &image_ids[1..] {
        delete_image_impl(&env.state, id, true).expect("delete");
    }

    // The outlier is now the only image left; without the 5 sharp images, it is not an outlier
    let conn = env.state.db.lock().expect("db lock");
    let row = flight_session::get_image(&conn, image_ids[0]).expect("row");
    assert_eq!(
        row.is_blurry,
        Some(false),
        "after deleting sharp images, it is no longer an outlier"
    );
}
