use super::*;
use crate::plugin_manager::executor::{ActiveJobTracker, JobRecord, JobStatusDto};
use crate::plugin_manager::models::{
    InputsSpec, PluginBundle, PluginManifest, PluginMetadata, PluginRuntime, PluginSource,
    PluginStateRecord,
};
use crate::plugin_manager::registry::PluginRegistry;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

fn test_app_state(state_path: Option<PathBuf>, library_root: PathBuf) -> crate::AppState {
    crate::AppState {
        plugins: std::sync::Arc::new(tokio::sync::RwLock::new(PluginRegistry::new())),
        jobs: std::sync::Arc::new(tokio::sync::RwLock::new(ActiveJobTracker::new())),
        state_path,
        db: std::sync::Arc::new(std::sync::Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        )),
        library_root: std::sync::Arc::new(library_root),
    }
}

fn create_test_app_state(temp_dir: &Path) -> crate::AppState {
    test_app_state(
        Some(temp_dir.join("plugin_state.json")),
        temp_dir.join("library"),
    )
}

fn make_test_bundle(
    id: &str,
    source: &str,
    install_path: PathBuf,
    parameters_raw: serde_json::Value,
) -> PluginBundle {
    PluginBundle {
        install_path,
        manifest: PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: PluginMetadata {
                id: id.to_string(),
                name: format!("Plugin {id}"),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: format!("Test plugin {id}"),
                category: Some("vegetation".to_string()),
                tags: vec![],
                min_core_version: None,
            },
            runtime: PluginRuntime {
                runtime_type: "python".to_string(),
                entrypoint: "main.py".to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: None,
        },
        inputs_spec: InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string()],
            requires_metadata: vec![],
            optional_metadata: vec![],
            supports_aoi: false,
            aoi_format: None,
        },
        inputs_raw: serde_json::json!({}),
        parameters_raw,
        outputs_raw: serde_json::json!({}),
        enabled: true,
        source: source.to_string(),
        health_status: "unknown".to_string(),
        custom_parameters: serde_json::json!({}),
    }
}

fn sample_parameters_schema() -> serde_json::Value {
    serde_json::json!({
        "parameters": [
            {
                "name": "threshold",
                "label": "Threshold",
                "type": "number",
                "default": 0.5,
                "min": 0.0,
                "max": 1.0
            },
            {
                "name": "mode",
                "label": "Mode",
                "type": "select",
                "options": ["FAST", "ACCURATE"],
                "default": "FAST"
            }
        ]
    })
}

fn make_dummy_manifest_json(plugin_id: &str) -> String {
    format!(
        r#"{{
            "manifest_version": 1,
            "metadata": {{
                "id": "{plugin_id}",
                "name": "Test User Plugin",
                "version": "1.0.0",
                "description": "User test plugin",
                "category": "vegetation",
                "tags": ["user", "test"]
            }},
            "runtime": {{
                "type": "python",
                "entrypoint": "main.py"
            }},
            "parameters": "parameters.json",
            "inputs": "inputs.json",
            "outputs": "outputs.json"
        }}"#
    )
}

fn create_valid_zip_archive(zip_path: &Path, plugin_id: &str) {
    let file = std::fs::File::create(zip_path).unwrap();
    let mut writer = zip::ZipWriter::new(file);

    let manifest = make_dummy_manifest_json(plugin_id);
    let parameters = serde_json::to_string_pretty(&sample_parameters_schema()).unwrap();

    let inputs = r#"{
        "granularity": "single_image",
        "supported_mime_types": ["image/jpeg"]
    }"#;

    let outputs = r#"{
        "metrics": {},
        "artifacts": {}
    }"#;

    writer
        .start_file("manifest.json", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(manifest.as_bytes()).unwrap();

    writer
        .start_file("parameters.json", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(parameters.as_bytes()).unwrap();

    writer
        .start_file("inputs.json", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(inputs.as_bytes()).unwrap();

    writer
        .start_file("outputs.json", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(outputs.as_bytes()).unwrap();

    writer
        .start_file("main.py", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"# entrypoint\nprint('hello')\n").unwrap();

    writer.finish().unwrap();
}

async fn insert_running_job(app_state: &crate::AppState, job_id: &str, plugin_id: &str) {
    let mut tracker = app_state.jobs.write().await;
    tracker.jobs.insert(
        job_id.to_string(),
        JobRecord {
            job_id: job_id.to_string(),
            plugin_id: plugin_id.to_string(),
            status: JobStatusDto::Running {
                progress_pct: 50,
                stage: "processing".to_string(),
            },
            result: None,
            started_at: "2026-10-25T12:00:00Z".to_string(),
            abort_handle: None,
            child_pid: None,
            output_dir: None,
        },
    );
}

#[test]
fn test_verify_and_extract_entry_safe() {
    let mut buffer = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
        writer
            .start_file("valid/file.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"safe payload").unwrap();
        writer.finish().unwrap();
    }
    let dir = tempdir().unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
    let mut entry = archive.by_index(0).unwrap();
    let res = verify_and_extract_entry(&mut entry, dir.path());
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), dir.path().join("valid/file.txt"));
}

#[test]
fn test_verify_and_extract_entry_zip_slip() {
    let mut buffer = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
        writer
            .start_file("../evil.sh", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"evil payload").unwrap();
        writer.finish().unwrap();
    }
    let dir = tempdir().unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
    let mut entry = archive.by_index(0).unwrap();
    let res = verify_and_extract_entry(&mut entry, dir.path());
    assert!(res.is_err());
    match res.unwrap_err() {
        PluginError::SecurityViolation(msg) => {
            assert!(msg.contains("Zip-Slip") || msg.contains("escapes"));
        }
        other => panic!("Expected SecurityViolation, got {:?}", other),
    }
}

#[test]
fn test_load_plugin_state_nonexistent_returns_default() {
    let temp_dir = tempdir().unwrap();
    let missing_path = temp_dir.path().join("plugin_state.json");
    let store = load_plugin_state(&missing_path).expect("should return default on missing");
    assert_eq!(store.schema_version, 1);
    assert!(store.plugins.is_empty());
}

#[test]
fn test_save_plugin_state_atomic_and_load_roundtrip() {
    let temp_dir = tempdir().unwrap();
    let state_path = temp_dir.path().join("subdir").join("plugin_state.json");

    let mut store = PluginStateStore::default();
    store.plugins.insert(
        "rgb-exg".to_string(),
        PluginStateRecord {
            version: "1.0.0".to_string(),
            source: PluginSource::Builtin,
            enabled: true,
            installed_at: "2026-09-15T14:32:00Z".to_string(),
            last_healthcheck: None,
            custom_parameters: serde_json::json!({"threshold": 0.42}),
            stats: Default::default(),
        },
    );

    save_plugin_state(&state_path, &store).expect("save_plugin_state should succeed");
    assert!(
        state_path.exists(),
        "State file should exist after atomic save"
    );

    let loaded = load_plugin_state(&state_path).expect("load_plugin_state should succeed");
    assert_eq!(loaded.schema_version, store.schema_version);
    assert!(loaded.plugins.contains_key("rgb-exg"));
    assert!(loaded.plugins.get("rgb-exg").unwrap().enabled);
    assert_eq!(
        loaded.plugins.get("rgb-exg").unwrap().custom_parameters["threshold"],
        0.42
    );
}

#[test]
fn test_load_plugin_state_malformed_json_returns_state_error() {
    let temp_dir = tempdir().unwrap();
    let state_path = temp_dir.path().join("plugin_state.json");
    std::fs::write(&state_path, "{ broken json").unwrap();

    let err = load_plugin_state(&state_path).unwrap_err();
    assert!(matches!(err, PluginError::StateError(_)));
}

#[tokio::test]
async fn test_set_plugin_status_updates_registry_and_flushes_state_file() {
    let temp_dir = tempdir().unwrap();
    let state_path = temp_dir.path().join("plugin_state.json");
    let app_state = create_test_app_state(temp_dir.path());

    {
        let mut registry = app_state.plugins.write().await;
        registry.register(make_test_bundle(
            "test-plugin",
            "builtin",
            temp_dir.path().to_path_buf(),
            serde_json::json!({}),
        ));
    }

    let res = set_plugin_status_internal("test-plugin", false, &app_state).await;
    assert!(res.is_ok());

    // In-memory check
    let reg = app_state.plugins.read().await;
    assert!(!reg.get_details("test-plugin").unwrap().enabled);
    drop(reg);

    // On-disk check
    assert!(
        state_path.exists(),
        "State file must exist after set_plugin_status"
    );
    let store = load_plugin_state(&state_path).unwrap();
    assert!(!store.plugins.get("test-plugin").unwrap().enabled);

    // Re-enable
    let res2 = set_plugin_status_internal("test-plugin", true, &app_state).await;
    assert!(res2.is_ok());

    let reg2 = app_state.plugins.read().await;
    assert!(reg2.get_details("test-plugin").unwrap().enabled);
    drop(reg2);

    let store2 = load_plugin_state(&state_path).unwrap();
    assert!(store2.plugins.get("test-plugin").unwrap().enabled);
}

#[tokio::test]
async fn test_set_plugin_status_missing_plugin_returns_not_found() {
    let app_state = test_app_state(None, PathBuf::from("/tmp"));

    let res = set_plugin_status_internal("missing-plugin", false, &app_state).await;
    assert!(matches!(res, Err(PluginError::NotFound(_))));
}

#[tokio::test]
async fn test_install_plugin_success() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let zip_path = temp_dir.path().join("user-plugin.zip");
    create_valid_zip_archive(&zip_path, "user-test-plugin");

    let summary = install_plugin_internal(&zip_path, &app_state)
        .await
        .expect("installation should succeed");
    assert_eq!(summary.id, "user-test-plugin");
    assert_eq!(summary.source, "user");
    assert!(summary.enabled);

    // Registry verification
    let registry = app_state.plugins.read().await;
    let details = registry.get_details("user-test-plugin").unwrap();
    assert!(details.enabled);
    drop(registry);

    // Filesystem verification
    let target_dir = temp_dir.path().join("user-test-plugin");
    assert!(target_dir.join("manifest.json").exists());
    assert!(target_dir.join("main.py").exists());

    // State persistence verification
    let state_path = temp_dir.path().join("plugin_state.json");
    let store = load_plugin_state(&state_path).unwrap();
    let rec = store.plugins.get("user-test-plugin").unwrap();
    assert_eq!(rec.source, PluginSource::User);
    assert!(rec.enabled);
}

#[tokio::test]
async fn test_install_plugin_zip_slip_rejected() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let zip_path = temp_dir.path().join("malicious.zip");

    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../evil.sh", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"malicious").unwrap();
        writer.finish().unwrap();
    }

    let res = install_plugin_internal(&zip_path, &app_state).await;
    assert!(res.is_err());
    match res.unwrap_err() {
        PluginError::SecurityViolation(msg) => {
            assert!(msg.contains("Zip-Slip") || msg.contains("escapes"));
        }
        other => panic!("Expected SecurityViolation, got {:?}", other),
    }

    // Verify zero disk remnants
    assert!(!temp_dir.path().join("evil.sh").exists());
}

#[tokio::test]
async fn test_install_plugin_cannot_overwrite_builtin() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());

    // Pre-register builtin plugin
    {
        let mut reg = app_state.plugins.write().await;
        reg.register(make_test_bundle(
            "builtin-plugin",
            "builtin",
            temp_dir.path().join("builtin-plugin"),
            serde_json::json!({}),
        ));
    }

    let zip_path = temp_dir.path().join("overwrite.zip");
    create_valid_zip_archive(&zip_path, "builtin-plugin");

    let res = install_plugin_internal(&zip_path, &app_state).await;
    assert!(matches!(res, Err(PluginError::SecurityViolation(_))));
}

#[tokio::test]
async fn test_install_plugin_corrupt_archive_no_remnants() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let bad_zip = temp_dir.path().join("corrupt.zip");
    std::fs::write(&bad_zip, b"this is not a zip file").unwrap();

    let res = install_plugin_internal(&bad_zip, &app_state).await;
    assert!(matches!(res, Err(PluginError::ArchiveError(_))));
}

#[tokio::test]
async fn test_remove_user_plugin_success() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let zip_path = temp_dir.path().join("plugin.zip");
    create_valid_zip_archive(&zip_path, "removable-plugin");

    // Install first
    let _ = install_plugin_internal(&zip_path, &app_state)
        .await
        .unwrap();
    let target_dir = temp_dir.path().join("removable-plugin");
    assert!(target_dir.exists());

    // Remove
    let remove_res = remove_plugin_internal("removable-plugin", &app_state).await;
    assert!(remove_res.is_ok());

    // Assert directory deleted
    assert!(!target_dir.exists());

    // Assert removed from registry
    let reg = app_state.plugins.read().await;
    assert!(!reg.plugins.contains_key("removable-plugin"));
    drop(reg);

    // Assert purged from state store
    let state_path = temp_dir.path().join("plugin_state.json");
    let store = load_plugin_state(&state_path).unwrap();
    assert!(!store.plugins.contains_key("removable-plugin"));
}

#[tokio::test]
async fn test_remove_builtin_plugin_rejected() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());

    // Register builtin
    {
        let mut reg = app_state.plugins.write().await;
        reg.register(make_test_bundle(
            "core-builtin",
            "builtin",
            temp_dir.path().join("core-builtin"),
            serde_json::json!({}),
        ));
    }

    let res = remove_plugin_internal("core-builtin", &app_state).await;
    assert!(matches!(res, Err(PluginError::SecurityViolation(_))));

    // Verify still in registry
    let reg = app_state.plugins.read().await;
    assert!(reg.plugins.contains_key("core-builtin"));
}

#[tokio::test]
async fn test_remove_plugin_running_job_rejected() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());

    // Register user plugin
    {
        let mut reg = app_state.plugins.write().await;
        reg.register(make_test_bundle(
            "busy-user-plugin",
            "user",
            temp_dir.path().join("busy-user-plugin"),
            serde_json::json!({}),
        ));
    }

    insert_running_job(&app_state, "job-123", "busy-user-plugin").await;

    let res = remove_plugin_internal("busy-user-plugin", &app_state).await;
    assert!(matches!(res, Err(PluginError::ExecutionError(_))));
}

#[tokio::test]
async fn test_remove_nonexistent_plugin_returns_not_found() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let res = remove_plugin_internal("ghost-plugin", &app_state).await;
    assert!(matches!(res, Err(PluginError::NotFound(_))));
}

#[tokio::test]
async fn test_update_plugin_config_success() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());

    // Register plugin with parameters
    {
        let mut reg = app_state.plugins.write().await;
        reg.register(make_test_bundle(
            "config-plugin",
            "user",
            temp_dir.path().join("config-plugin"),
            sample_parameters_schema(),
        ));
    }

    let custom_params = serde_json::json!({
        "threshold": 0.85,
        "mode": "ACCURATE"
    });

    let res =
        update_plugin_config_internal("config-plugin", custom_params.clone(), &app_state).await;
    assert!(res.is_ok());

    // Check state file on disk
    let state_path = temp_dir.path().join("plugin_state.json");
    let store = load_plugin_state(&state_path).unwrap();
    let rec = store.plugins.get("config-plugin").unwrap();
    assert_eq!(rec.custom_parameters["threshold"], 0.85);
    assert_eq!(rec.custom_parameters["mode"], "ACCURATE");

    // Check in-memory bundle updated as well
    let reg = app_state.plugins.read().await;
    let in_memory = reg.plugins.get("config-plugin").unwrap();
    assert_eq!(in_memory.custom_parameters["threshold"], 0.85);
    assert_eq!(in_memory.custom_parameters["mode"], "ACCURATE");

    // Also check get_details reflects custom_parameters
    let details = reg.get_details("config-plugin").unwrap();
    assert_eq!(details.custom_parameters["threshold"], 0.85);
    assert_eq!(details.custom_parameters["mode"], "ACCURATE");
}

#[tokio::test]
async fn test_update_plugin_config_missing_plugin_returns_not_found() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let res = update_plugin_config_internal(
        "missing-plugin",
        serde_json::json!({"foo": "bar"}),
        &app_state,
    )
    .await;
    assert!(matches!(res, Err(PluginError::NotFound(_))));
}

#[tokio::test]
async fn test_update_plugin_config_invalid_parameter_rejected() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());

    // Register plugin
    {
        let mut reg = app_state.plugins.write().await;
        reg.register(make_test_bundle(
            "strict-plugin",
            "user",
            temp_dir.path().join("strict-plugin"),
            sample_parameters_schema(),
        ));
    }

    // 1. Unknown param
    let res1 = update_plugin_config_internal(
        "strict-plugin",
        serde_json::json!({"unknown_param": 10}),
        &app_state,
    )
    .await;
    assert!(matches!(res1, Err(PluginError::SubcontractInvalid { .. })));

    // 2. Number exceeds max
    let res2 = update_plugin_config_internal(
        "strict-plugin",
        serde_json::json!({"threshold": 1.5}),
        &app_state,
    )
    .await;
    assert!(matches!(res2, Err(PluginError::SubcontractInvalid { .. })));

    // 3. Number below min
    let res3 = update_plugin_config_internal(
        "strict-plugin",
        serde_json::json!({"threshold": -0.1}),
        &app_state,
    )
    .await;
    assert!(matches!(res3, Err(PluginError::SubcontractInvalid { .. })));

    // 4. Invalid select option
    let res4 = update_plugin_config_internal(
        "strict-plugin",
        serde_json::json!({"mode": "ULTRA_FAST"}),
        &app_state,
    )
    .await;
    assert!(matches!(res4, Err(PluginError::SubcontractInvalid { .. })));

    // 5. Wrong type (string for number)
    let res5 = update_plugin_config_internal(
        "strict-plugin",
        serde_json::json!({"threshold": "not_a_number"}),
        &app_state,
    )
    .await;
    assert!(matches!(res5, Err(PluginError::SubcontractInvalid { .. })));
}

#[tokio::test]
async fn test_install_plugin_cannot_overwrite_running_job() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let zip_path = temp_dir.path().join("busy.zip");
    create_valid_zip_archive(&zip_path, "busy-user-plugin");

    // Install first
    install_plugin_internal(&zip_path, &app_state)
        .await
        .unwrap();

    // Mark running job
    insert_running_job(&app_state, "job-123", "busy-user-plugin").await;

    // Attempt reinstall while running
    let res = install_plugin_internal(&zip_path, &app_state).await;
    assert!(
        matches!(res, Err(PluginError::ExecutionError(_))),
        "Expected ExecutionError when overwriting active running job, got {:?}",
        res
    );
}

#[test]
fn test_verify_and_extract_entry_rejects_symlinks() {
    let mut buffer = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
        writer
            .add_symlink(
                "malicious_symlink",
                "/etc/passwd",
                SimpleFileOptions::default(),
            )
            .unwrap();
        writer.finish().unwrap();
    }
    let dir = tempdir().unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
    let mut entry = archive.by_index(0).unwrap();
    let res = verify_and_extract_entry(&mut entry, dir.path());
    assert!(res.is_err(), "Symlinks in zip must be rejected");
    assert!(matches!(
        res.unwrap_err(),
        PluginError::SecurityViolation(_)
    ));
}

#[test]
#[cfg(unix)]
fn test_verify_and_extract_entry_preserves_executable_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let mut buffer = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut buffer));
        let options = SimpleFileOptions::default().unix_permissions(0o755);
        writer.start_file("bin/run.sh", options).unwrap();
        writer.write_all(b"#!/bin/sh\necho ok\n").unwrap();
        writer.finish().unwrap();
    }
    let dir = tempdir().unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(buffer)).unwrap();
    let mut entry = archive.by_index(0).unwrap();
    let extracted = verify_and_extract_entry(&mut entry, dir.path()).unwrap();
    let meta = std::fs::metadata(&extracted).unwrap();
    assert_eq!(
        meta.permissions().mode() & 0o111,
        0o111,
        "Executable bits must be preserved"
    );
}

#[tokio::test]
async fn test_remove_plugin_invalid_id_path_traversal_rejected() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let res = remove_plugin_internal("../../etc/passwd", &app_state).await;
    assert!(matches!(res, Err(PluginError::NotFound(_))));
}

#[tokio::test]
async fn test_install_plugin_ambiguous_multiple_manifests_rejected() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());
    let zip_path = temp_dir.path().join("ambiguous.zip");

    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);

        writer
            .start_file("plugin-a/manifest.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"{\"manifest_version\": 1}").unwrap();

        writer
            .start_file("plugin-b/manifest.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"{\"manifest_version\": 1}").unwrap();

        writer.finish().unwrap();
    }

    let res = install_plugin_internal(&zip_path, &app_state).await;
    assert!(matches!(res, Err(PluginError::ManifestInvalid { .. })));
}

#[tokio::test]
async fn test_install_plugin_state_save_failure_rolls_back_filesystem_and_registry() {
    let temp_dir = tempdir().unwrap();
    let app_state = create_test_app_state(temp_dir.path());

    // Make state_path a directory so rename(&temp_file_path, state_path) fails with EISDIR
    let state_path = temp_dir.path().join("plugin_state.json");
    std::fs::create_dir_all(&state_path).unwrap();

    let zip_path = temp_dir.path().join("rollback.zip");
    create_valid_zip_archive(&zip_path, "rollback-plugin");

    let res = install_plugin_internal(&zip_path, &app_state).await;
    assert!(
        res.is_err(),
        "Installation must fail when state saving fails"
    );

    // Verify rollback: target directory must NOT exist on disk
    let target_dir = temp_dir.path().join("rollback-plugin");
    assert!(
        !target_dir.exists(),
        "Target directory must be deleted/rolled back on state save failure"
    );

    // Verify rollback: in-memory registry must NOT contain the plugin
    let reg = app_state.plugins.read().await;
    assert!(
        !reg.plugins.contains_key("rollback-plugin"),
        "Registry must not retain plugin on state save failure"
    );
}
