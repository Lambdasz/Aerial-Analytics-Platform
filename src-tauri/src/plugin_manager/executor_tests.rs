use super::*;
use crate::plugin_manager::models::{
    InputsSpec, PluginBundle, PluginExecutionConfig, PluginManifest, PluginMetadata, PluginRuntime,
};
use crate::plugin_manager::registry::PluginRegistry;
use std::collections::HashMap;

fn test_app_state(registry: PluginRegistry, db_conn: rusqlite::Connection) -> crate::AppState {
    crate::AppState {
        plugins: std::sync::Arc::new(tokio::sync::RwLock::new(registry)),
        jobs: std::sync::Arc::new(tokio::sync::RwLock::new(ActiveJobTracker::new())),
        state_path: None,
        db: std::sync::Arc::new(std::sync::Mutex::new(db_conn)),
        library_root: std::sync::Arc::new(std::env::temp_dir()),
    }
}

fn mock_plugin_bundle(
    id: &str,
    install_path: PathBuf,
    runtime_type: &str,
    entrypoint: &str,
    enabled: bool,
    timeout_seconds: u64,
) -> PluginBundle {
    PluginBundle {
        install_path,
        manifest: PluginManifest {
            schema: None,
            manifest_version: 1,
            metadata: PluginMetadata {
                id: id.to_string(),
                name: "Test Executor Plugin".to_string(),
                version: "1.0.0".to_string(),
                author: None,
                license: None,
                description: "Test plugin for executor".to_string(),
                category: None,
                tags: vec![],
                min_core_version: None,
            },
            runtime: PluginRuntime {
                runtime_type: runtime_type.to_string(),
                entrypoint: entrypoint.to_string(),
                min_python_version: None,
                dependencies_file: None,
                healthcheck_flag: None,
            },
            parameters: "parameters.json".to_string(),
            inputs: "inputs.json".to_string(),
            outputs: "outputs.json".to_string(),
            execution: Some(PluginExecutionConfig {
                timeout_seconds,
                provides_output_dir_arg: true,
                concurrency: "sequential".to_string(),
                gpu: None,
            }),
        },
        parameters_raw: serde_json::json!({}),
        inputs_spec: InputsSpec {
            granularity: "single_image".to_string(),
            supported_mime_types: vec!["image/jpeg".to_string()],
            requires_metadata: vec![],
            optional_metadata: vec![],
            supports_aoi: false,
            aoi_format: None,
        },
        inputs_raw: serde_json::json!({}),
        outputs_raw: serde_json::json!({}),
        enabled,
        source: "builtin".to_string(),
        health_status: "healthy".to_string(),
        custom_parameters: serde_json::json!({}),
    }
}

#[test]
fn test_generate_job_id_format() {
    let id = generate_job_id();
    assert!(id.starts_with("exec_"), "job_id must start with 'exec_'");
    let parts: Vec<&str> = id.splitn(3, '_').collect();
    assert_eq!(parts.len(), 3, "job_id must have 3 segments");
    assert_eq!(parts[1].len(), 8, "date segment must be 8 digits");
}

#[test]
fn test_read_and_validate_result_valid() {
    let temp_dir = tempfile::tempdir().unwrap();
    let out_dir = temp_dir.path();
    let artifact_file = out_dir.join("mask.png");
    std::fs::write(&artifact_file, b"fake png data").unwrap();

    let result_json = serde_json::json!({
        "execution_id": "exec_test",
        "status": "success",
        "execution_time_ms": 150,
        "metrics": {
            "coverage_pct": 72.5
        },
        "artifacts": {
            "mask": {
                "file_path": "mask.png",
                "format": "image/png",
                "bounds": [116.85, -1.24, 116.86, -1.23]
            }
        },
        "error": null
    });

    let result_path = out_dir.join("result.json");
    std::fs::write(
        &result_path,
        serde_json::to_string_pretty(&result_json).unwrap(),
    )
    .unwrap();

    let parsed = read_and_validate_result(&result_path, out_dir).unwrap();
    assert_eq!(parsed.job_id, "exec_test");
    assert_eq!(parsed.status, "success");
    assert_eq!(parsed.execution_time_ms, 150);
    assert!(parsed.artifacts.contains_key("mask"));
}

#[test]
fn test_read_and_validate_result_missing_artifact() {
    let temp_dir = tempfile::tempdir().unwrap();
    let out_dir = temp_dir.path();

    let result_json = serde_json::json!({
        "execution_id": "exec_test",
        "status": "success",
        "execution_time_ms": 150,
        "metrics": {},
        "artifacts": {
            "mask": {
                "file_path": "non_existent.png",
                "format": "image/png"
            }
        }
    });

    let result_path = out_dir.join("result.json");
    std::fs::write(
        &result_path,
        serde_json::to_string_pretty(&result_json).unwrap(),
    )
    .unwrap();

    let err = read_and_validate_result(&result_path, out_dir);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().code(), "ARTIFACT_MISSING");
}

#[test]
fn test_read_and_validate_result_sandbox_escape() {
    let temp_dir = tempfile::tempdir().unwrap();
    let out_dir = temp_dir.path();

    // Create an artifact file outside the sandbox
    let outside_dir = tempfile::tempdir().unwrap();
    let outside_file = outside_dir.path().join("evil.txt");
    std::fs::write(&outside_file, b"escaped!").unwrap();

    let result_json = serde_json::json!({
        "execution_id": "exec_test",
        "status": "success",
        "execution_time_ms": 150,
        "metrics": {},
        "artifacts": {
            "evil": {
                "file_path": outside_file.to_string_lossy(),
                "format": "text/plain"
            }
        }
    });

    let result_path = out_dir.join("result.json");
    std::fs::write(
        &result_path,
        serde_json::to_string_pretty(&result_json).unwrap(),
    )
    .unwrap();

    let err = read_and_validate_result(&result_path, out_dir);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().code(), "SECURITY_VIOLATION");
}

#[test]
fn test_build_plugin_command_python_and_unsupported_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("main.py");
    std::fs::write(&script, "print('ok')\n").unwrap();

    let cmd = build_plugin_command(
        dir.path(),
        "main.py",
        "python",
        &[std::ffi::OsStr::new("--healthcheck")],
    );
    assert!(cmd.is_ok());

    let missing = build_plugin_command(
        dir.path(),
        "missing.py",
        "python",
        &[std::ffi::OsStr::new("--healthcheck")],
    );
    assert!(matches!(missing, Err(PluginError::SpawnFailed(_))));

    let unsupported = build_plugin_command(dir.path(), "main.wasm", "wasm", &[]);
    assert!(matches!(unsupported, Err(PluginError::SpawnFailed(_))));
}

#[test]
fn test_resolve_executable_python_binary_missing_and_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("main.py");
    std::fs::write(&script, "print('ok')\n").unwrap();

    let bin_name = if cfg!(target_os = "windows") {
        "worker.exe"
    } else {
        "worker"
    };
    let bin_path = dir.path().join(bin_name);
    std::fs::write(&bin_path, b"stub").unwrap();

    let payload_path = dir.path().join("payload.json");
    let result_path = dir.path().join("result.json");

    let py_cmd =
        resolve_executable(dir.path(), "main.py", "python", &payload_path, &result_path).unwrap();
    let py_args: Vec<String> = py_cmd
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(py_args.contains(&"--input".to_string()));
    assert!(py_args.contains(&"--output".to_string()));
    assert!(py_args.contains(&payload_path.to_string_lossy().into_owned()));
    assert!(py_args.contains(&result_path.to_string_lossy().into_owned()));

    let bin_cmd =
        resolve_executable(dir.path(), "worker", "binary", &payload_path, &result_path).unwrap();
    assert_eq!(
        PathBuf::from(bin_cmd.as_std().get_program()),
        std::path::absolute(&bin_path).unwrap_or_else(|_| bin_path.clone())
    );

    let missing = resolve_executable(
        dir.path(),
        "absent.py",
        "python",
        &payload_path,
        &result_path,
    );
    assert!(matches!(missing, Err(PluginError::SpawnFailed(_))));

    let unsupported =
        resolve_executable(dir.path(), "main.py", "ruby", &payload_path, &result_path);
    assert!(matches!(unsupported, Err(PluginError::SpawnFailed(_))));
}

#[test]
fn test_autofill_image_metadata_from_sqlite_fixture() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::modules::module_01::init_schema(&conn).unwrap();

    let project_id = uuid::Uuid::new_v4().to_string();
    let session_id = uuid::Uuid::new_v4().to_string();
    let full_img_id = uuid::Uuid::new_v4().to_string();
    let sparse_img_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO project (id, name, created_at, updated_at) VALUES (?1, 'Proj', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [&project_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session (id, project_id, name, created_at, updated_at) VALUES (?1, ?2, 'Sess', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [&session_id, &project_id],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at, gps_altitude_m, width, height)
         VALUES (?1, ?2, '/lib/full.jpg', -1.25, 116.85, '2026-05-01T08:30:00', 82.5, 4000, 3000)",
        [&full_img_id, &session_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO image (id, session_id, file_path)
         VALUES (?1, ?2, '/lib/sparse.jpg')",
        [&sparse_img_id, &session_id],
    )
    .unwrap();

    assert_eq!(autofill_image_metadata(&conn, "/lib/missing.jpg"), None);
    assert_eq!(autofill_image_metadata(&conn, "/lib/sparse.jpg"), None);

    let full = autofill_image_metadata(&conn, "/lib/full.jpg").expect("metadata expected");
    assert_eq!(full["resolution"], serde_json::json!([4000, 3000]));
    assert_eq!(
        full["gps"],
        serde_json::json!({ "lat": -1.25, "lon": 116.85 })
    );
    assert_eq!(full["altitude"], serde_json::json!(82.5));
    assert_eq!(full["relative_altitude"], serde_json::json!(82.5));
    assert_eq!(full["timestamp"], serde_json::json!("2026-05-01 08:30:00"));
}

#[tokio::test]
async fn test_supervise_execution_streams_progress_and_validates_result() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path().join("plugin");
    let out_dir = temp_dir.path().join("out");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let script = plugin_dir.join("main.py");
    std::fs::write(
        &script,
        r#"
import json, pathlib, sys, time
out_idx = sys.argv.index("--output") + 1
result_path = pathlib.Path(sys.argv[out_idx])
artifact = result_path.parent / "overlay.png"
artifact.write_bytes(b"png")
print('PROGRESS: {"job_id": "exec_sup_1", "percent": 40, "stage": "inferring"}', flush=True)
time.sleep(0.05)
result_path.write_text(json.dumps({
    "execution_id": "exec_sup_1",
    "status": "success",
    "execution_time_ms": 12,
    "metrics": {"coverage_pct": 88.0},
    "artifacts": {"overlay": {"file_path": "overlay.png", "format": "image/png"}}
}))
"#,
    )
    .unwrap();

    let payload_path = temp_dir.path().join("payload.json");
    std::fs::write(&payload_path, "{}").unwrap();
    let result_path = out_dir.join("result.json");

    let mut cmd = resolve_executable(
        &plugin_dir,
        "main.py",
        "python",
        &payload_path,
        &result_path,
    )
    .unwrap();
    let child = cmd.spawn().unwrap();

    let tracker = std::sync::Arc::new(tokio::sync::RwLock::new(ActiveJobTracker::new()));
    tracker.write().await.jobs.insert(
        "exec_sup_1".to_string(),
        JobRecord {
            job_id: "exec_sup_1".to_string(),
            plugin_id: "p1".to_string(),
            status: JobStatusDto::Queued,
            result: None,
            started_at: "2026-01-01T00:00:00Z".to_string(),
            abort_handle: None,
            child_pid: child.id(),
            output_dir: Some(out_dir.clone()),
        },
    );

    let mut events = Vec::new();
    let dto = supervise_execution_with_emitter(
        child,
        "exec_sup_1".to_string(),
        result_path,
        out_dir,
        10,
        tracker.clone(),
        |progress| events.push(progress.clone()),
    )
    .await
    .unwrap();

    assert_eq!(dto.status, "success");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].percent, 40);
    assert_eq!(events[0].stage, "inferring");

    let guard = tracker.read().await;
    assert!(matches!(
        guard.jobs["exec_sup_1"].status,
        JobStatusDto::Running {
            progress_pct: 40,
            ..
        }
    ));
}

#[tokio::test]
async fn test_supervise_execution_captures_stderr_and_enforces_timeout() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path().join("plugin");
    let out_dir = temp_dir.path().join("out");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let fail_script = plugin_dir.join("fail.py");
    std::fs::write(
        &fail_script,
        "import sys\nsys.stderr.write('boom diagnostics\\n')\nsys.exit(3)\n",
    )
    .unwrap();
    let payload_path = temp_dir.path().join("payload.json");
    std::fs::write(&payload_path, "{}").unwrap();
    let result_path = out_dir.join("result.json");

    let tracker = std::sync::Arc::new(tokio::sync::RwLock::new(ActiveJobTracker::new()));
    let mut fail_cmd = resolve_executable(
        &plugin_dir,
        "fail.py",
        "python",
        &payload_path,
        &result_path,
    )
    .unwrap();
    let fail_child = fail_cmd.spawn().unwrap();
    let fail_err = supervise_execution_with_emitter(
        fail_child,
        "exec_fail".to_string(),
        result_path.clone(),
        out_dir.clone(),
        10,
        tracker.clone(),
        |_| {},
    )
    .await
    .unwrap_err();
    match fail_err {
        PluginError::SpawnFailed(msg) => assert!(msg.contains("boom diagnostics")),
        other => panic!("Expected SpawnFailed with stderr, got {:?}", other),
    }

    let hang_script = plugin_dir.join("hang.py");
    std::fs::write(&hang_script, "import time\ntime.sleep(30)\n").unwrap();
    let mut hang_cmd = resolve_executable(
        &plugin_dir,
        "hang.py",
        "python",
        &payload_path,
        &result_path,
    )
    .unwrap();
    let hang_child = hang_cmd.spawn().unwrap();
    let timeout_err = supervise_execution_with_emitter(
        hang_child,
        "exec_timeout".to_string(),
        result_path,
        out_dir,
        1,
        tracker,
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(matches!(timeout_err, PluginError::Timeout(1)));
}

#[tokio::test]
async fn test_start_plugin_job_gating_autofill_and_execution() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path().join("plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let script = plugin_dir.join("main.py");
    std::fs::write(
        &script,
        r#"
import json, pathlib, sys
in_path = pathlib.Path(sys.argv[sys.argv.index("--input") + 1])
out_path = pathlib.Path(sys.argv[sys.argv.index("--output") + 1])
payload = json.loads(in_path.read_text())
has_res = payload.get("target", {}).get("metadata", {}).get("resolution") == [2000, 1500]
out_path.write_text(json.dumps({
    "execution_id": payload["execution_id"],
    "status": "success" if has_res else "failure",
    "execution_time_ms": 5,
    "metrics": {"autofilled": has_res},
    "artifacts": {},
    "error": None if has_res else "Missing autofilled resolution"
}))
"#,
    )
    .unwrap();

    let image_path = temp_dir.path().join("sample.jpg");
    std::fs::write(&image_path, b"jpg").unwrap();
    let image_str = image_path.to_string_lossy().into_owned();

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::modules::module_01::init_schema(&conn).unwrap();
    let project_id = uuid::Uuid::new_v4().to_string();
    let session_id = uuid::Uuid::new_v4().to_string();
    let image_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO project (id, name, created_at, updated_at) VALUES (?1, 'Proj', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [&project_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session (id, project_id, name, created_at, updated_at) VALUES (?1, ?2, 'Sess', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [&session_id, &project_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO image (id, session_id, file_path, width, height) VALUES (?1, ?2, ?3, 2000, 1500)",
        [&image_id, &session_id, &image_str],
    )
    .unwrap();

    let mut registry = PluginRegistry::new();
    registry.register(mock_plugin_bundle(
        "enabled-plugin",
        plugin_dir.clone(),
        "python",
        "main.py",
        true,
        10,
    ));
    registry.register(mock_plugin_bundle(
        "disabled-plugin",
        plugin_dir,
        "python",
        "main.py",
        false,
        10,
    ));

    let state = test_app_state(registry, conn);

    let missing_err = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "unknown".to_string(),
            session_id: None,
            target_image_path: image_str.clone(),
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: None,
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap_err();
    assert_eq!(missing_err.code, "PLUGIN_UNAVAILABLE");

    let disabled_err = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "disabled-plugin".to_string(),
            session_id: None,
            target_image_path: image_str.clone(),
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: None,
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap_err();
    assert_eq!(disabled_err.code, "PLUGIN_UNAVAILABLE");

    let missing_img_err = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "enabled-plugin".to_string(),
            session_id: None,
            target_image_path: temp_dir
                .path()
                .join("missing.jpg")
                .to_string_lossy()
                .into_owned(),
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: None,
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap_err();
    assert_eq!(missing_img_err.code, "IMAGE_NOT_FOUND");

    let handle = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "enabled-plugin".to_string(),
            session_id: Some("s1".to_string()),
            target_image_path: image_str,
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: None,
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(handle.status, "running");

    let mut completed_result = None;
    let mut output_dir_to_clean = None;
    for _ in 0..50 {
        {
            let guard = state.jobs.read().await;
            let rec = &guard.jobs[&handle.job_id];
            output_dir_to_clean = rec.output_dir.clone();
            if matches!(rec.status, JobStatusDto::Completed { .. }) {
                completed_result = rec.result.clone();
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    if let Some(dir) = output_dir_to_clean {
        let _ = std::fs::remove_dir_all(dir);
    }

    let dto = completed_result.expect("job should reach Completed");
    assert_eq!(dto.status, "success");
    assert_eq!(
        dto.metrics.get("autofilled"),
        Some(&serde_json::json!(true))
    );
}

#[tokio::test]
async fn test_abort_plugin_job_internal_missing_finished_and_running_cleanup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path().join("plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("main.py"), "import time\ntime.sleep(30)\n").unwrap();

    let image_path = temp_dir.path().join("img.jpg");
    std::fs::write(&image_path, b"jpg").unwrap();

    let mut registry = PluginRegistry::new();
    registry.register(mock_plugin_bundle(
        "sleep-plugin",
        plugin_dir,
        "python",
        "main.py",
        true,
        30,
    ));

    let state = test_app_state(registry, rusqlite::Connection::open_in_memory().unwrap());

    let missing_err = abort_plugin_job_internal("nonexistent-job", &state)
        .await
        .unwrap_err();
    assert_eq!(missing_err.code, "JOB_NOT_FOUND");

    // Pre-finished job should clean up directory and clear pid without killing arbitrary PIDs
    let finished_dir = temp_dir.path().join("finished_out");
    std::fs::create_dir_all(&finished_dir).unwrap();
    state.jobs.write().await.jobs.insert(
        "finished-job".to_string(),
        JobRecord {
            job_id: "finished-job".to_string(),
            plugin_id: "sleep-plugin".to_string(),
            status: JobStatusDto::Completed {
                execution_time_ms: 10,
            },
            result: None,
            started_at: "2026-01-01T00:00:00Z".to_string(),
            abort_handle: None,
            child_pid: Some(999_999),
            output_dir: Some(finished_dir.clone()),
        },
    );
    abort_plugin_job_internal("finished-job", &state)
        .await
        .unwrap();
    assert!(!finished_dir.exists());
    {
        let guard = state.jobs.read().await;
        let rec = &guard.jobs["finished-job"];
        assert!(matches!(rec.status, JobStatusDto::Aborted));
        assert_eq!(rec.child_pid, None);
    }

    // Running live subprocess gets killed and its sandbox dir gets removed
    let handle = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "sleep-plugin".to_string(),
            session_id: None,
            target_image_path: image_path.to_string_lossy().into_owned(),
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: Some(serde_json::json!({})),
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap();

    let live_dir = {
        let guard = state.jobs.read().await;
        let rec = &guard.jobs[&handle.job_id];
        assert!(rec.child_pid.is_some());
        rec.output_dir.clone().unwrap()
    };
    assert!(live_dir.exists());

    abort_plugin_job_internal(&handle.job_id, &state)
        .await
        .unwrap();
    assert!(!live_dir.exists());
    let guard = state.jobs.read().await;
    let rec = &guard.jobs[&handle.job_id];
    assert!(matches!(rec.status, JobStatusDto::Aborted));
    assert_eq!(rec.child_pid, None);
}

#[tokio::test]
async fn test_start_plugin_job_missing_entrypoint_and_fast_exit_no_race() {
    let temp_dir = tempfile::tempdir().unwrap();
    let plugin_dir = temp_dir.path().join("plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    // Missing entrypoint triggers resolve_executable failure and cleans up jobs map
    let image_path = temp_dir.path().join("img.jpg");
    std::fs::write(&image_path, b"jpg").unwrap();
    let image_str = image_path.to_string_lossy().into_owned();

    let mut registry = PluginRegistry::new();
    registry.register(mock_plugin_bundle(
        "broken-plugin",
        plugin_dir.clone(),
        "python",
        "nonexistent.py",
        true,
        10,
    ));

    // Fast-exiting script finishes immediately to stress-test job registration ordering
    std::fs::write(
        plugin_dir.join("fast.py"),
        "import sys\nsys.stderr.write('fast fail\\n')\nsys.exit(1)\n",
    )
    .unwrap();
    registry.register(mock_plugin_bundle(
        "fast-plugin",
        plugin_dir,
        "python",
        "fast.py",
        true,
        10,
    ));

    let state = test_app_state(registry, rusqlite::Connection::open_in_memory().unwrap());

    let missing_err = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "broken-plugin".to_string(),
            session_id: None,
            target_image_path: image_str.clone(),
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: Some(serde_json::json!({})),
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap_err();
    assert_eq!(missing_err.code, "SPAWN_FAILED");
    assert!(state.jobs.read().await.jobs.is_empty());

    let handle = start_plugin_job_internal(
        StartJobRequestDto {
            plugin_id: "fast-plugin".to_string(),
            session_id: None,
            target_image_path: image_str,
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            metadata: Some(serde_json::json!({})),
            parameters: HashMap::new(),
        },
        None,
        &state,
    )
    .await
    .unwrap();

    let mut reached_failed = false;
    let mut output_dir_to_clean = None;
    for _ in 0..50 {
        {
            let guard = state.jobs.read().await;
            let rec = &guard.jobs[&handle.job_id];
            output_dir_to_clean = rec.output_dir.clone();
            if matches!(rec.status, JobStatusDto::Failed { .. }) {
                reached_failed = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    if let Some(dir) = output_dir_to_clean {
        let _ = std::fs::remove_dir_all(dir);
    }

    assert!(
        reached_failed,
        "fast-exiting plugin must transition to Failed rather than remaining stuck in Running"
    );
}
