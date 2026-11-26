//! # Domain 3: Async Execution, Progress & Output Resolution
//!
//! **Role**: Subprocess Supervisor & Isolation Engineer (Imperative Shell & Facade)
//! **Deliverable**: `plugin_manager/executor.rs`
//!
//! Responsible for:
//! - M2.7: Plugin job execution (spawn subprocess, enforce timeout)
//! - M2.9: Error isolation (a crashing plugin MUST NOT crash the host)
//!
//! ## Architectural Mandate
//! All subprocess operations MUST be async (Tokio). The Tauri main thread must
//! never block. A plugin that hangs, panics, or exhausts memory MUST be killed
//! and its failure surfaced as a `JobStatusDto::Failed` — never a host panic.
//!
//! ## Purity Boundary
//!
//! This module is the **Imperative Shell** and public/crate facade over
//! [`super::executor_core`]. All Tauri IPC command handling, `AppState` /
//! `tokio::sync::RwLock` state synchronization, `rusqlite` metadata lookups,
//! filesystem I/O, sandbox directory creation, subprocess spawning and
//! supervision, OS process-tree signaling, `chrono::Utc::now()` clock reads,
//! `uuid::Uuid::new_v4()` ID generation, and `tokio::task::spawn_blocking`
//! dispatch live here. Pure domain logic (execution gating, target extraction,
//! payload construction, progress parsing/throttling, kill-target guards, and
//! parallel JSON result/artifact parsing) is delegated to
//! [`super::executor_core`].
//!
//! Refer to: `src-tauri/src/plugin_manager/DOCS/03_DOMAIN_3_EXECUTION_SUPERVISOR.md`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use super::error::{CommandError, PluginError};
pub(crate) use super::executor_core::{
    build_execution_payload, extract_execution_target, kill_target, parse_progress_line,
    parse_result_json, should_emit_progress, KillTarget, PluginExecutionTarget,
};
pub use super::executor_core::{JobHandleDto, JobProgress, JobStatusDto, StartJobRequestDto};
use super::models::StandardJobResultDto;

// ---------------------------------------------------------------------------
// 2. DATA TRANSFER OBJECTS (re-exported from `executor_core`)
// ---------------------------------------------------------------------------
//
// `StartJobRequestDto`, `JobHandleDto`, `JobProgress`, and `JobStatusDto`
// live in `super::executor_core` and are re-exported above.

// ---------------------------------------------------------------------------
// 3. ACTIVE JOB TRACKER (Shared State)
// ---------------------------------------------------------------------------
//
// `VerifiedArtifactDto`, `StandardJobResultDto`, `ExecutionPayload`, and
// `ExecutionTarget` live in `super::models` (Role 3's Data Bridge deliverable)
// — imported above — so this module only owns job-tracking/IPC-transport DTOs.

/// Internal record stored per active or completed job.
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub job_id: String,
    pub plugin_id: String,
    pub status: JobStatusDto,
    /// Populated once job reaches `Completed` state.
    pub result: Option<StandardJobResultDto>,
    pub started_at: String,
    /// Async abort handle to cancel Tokio supervisor task.
    pub abort_handle: Option<tokio::task::AbortHandle>,
    /// Child process ID (for process tree termination).
    pub child_pid: Option<u32>,
    /// Sandboxed output directory for cleanup on abort.
    pub output_dir: Option<PathBuf>,
}

/// Shared state managed by Tauri's `.manage()` via `Arc<RwLock<ActiveJobTracker>>`.
///
/// Stores all in-flight and recently completed jobs so the UI can poll or
/// re-attach after a page navigation.
#[derive(Debug, Default)]
pub struct ActiveJobTracker {
    pub jobs: HashMap<String, JobRecord>,
}

impl ActiveJobTracker {
    /// Creates an empty active-job tracker map.
    ///
    /// # Purity
    /// Pure constructor for an empty in-memory map.
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// 4. TAURI IPC COMMANDS (pub — registered in lib.rs invoke_handler)
// ---------------------------------------------------------------------------

/// Initiates an asynchronous plugin analysis job.
///
/// Serves as the primary execution launcher exposed to the frontend via Tauri IPC (`invoke("start_plugin_job", { request })`).
/// Invoked by Module 1 (Image Management), Module 3 (Geospatial Map Explorer), and Module 10 (Temporal Change Analysis)
/// to kick off compute-intensive analysis workflows in an isolated child process without blocking the main Tauri desktop runtime.
///
/// # Purity
/// Impure Tauri IPC command handler: orchestrates registry read locks, SQLite
/// metadata autofill, sandbox filesystem I/O, subprocess spawning, and tracker mutation.
#[tauri::command]
pub async fn start_plugin_job(
    mut request: StartJobRequestDto,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<JobHandleDto, CommandError> {
    let target = resolve_plugin_target(&request.plugin_id, &state).await?;
    validate_and_autofill_request(&mut request, &state).await?;

    let job_id = generate_job_id();
    let (output_dir, payload_path, result_path) =
        prepare_sandbox_payload(&job_id, &request).await?;

    let mut cmd = {
        let plugin_dir = target.plugin_dir.clone();
        let entrypoint = target.entrypoint.clone();
        let runtime_type = target.runtime_type.clone();
        let payload_path_owned = payload_path.clone();
        let result_path_owned = result_path.clone();
        tokio::task::spawn_blocking(move || {
            resolve_executable(
                &plugin_dir,
                &entrypoint,
                &runtime_type,
                &payload_path_owned,
                &result_path_owned,
            )
        })
        .await
        .map_err(|e| CommandError {
            code: "SPAWN_FAILED".to_string(),
            message: format!("Plugin executable resolution task failed: {}", e),
        })??
    };

    let child = cmd.spawn().map_err(|e| CommandError {
        code: "SPAWN_FAILED".to_string(),
        message: format!("Failed to spawn plugin subprocess: {}", e),
    })?;
    let child_pid = child.id();

    let supervisor_task = spawn_supervisor_task(
        child,
        job_id.clone(),
        result_path,
        output_dir.clone(),
        target.timeout_seconds,
        app,
        state.jobs.clone(),
    );

    {
        let mut tracker = state.jobs.write().await;
        tracker.jobs.insert(
            job_id.clone(),
            JobRecord {
                job_id: job_id.clone(),
                plugin_id: request.plugin_id.clone(),
                status: JobStatusDto::Running {
                    progress_pct: 0,
                    stage: "spawning".to_string(),
                },
                result: None,
                started_at: Utc::now().to_rfc3339(),
                abort_handle: Some(supervisor_task.abort_handle()),
                child_pid,
                output_dir: Some(output_dir),
            },
        );
    }

    Ok(JobHandleDto {
        job_id,
        plugin_id: request.plugin_id,
        status: "running".to_string(),
    })
}

/// Acquires a short read lock on `state.plugins` and delegates pure execution
/// gating + target extraction to [`extract_execution_target`].
///
/// # Purity
/// Imperative shell: acquires async read lock on `state.plugins`, drops it
/// immediately, and delegates target extraction to pure [`extract_execution_target`].
async fn resolve_plugin_target(
    plugin_id: &str,
    state: &crate::AppState,
) -> Result<PluginExecutionTarget, CommandError> {
    let registry = state.plugins.read().await;
    extract_execution_target(plugin_id, &registry).map_err(Into::into)
}

/// Validates target image existence on disk and backfills missing EXIF/image
/// metadata from SQLite when the caller omitted `request.metadata`.
///
/// # Purity
/// Imperative shell: checks filesystem existence via `tokio::fs` and reads SQLite
/// under `state.db` lock inside `tokio::task::spawn_blocking`.
async fn validate_and_autofill_request(
    request: &mut StartJobRequestDto,
    state: &crate::AppState,
) -> Result<(), CommandError> {
    let target_img = PathBuf::from(&request.target_image_path);
    if !tokio::fs::try_exists(&target_img).await.unwrap_or(false) {
        return Err(CommandError {
            code: "IMAGE_NOT_FOUND".to_string(),
            message: format!("Target image file not found: {:?}", target_img),
        });
    }

    // Callers that did not decode EXIF can still get complete payloads:
    // fill resolution/GPS/altitude/timestamp from the imported image row.
    if request.metadata.is_none() {
        let db = state.db.clone();
        let target_path = request.target_image_path.clone();
        request.metadata = tokio::task::spawn_blocking(move || {
            let conn = db.lock().map_err(|_| CommandError {
                code: "STATE_ERROR".to_string(),
                message: "Database lock poisoned while resolving image metadata".to_string(),
            })?;
            Ok::<_, CommandError>(autofill_image_metadata(&conn, &target_path))
        })
        .await
        .map_err(|e| CommandError {
            code: "STATE_ERROR".to_string(),
            message: format!("Image metadata lookup task failed: {}", e),
        })??;
    }

    Ok(())
}

/// Creates a sandboxed output directory, constructs the canonical execution
/// payload via [`build_execution_payload`], and writes `payload.json` to disk.
///
/// # Purity
/// Imperative shell: creates temporary directory on `tokio::task::spawn_blocking`
/// and writes `payload.json` via `tokio::fs`.
async fn prepare_sandbox_payload(
    job_id: &str,
    request: &StartJobRequestDto,
) -> Result<(PathBuf, PathBuf, PathBuf), CommandError> {
    let prefix = format!("{}_", job_id);
    let output_dir = tokio::task::spawn_blocking(move || {
        tempfile::Builder::new()
            .prefix(&prefix)
            .tempdir()
            .map(|dir| dir.keep())
            .map_err(|e| CommandError {
                code: "IO_ERROR".to_string(),
                message: format!("Failed to create sandboxed output directory: {}", e),
            })
    })
    .await
    .map_err(|e| CommandError {
        code: "IO_ERROR".to_string(),
        message: format!("Sandbox setup task failed: {}", e),
    })??;

    let payload = build_execution_payload(job_id, &output_dir, request)?;
    let payload_path = output_dir.join("payload.json");
    let result_path = output_dir.join("result.json");

    let payload_str = serde_json::to_string_pretty(&payload).map_err(|e| CommandError {
        code: "SERIALIZATION_ERROR".to_string(),
        message: format!("Failed to serialize execution payload: {}", e),
    })?;

    tokio::fs::write(&payload_path, payload_str)
        .await
        .map_err(|e| CommandError {
            code: "IO_ERROR".to_string(),
            message: format!("Failed to write payload.json: {}", e),
        })?;

    Ok((output_dir, payload_path, result_path))
}

/// Spawns the asynchronous supervisor task that awaits subprocess completion,
/// updates `ActiveJobTracker`, and emits `plugin://completed` IPC events.
///
/// # Purity
/// Imperative shell: spawns a Tokio background task mutating shared job state
/// and emitting Tauri IPC events.
fn spawn_supervisor_task(
    child: tokio::process::Child,
    job_id: String,
    result_path: PathBuf,
    output_dir: PathBuf,
    timeout_secs: u64,
    app: tauri::AppHandle,
    tracker_arc: std::sync::Arc<tokio::sync::RwLock<ActiveJobTracker>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let res = supervise_execution(
            child,
            job_id.clone(),
            result_path,
            output_dir,
            timeout_secs,
            app.clone(),
            tracker_arc.clone(),
        )
        .await;

        let mut tracker = tracker_arc.write().await;
        if let Some(record) = tracker.jobs.get_mut(&job_id) {
            use tauri::Emitter;
            match res {
                Ok(dto) => {
                    if dto.status == "failure" {
                        record.status = JobStatusDto::Failed {
                            error: dto
                                .error
                                .clone()
                                .unwrap_or_else(|| "Plugin reported execution failure".to_string()),
                        };
                    } else {
                        record.status = JobStatusDto::Completed {
                            execution_time_ms: dto.execution_time_ms,
                        };
                    }
                    record.result = Some(dto.clone());
                    // The pid is stale from now on: never signal it again.
                    record.child_pid = None;
                    let _ = app.emit("plugin://completed", &dto);
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    record.status = JobStatusDto::Failed {
                        error: err_msg.clone(),
                    };
                    record.child_pid = None;
                    let _ = app.emit(
                        "plugin://completed",
                        serde_json::json!({
                            "job_id": job_id,
                            "status": "failure",
                            "error": err_msg,
                        }),
                    );
                }
            }
        }
    })
}

/// Forcibly terminates an in-flight plugin job.
///
/// Provides user-cancellation capabilities exposed to the frontend via Tauri IPC (`invoke("abort_plugin_job", { jobId })`).
/// Invoked by Module 3 (Map Explorer task drawer) and Module 11 (Task Monitor) when a user cancels an active analysis,
/// ensuring runaway or stalled subprocesses are cleanly killed.
///
/// # Purity
/// Impure Tauri IPC command handler; delegates tracker mutation and blocking
/// cleanup to [`abort_plugin_job_internal`].
#[tauri::command]
pub async fn abort_plugin_job(
    job_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    abort_plugin_job_internal(&job_id, &state).await
}

/// Internal helper that aborts an in-flight job, releases `state.jobs` before
/// blocking cleanup, and kills the OS process tree + removes sandbox outputs on
/// `tokio::task::spawn_blocking`.
///
/// # Purity
/// Imperative shell: mutates shared job tracker under short async write lock and
/// offloads OS process termination and filesystem cleanup to blocking threads.
pub(crate) async fn abort_plugin_job_internal(
    job_id: &str,
    state: &crate::AppState,
) -> Result<(), CommandError> {
    let (pid_to_kill, output_dir_to_remove) = {
        let mut tracker = state.jobs.write().await;
        let record = tracker
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| PluginError::JobNotFound(job_id.to_string()))?;

        // Abort async supervisor task
        if let Some(ref handle) = record.abort_handle {
            handle.abort();
        }

        // Forcibly terminate child OS process tree. Only for jobs that can still
        // be running: finished jobs keep no pid, and a stale pid could now belong
        // to an unrelated process (a negated pid would target its whole group).
        let finished = matches!(
            record.status,
            JobStatusDto::Completed { .. } | JobStatusDto::Failed { .. } | JobStatusDto::Aborted
        );
        let pid = if finished {
            record.child_pid = None;
            None
        } else {
            record.child_pid.take()
        };
        let dir = record.output_dir.clone();
        record.status = JobStatusDto::Aborted;
        (pid, dir)
    };

    if pid_to_kill.is_some() || output_dir_to_remove.is_some() {
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(pid) = pid_to_kill {
                kill_process_tree(pid);
            }
            if let Some(dir) = output_dir_to_remove {
                let _ = std::fs::remove_dir_all(dir);
            }
        })
        .await;
    }

    Ok(())
}

/// Polls the current lifecycle state of a job.
///
/// Exposes read-only job status polling to the frontend via Tauri IPC (`invoke("get_job_status", { jobId })`).
///
/// # Purity
/// Impure Tauri IPC command handler: reads shared async `AppState.jobs` lock.
#[tauri::command]
pub async fn get_job_status(
    job_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<JobStatusDto, CommandError> {
    let tracker = state.jobs.read().await;
    let record = tracker
        .jobs
        .get(&job_id)
        .ok_or_else(|| PluginError::JobNotFound(job_id.clone()))?;
    Ok(record.status.clone())
}

/// Delivers the full analytical result to downstream modules.
///
/// Exposes validated execution outputs to downstream modules via Tauri IPC (`invoke("get_job_result", { jobId })`).
///
/// # Purity
/// Impure Tauri IPC command handler: reads shared async `AppState.jobs` lock.
#[tauri::command]
pub async fn get_job_result(
    job_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<StandardJobResultDto, CommandError> {
    let tracker = state.jobs.read().await;
    let record = tracker
        .jobs
        .get(&job_id)
        .ok_or_else(|| PluginError::JobNotFound(job_id.clone()))?;

    match &record.status {
        JobStatusDto::Completed { .. } => record.result.clone().ok_or_else(|| CommandError {
            code: "RESULT_MISSING".to_string(),
            message: format!("Job '{}' is completed but result is missing", job_id),
        }),
        _ => Err(PluginError::NotCompleted(job_id).into()),
    }
}

/// Emits a real-time job progress event to the frontend over Tauri IPC.
///
/// Streams progress payloads emitted by child subprocesses to listening UI components via Tauri's event bus.
///
/// # Purity
/// Impure IPC side effect: emits `"plugin://progress"` over Tauri's `AppHandle`.
pub fn emit_job_progress(
    app: &tauri::AppHandle,
    progress: &JobProgress,
) -> Result<(), CommandError> {
    use tauri::Emitter;
    app.emit("plugin://progress", progress)
        .map_err(|e| CommandError {
            code: "IPC_EMIT_FAILED".to_string(),
            message: format!("Failed to emit progress event: {}", e),
        })
}

// ---------------------------------------------------------------------------
// 5. INTERNAL HELPERS (pub(crate) — I/O, called from the background task)
// ---------------------------------------------------------------------------
//
// Pure functions (`extract_execution_target`, `build_execution_payload`,
// `parse_progress_line`, `should_emit_progress`, `kill_target`,
// `parse_result_json`) and `PluginExecutionTarget` / `KillTarget` live in
// `super::executor_core` and are re-exported above.

/// Builds plugin payload metadata from the imported image row.
///
/// Returns `None` when the path is not a tracked library image or when the row
/// carries no usable values, so plugins keep their own fallbacks.
///
/// # Purity
/// Impure: performs a synchronous SQLite query against an already-open `&rusqlite::Connection`.
pub(crate) fn autofill_image_metadata(
    conn: &rusqlite::Connection,
    image_path: &str,
) -> Option<serde_json::Value> {
    let image =
        crate::modules::module_01::flight_session::find_image_by_path(conn, image_path).ok()??;

    let mut metadata = serde_json::Map::new();
    if let (Some(width), Some(height)) = (image.width, image.height) {
        metadata.insert("resolution".to_string(), serde_json::json!([width, height]));
    }
    if let (Some(lat), Some(lon)) = (image.location_lat, image.location_lon) {
        metadata.insert(
            "gps".to_string(),
            serde_json::json!({ "lat": lat, "lon": lon }),
        );
    }
    if let Some(altitude) = image.gps_altitude_m {
        metadata.insert("altitude".to_string(), serde_json::json!(altitude));
        metadata.insert("relative_altitude".to_string(), serde_json::json!(altitude));
    }
    if let Some(captured_at) = image.captured_at {
        metadata.insert(
            "timestamp".to_string(),
            serde_json::json!(captured_at.to_string()),
        );
    }

    if metadata.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(metadata))
    }
}

/// Attempts to read and parse `dir/manifest.json`, returning a [`PluginExecutionTarget`]
/// if `metadata.id` matches `plugin_id`.
///
/// # Purity
/// Impure filesystem I/O: checks `dir.is_dir()` and reads `manifest.json` from disk.
fn try_load_target_from_dir(dir: &Path, plugin_id: &str) -> Option<PluginExecutionTarget> {
    if !dir.is_dir() {
        return None;
    }
    let manifest_path = dir.join("manifest.json");
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let val = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let id = val["metadata"]["id"].as_str().unwrap_or_default();
    if id != plugin_id {
        return None;
    }

    Some(PluginExecutionTarget {
        plugin_dir: dir.to_path_buf(),
        entrypoint: val["runtime"]["entrypoint"]
            .as_str()
            .unwrap_or("main.py")
            .to_string(),
        runtime_type: val["runtime"]["type"]
            .as_str()
            .unwrap_or("python")
            .to_string(),
        timeout_seconds: val["execution"]["timeout_seconds"].as_u64().unwrap_or(60),
    })
}

/// Locates a plugin's directory and manifest by `plugin_id`.
///
/// # Purity
/// Impure filesystem I/O: scans candidate plugin directories and reads `manifest.json`.
#[allow(dead_code)]
pub(crate) fn find_plugin_info(plugin_id: &str) -> Result<PluginExecutionTarget, PluginError> {
    let possible_roots = [
        PathBuf::from("plugins"),
        PathBuf::from("../plugins"),
        PathBuf::from("../../plugins"),
    ];

    for root in &possible_roots {
        if !root.exists() {
            continue;
        }

        // Direct directory name check: plugins/<plugin_id>/manifest.json
        if let Some(target) = try_load_target_from_dir(&root.join(plugin_id), plugin_id) {
            return Ok(target);
        }

        // Search subdirectories for matching manifest ID
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                if let Some(target) = try_load_target_from_dir(&entry.path(), plugin_id) {
                    return Ok(target);
                }
            }
        }
    }

    Err(PluginError::PluginUnavailable(plugin_id.to_string()))
}

/// Forcibly terminates a plugin's entire process tree.
///
/// On Unix, the child is spawned as its own process group leader (see
/// `resolve_executable`), so signaling the negated PID targets the whole
/// group — including grandchildren (e.g. Python `multiprocessing` workers
/// or OpenCV thread pools) that would otherwise survive as orphaned
/// zombies if only the direct child PID were signaled. On Windows,
/// `taskkill /T` already walks the process tree.
///
/// # Purity
/// Impure OS process-control side effect: reads `/proc`, emits `eprintln!`,
/// and sends `SIGKILL` (Unix) or spawns `taskkill` (Windows).
pub(crate) fn kill_process_tree(pid: u32) {
    match kill_target(pid, process_group_of(pid), std::process::id()) {
        KillTarget::Refuse(reason) => {
            eprintln!("plugin cancel: refusing to signal pid {pid} ({reason})");
        }
        KillTarget::Process => {
            eprintln!("plugin cancel: SIGKILL pid {pid}");
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .output();
            }
        }
        KillTarget::Group => {
            eprintln!("plugin cancel: SIGKILL process group {pid}");
            #[cfg(unix)]
            unsafe {
                // Negative pid targets the whole group, which is safe only
                // because `kill_target` verified the child is its leader.
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .output();
            }
        }
    }
}

/// Process group id read from `/proc/<pid>/stat`, when available.
///
/// # Purity
/// Impure OS/filesystem read of `/proc/<pid>/stat`.
#[cfg(unix)]
fn process_group_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Fields after the parenthesised command name: state ppid pgrp ...
    let after_comm = stat.rsplit_once(')')?.1;
    let mut fields = after_comm.split_whitespace();
    let _state = fields.next()?;
    let _ppid = fields.next()?;
    fields.next()?.parse().ok()
}

/// Non-Unix stub for process group lookup.
///
/// # Purity
/// Pure constant stub returning `None` on non-Unix targets.
#[cfg(not(unix))]
fn process_group_of(_pid: u32) -> Option<u32> {
    None
}

/// Configures standard plugin CLI flags, working directory, piped stdio, and
/// Builds a cross-platform `tokio::process::Command` for a Python or native binary plugin
/// entrypoint with piped stdout/stderr, working directory `plugin_dir`, and Unix
/// process-group isolation (`process_group(0)`).
///
/// # Purity
/// Impure filesystem/process helper: resolves absolute paths and probes entrypoint existence
/// synchronously on disk. Intended to be called inside `tokio::task::spawn_blocking` from
/// async callers.
pub(crate) fn build_plugin_command(
    plugin_dir: &Path,
    entrypoint: &str,
    runtime_type: &str,
    args: &[&std::ffi::OsStr],
) -> Result<tokio::process::Command, PluginError> {
    // Absolute paths only: spawning a relative program together with a
    // relative `current_dir` resolves the program against the child's cwd.
    let absolute_dir = std::path::absolute(plugin_dir).unwrap_or_else(|_| plugin_dir.to_path_buf());
    let plugin_dir = absolute_dir.as_path();

    let mut cmd = match runtime_type {
        "python" => {
            let venv_python = if cfg!(windows) {
                plugin_dir.join(".venv").join("Scripts").join("python.exe")
            } else {
                plugin_dir.join(".venv").join("bin").join("python")
            };

            let interpreter = if venv_python.exists() {
                venv_python
            } else if cfg!(windows) {
                PathBuf::from("python")
            } else {
                PathBuf::from("python3")
            };

            let script_path = plugin_dir.join(entrypoint);
            if !script_path.exists() {
                return Err(PluginError::SpawnFailed(format!(
                    "Plugin Python entrypoint script not found: {:?}",
                    script_path
                )));
            }

            let mut command = tokio::process::Command::new(interpreter);
            command.arg(&script_path);
            command
        }
        "binary" => {
            let mut bin_path = plugin_dir.join(entrypoint);
            if cfg!(windows) && !entrypoint.ends_with(".exe") {
                let exe_candidate = plugin_dir.join(format!("{}.exe", entrypoint));
                if exe_candidate.exists() {
                    bin_path = exe_candidate;
                }
            }

            if !bin_path.exists() {
                return Err(PluginError::SpawnFailed(format!(
                    "Plugin binary entrypoint not found: {:?}",
                    bin_path
                )));
            }

            tokio::process::Command::new(&bin_path)
        }
        other => {
            return Err(PluginError::SpawnFailed(format!(
                "Unsupported runtime type: '{}'",
                other
            )));
        }
    };

    cmd.args(args)
        .current_dir(plugin_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        // Make the child its own process group leader so kill_process_tree()
        // can signal the whole group (covers grandchild workers) instead of
        // only this direct PID.
        cmd.process_group(0);
    }
    Ok(cmd)
}

/// Prepares the cross-platform Tokio process command for plugin execution (`--input <payload> --output <result>`).
///
/// # Purity
/// Impure synchronous filesystem I/O: resolves absolute paths and checks entrypoint existence on disk
/// (callers inside async contexts should offload via `tokio::task::spawn_blocking`).
pub(crate) fn resolve_executable(
    plugin_dir: &Path,
    entrypoint: &str,
    runtime_type: &str,
    payload_path: &Path,
    result_path: &Path,
) -> Result<tokio::process::Command, PluginError> {
    build_plugin_command(
        plugin_dir,
        entrypoint,
        runtime_type,
        &[
            std::ffi::OsStr::new("--input"),
            payload_path.as_os_str(),
            std::ffi::OsStr::new("--output"),
            result_path.as_os_str(),
        ],
    )
}

/// Supervises an already-spawned child process to completion while streaming progress events.
///
/// # Purity
/// Impure async subprocess supervision: drains stdout/stderr pipes, reads the
/// monotonic clock (`Instant::now()`), mutates `jobs_tracker` under async write
/// locks, emits IPC progress events, enforces timeouts via `kill_process_tree`,
/// and offloads [`read_and_validate_result`] onto `tokio::task::spawn_blocking`.
pub(crate) async fn supervise_execution(
    mut child: tokio::process::Child,
    job_id: String,
    result_path: PathBuf,
    output_dir: PathBuf,
    timeout_secs: u64,
    app: tauri::AppHandle,
    jobs_tracker: std::sync::Arc<tokio::sync::RwLock<ActiveJobTracker>>,
) -> Result<StandardJobResultDto, PluginError> {
    use tokio::io::AsyncBufReadExt;
    use tokio::time::{timeout, Duration};

    let child_pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PluginError::SpawnFailed("Failed to capture stdout pipe".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| PluginError::SpawnFailed("Failed to capture stderr pipe".into()))?;

    // Drain stderr in background to prevent buffer deadlock
    let stderr_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = tokio::io::BufReader::new(stderr);
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).into_owned()
    });

    // Throttle window for progress updates: a chatty plugin writing many
    // PROGRESS: lines per second must not flood the shared jobs_tracker
    // write-lock or the Tauri IPC channel.
    const PROGRESS_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

    let supervise_fut = async {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        let mut last_emitted: Option<(u8, String, std::time::Instant)> = None;
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(progress) = parse_progress_line(&line) {
                let now = std::time::Instant::now();
                let emit = should_emit_progress(
                    last_emitted
                        .as_ref()
                        .map(|(pct, stage, at)| (*pct, stage.as_str(), *at)),
                    &progress,
                    now,
                    PROGRESS_MIN_INTERVAL,
                );

                if emit {
                    // Update tracker status in memory
                    {
                        let mut tracker = jobs_tracker.write().await;
                        if let Some(record) = tracker.jobs.get_mut(&job_id) {
                            record.status = JobStatusDto::Running {
                                progress_pct: progress.percent,
                                stage: progress.stage.clone(),
                            };
                        }
                    }
                    // Emit to Tauri IPC event stream
                    emit_job_progress(&app, &progress).ok();
                    last_emitted = Some((progress.percent, progress.stage.clone(), now));
                }
            }
        }
        child.wait().await
    };

    let execution_result = timeout(Duration::from_secs(timeout_secs), supervise_fut).await;

    match execution_result {
        Err(_) => {
            // Execution timed out: forcibly kill child process
            let _ = child.kill().await;
            if let Some(pid) = child_pid {
                let _ = tokio::task::spawn_blocking(move || {
                    kill_process_tree(pid);
                })
                .await;
            }
            Err(PluginError::Timeout(timeout_secs))
        }
        Ok(Err(io_err)) => Err(PluginError::Io(io_err.to_string())),
        Ok(Ok(status)) => {
            let stderr_output = stderr_handle.await.unwrap_or_default();
            let has_result_file = tokio::fs::try_exists(&result_path).await.unwrap_or(false);
            if status.success() || has_result_file {
                // Plugin may have written a structured failure result.json before exit 1.
                // Offload synchronous fs checks and result.json parsing off the Tokio worker.
                let rp = result_path.clone();
                let od = output_dir.clone();
                tokio::task::spawn_blocking(move || read_and_validate_result(&rp, &od))
                    .await
                    .map_err(|e| PluginError::Io(format!("Result validation task failed: {}", e)))?
            } else {
                Err(PluginError::SpawnFailed(format!(
                    "Plugin process failed with status {}. Stderr: {}",
                    status,
                    stderr_output.trim()
                )))
            }
        }
    }
}

/// Validates that a single artifact exists on disk and resides strictly inside `output_dir`.
///
/// # Purity
/// Impure filesystem I/O: checks `exists()` and canonicalizes paths to enforce
/// sandbox containment.
fn verify_artifact_sandbox(
    output_dir: &Path,
    key: &str,
    file_path: &str,
) -> Result<(), PluginError> {
    let fp = Path::new(file_path);
    if !fp.exists() {
        return Err(PluginError::ArtifactMissing(format!(
            "Artifact '{}' file not found at: {:?}",
            key, fp
        )));
    }

    match (output_dir.canonicalize(), fp.canonicalize()) {
        (Ok(canonical_dir), Ok(canonical_art)) => {
            if !canonical_art.starts_with(&canonical_dir) {
                return Err(PluginError::SecurityViolation(format!(
                    "Artifact '{}' escapes sandboxed output directory: {:?}",
                    key, fp
                )));
            }
            Ok(())
        }
        _ => Err(PluginError::SecurityViolation(format!(
            "Failed to verify sandbox canonical boundaries for artifact '{}': {:?}",
            key, fp
        ))),
    }
}

/// Reads the plugin output file and validates it against the execution result schema.
///
/// # Purity
/// Impure filesystem I/O: reads `result_path` from disk, delegates pure parallel
/// JSON parsing to [`parse_result_json`], and validates artifact paths on disk
/// via [`verify_artifact_sandbox`].
pub(crate) fn read_and_validate_result(
    result_path: &Path,
    output_dir: &Path,
) -> Result<StandardJobResultDto, PluginError> {
    if !result_path.exists() {
        return Err(PluginError::ArtifactMissing(format!(
            "Result file not found: {:?}",
            result_path
        )));
    }

    let raw = std::fs::read_to_string(result_path)?;
    let dto = parse_result_json(&raw, result_path, output_dir)?;

    for (key, artifact) in &dto.artifacts {
        verify_artifact_sandbox(output_dir, key, &artifact.file_path)?;
    }

    Ok(dto)
}

// ---------------------------------------------------------------------------
// 7. HELPER — unique job ID generator
// ---------------------------------------------------------------------------

/// Generates a unique, time-stamped execution ID.
///
/// Format: `exec_<YYYYMMDD>_<uuid_v4_short>`
///
/// # Purity
/// Impure: reads the `Utc::now()` system clock and `Uuid::new_v4()` RNG.
pub(crate) fn generate_job_id() -> String {
    let date = Utc::now().format("%Y%m%d");
    let uid = &Uuid::new_v4().to_string()[..8];
    format!("exec_{}_{}", date, uid)
}

// ---------------------------------------------------------------------------
// 8. UNIT TESTS (Imperative Shell / I/O tests; pure tests live in `executor_core`)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
}
