//! # Domain 3: Async Execution, Progress & Output Resolution
//!
//! **Role**: Subprocess Supervisor & Isolation Engineer
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
//! Refer to: `src-tauri/src/plugin_manager/DOCS/03_DOMAIN_3_EXECUTION_SUPERVISOR.md`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::{CommandError, PluginError};
use super::models::{ExecutionPayload, ExecutionTarget, StandardJobResultDto, VerifiedArtifactDto};

// ---------------------------------------------------------------------------
// 2. DATA TRANSFER OBJECTS (DTOs)
// ---------------------------------------------------------------------------

/// Analysis request delivered by the Frontend or Modules 1 & 3.
///
/// Passed as the argument to `start_plugin_job`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartJobRequestDto {
    /// Unique plugin slug, e.g. `"rgb-vegetation-exg"`.
    pub plugin_id: String,
    /// Optional parent flight session / project ID.
    pub session_id: Option<String>,
    /// Absolute filesystem path to the source aerial RGB image.
    pub target_image_path: String,
    /// MIME type of the source image (`"image/jpeg"`, `"image/png"`, `"image/tiff"`).
    pub mime_type: String,
    /// Optional user-selected bounding geometry (RFC 7946 GeoJSON).
    pub aoi: Option<serde_json::Value>,
    /// User-configured parameter values corresponding to the plugin's `parameters.json`.
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Instant return handle from `start_plugin_job`.
///
/// Returned immediately (non-blocking) so the UI can track the job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHandleDto {
    /// Unique execution ID, e.g. `"exec_20260916_<uuid>"`.
    pub job_id: String,
    /// Plugin that was invoked.
    pub plugin_id: String,
    /// Initial state: `"queued"` or `"running"`.
    pub status: String,
}

/// Real-time progress event streamed from the plugin's stdout over Tauri IPC.
///
/// Emitted as: `app.emit("plugin://progress", JobProgress { ... })`
///
/// Plugins signal progress by writing to stdout:
/// ```text
/// PROGRESS: {"job_id":"...", "percent": 45, "stage": "evaluating_index"}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    /// Matches the `job_id` returned by `start_plugin_job`.
    pub job_id: String,
    /// Completion percentage: 0–100.
    pub percent: u8,
    /// Human-readable stage label, e.g. `"preprocessing"`, `"saving_mask"`.
    pub stage: String,
}

/// Current lifecycle state of a job — returned by `get_job_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "data")]
pub enum JobStatusDto {
    /// Queued, not yet spawned.
    Queued,
    /// Subprocess is running; streaming progress.
    Running { progress_pct: u8, stage: String },
    /// Subprocess exited successfully; result is ready.
    Completed { execution_time_ms: u64 },
    /// Subprocess exited with an error or produced invalid output.
    Failed { error: String },
    /// Job was killed by `abort_plugin_job`.
    Aborted,
}

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
#[tauri::command]
pub async fn start_plugin_job(
    request: StartJobRequestDto,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<JobHandleDto, CommandError> {
    // 1. Verify plugin is registered and enabled (Execution Gating)
    let target = {
        let registry = state.plugins.read().await;
        verify_plugin_executable(&request.plugin_id, &registry)?;
        let bundle = registry
            .plugins
            .get(&request.plugin_id)
            .ok_or_else(|| PluginError::PluginUnavailable(request.plugin_id.clone()))?;
        PluginExecutionTarget {
            plugin_dir: bundle.install_path.clone(),
            entrypoint: bundle.manifest.runtime.entrypoint.clone(),
            runtime_type: bundle.manifest.runtime.runtime_type.clone(),
            timeout_seconds: bundle
                .manifest
                .execution
                .as_ref()
                .map(|e| e.timeout_seconds)
                .unwrap_or(60),
        }
    };

    // 2. Verify target image exists on physical disk
    let target_img = PathBuf::from(&request.target_image_path);
    if !target_img.exists() {
        return Err(CommandError {
            code: "IMAGE_NOT_FOUND".to_string(),
            message: format!("Target image file not found: {:?}", target_img),
        });
    }

    // 3. Generate unique execution ID
    let job_id = generate_job_id();

    // 4. Create sandboxed output directory
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{}_", job_id))
        .tempdir()
        .map_err(|e| CommandError {
            code: "IO_ERROR".to_string(),
            message: format!("Failed to create sandboxed output directory: {}", e),
        })?;
    let output_dir = temp_dir.keep();

    // 5. Build canonical execution payload and write to payload.json
    let payload = build_execution_payload(&job_id, &output_dir, &request)?;
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

    // 6. Resolve executable and command configuration
    let mut cmd = resolve_executable(
        &target.plugin_dir,
        &target.entrypoint,
        &target.runtime_type,
        &payload_path,
        &result_path,
    )
    .await?;

    let child = cmd.spawn().map_err(|e| CommandError {
        code: "SPAWN_FAILED".to_string(),
        message: format!("Failed to spawn plugin subprocess: {}", e),
    })?;

    let child_pid = child.id();

    // 7. Spawn async supervisor background task
    let tracker_arc = state.jobs.clone();
    let app_handle = app.clone();
    let job_id_clone = job_id.clone();
    let timeout_secs = target.timeout_seconds;
    let result_path_clone = result_path.clone();
    let output_dir_clone = output_dir.clone();

    let supervisor_task = tokio::spawn(async move {
        let res = supervise_execution(
            child,
            job_id_clone.clone(),
            result_path_clone,
            output_dir_clone,
            timeout_secs,
            app_handle.clone(),
            tracker_arc.clone(),
        )
        .await;

        let mut tracker = tracker_arc.write().await;
        if let Some(record) = tracker.jobs.get_mut(&job_id_clone) {
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
                    use tauri::Emitter;
                    let _ = app_handle.emit("plugin://completed", &dto);
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    record.status = JobStatusDto::Failed {
                        error: err_msg.clone(),
                    };
                    use tauri::Emitter;
                    let _ = app_handle.emit(
                        "plugin://completed",
                        serde_json::json!({
                            "job_id": job_id_clone,
                            "status": "failure",
                            "error": err_msg,
                        }),
                    );
                }
            }
        }
    });

    // 8. Register job in ActiveJobTracker
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

    // 9. Return JobHandleDto immediately (non-blocking)
    Ok(JobHandleDto {
        job_id,
        plugin_id: request.plugin_id,
        status: "running".to_string(),
    })
}

/// Forcibly terminates an in-flight plugin job.
///
/// Provides user-cancellation capabilities exposed to the frontend via Tauri IPC (`invoke("abort_plugin_job", { jobId })`).
/// Invoked by Module 3 (Map Explorer task drawer) and Module 11 (Task Monitor) when a user cancels an active analysis,
/// ensuring runaway or stalled subprocesses are cleanly killed.
#[tauri::command]
pub async fn abort_plugin_job(
    job_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), CommandError> {
    let mut tracker = state.jobs.write().await;
    let record = tracker
        .jobs
        .get_mut(&job_id)
        .ok_or_else(|| PluginError::JobNotFound(job_id.clone()))?;

    // Abort async supervisor task
    if let Some(ref handle) = record.abort_handle {
        handle.abort();
    }

    // Forcibly terminate child OS process tree
    if let Some(pid) = record.child_pid {
        kill_process_tree(pid);
    }

    // Clean up partial sandboxed output files
    if let Some(ref dir) = record.output_dir {
        let _ = std::fs::remove_dir_all(dir);
    }

    record.status = JobStatusDto::Aborted;
    Ok(())
}

/// Polls the current lifecycle state of a job.
///
/// Exposes read-only job status polling to the frontend via Tauri IPC (`invoke("get_job_status", { jobId })`).
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
// 5. PURE FUNCTIONS (pub(crate) — no side effects, fully unit-testable)
// ---------------------------------------------------------------------------

/// Execution gating predicate verifying that a plugin is both registered in the registry and currently enabled.
pub(crate) fn verify_plugin_executable(
    plugin_id: &str,
    registry: &crate::plugin_manager::registry::PluginRegistry,
) -> Result<(), PluginError> {
    let bundle = registry
        .plugins
        .get(plugin_id)
        .ok_or_else(|| PluginError::PluginUnavailable(plugin_id.to_string()))?;

    if !bundle.enabled {
        return Err(PluginError::PluginUnavailable(plugin_id.to_string()));
    }

    Ok(())
}

/// Constructs the `ExecutionPayload` struct for a given job request.
///
/// **Pure function** — no filesystem I/O, no subprocess interaction.
/// Caller is responsible for serializing and writing the result to disk.
pub(crate) fn build_execution_payload(
    job_id: &str,
    output_dir: &Path,
    req: &StartJobRequestDto,
) -> Result<ExecutionPayload, PluginError> {
    let target = ExecutionTarget {
        granularity: "single_image".to_string(),
        image_path: req.target_image_path.clone(),
        mime_type: req.mime_type.clone(),
        metadata: None,
    };

    Ok(ExecutionPayload {
        execution_id: job_id.to_string(),
        session_id: req.session_id.clone(),
        output_dir: output_dir.to_string_lossy().into_owned(),
        target,
        aoi: req.aoi.clone(),
        parameters: req.parameters.clone(),
    })
}

/// Extracts a `JobProgress` update from a single line of plugin stdout.
///
/// **Pure function** — only string parsing, zero I/O.
///
/// Plugins signal progress by writing to stdout:
/// ```text
/// PROGRESS: {"job_id":"exec_...", "percent": 45, "stage": "evaluating_index"}
/// ```
pub(crate) fn parse_progress_line(line: &str) -> Option<JobProgress> {
    let trimmed = line.trim();
    let json_slice = trimmed
        .strip_prefix("PROGRESS:")
        .or_else(|| trimmed.strip_prefix("progress:"))?;
    serde_json::from_str::<JobProgress>(json_slice.trim()).ok()
}

/// Decides whether a newly parsed progress update should be emitted.
///
/// **Pure function** — no I/O, no locking, no clock reads of its own
/// (`now` is passed in). Emits when the `(percent, stage)` pair differs
/// from the last emitted update, or when at least `min_interval` has
/// elapsed since the last emission — whichever comes first. This keeps a
/// chatty plugin from flooding the shared `jobs_tracker` write-lock and
/// the Tauri IPC channel with redundant or high-frequency progress lines.
pub(crate) fn should_emit_progress(
    last: Option<(u8, &str, std::time::Instant)>,
    next: &JobProgress,
    now: std::time::Instant,
    min_interval: std::time::Duration,
) -> bool {
    match last {
        None => true,
        Some((last_pct, last_stage, last_at)) => {
            (last_pct, last_stage) != (next.percent, next.stage.as_str())
                || now.duration_since(last_at) >= min_interval
        }
    }
}

// ---------------------------------------------------------------------------
// 6. INTERNAL HELPERS (pub(crate) — I/O, called from the background task)
// ---------------------------------------------------------------------------

/// Resolved plugin execution metadata discovered from manifest.
#[derive(Debug, Clone)]
pub(crate) struct PluginExecutionTarget {
    pub plugin_dir: PathBuf,
    pub entrypoint: String,
    pub runtime_type: String,
    pub timeout_seconds: u64,
}

/// Locates a plugin's directory and manifest by `plugin_id`.
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
        let direct_dir = root.join(plugin_id);
        if direct_dir.is_dir() {
            let manifest_path = direct_dir.join("manifest.json");
            if manifest_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        let id = val["metadata"]["id"].as_str().unwrap_or_default();
                        if id == plugin_id {
                            let entrypoint = val["runtime"]["entrypoint"]
                                .as_str()
                                .unwrap_or("main.py")
                                .to_string();
                            let runtime_type = val["runtime"]["type"]
                                .as_str()
                                .unwrap_or("python")
                                .to_string();
                            let timeout_seconds =
                                val["execution"]["timeout_seconds"].as_u64().unwrap_or(60);

                            return Ok(PluginExecutionTarget {
                                plugin_dir: direct_dir,
                                entrypoint,
                                runtime_type,
                                timeout_seconds,
                            });
                        }
                    }
                }
            }
        }

        // Search subdirectories for matching manifest ID
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("manifest.json");
                    if manifest_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                let id = val["metadata"]["id"].as_str().unwrap_or_default();
                                if id == plugin_id {
                                    let entrypoint = val["runtime"]["entrypoint"]
                                        .as_str()
                                        .unwrap_or("main.py")
                                        .to_string();
                                    let runtime_type = val["runtime"]["type"]
                                        .as_str()
                                        .unwrap_or("python")
                                        .to_string();
                                    let timeout_seconds =
                                        val["execution"]["timeout_seconds"].as_u64().unwrap_or(60);

                                    return Ok(PluginExecutionTarget {
                                        plugin_dir: path,
                                        entrypoint,
                                        runtime_type,
                                        timeout_seconds,
                                    });
                                }
                            }
                        }
                    }
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
pub(crate) fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &format!("-{}", pid)])
            .output();
    }
}

/// Prepares the cross-platform Tokio process command for the plugin entrypoint.
pub(crate) async fn resolve_executable(
    plugin_dir: &Path,
    entrypoint: &str,
    runtime_type: &str,
    payload_path: &Path,
    result_path: &Path,
) -> Result<tokio::process::Command, PluginError> {
    match runtime_type {
        "python" => {
            // Check for local virtual environment Python
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

            let mut cmd = tokio::process::Command::new(interpreter);
            cmd.arg(&script_path)
                .arg("--input")
                .arg(payload_path)
                .arg("--output")
                .arg(result_path)
                .current_dir(plugin_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            #[cfg(unix)]
            {
                // Make the child its own process group leader so
                // kill_process_tree() can signal the whole group (covers
                // grandchild workers) instead of only this direct PID.
                cmd.process_group(0);
            }

            Ok(cmd)
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

            let mut cmd = tokio::process::Command::new(&bin_path);
            cmd.arg("--input")
                .arg(payload_path)
                .arg("--output")
                .arg(result_path)
                .current_dir(plugin_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            #[cfg(unix)]
            {
                // See the python branch above: new process group so the
                // full tree can be signaled on abort/timeout.
                cmd.process_group(0);
            }

            Ok(cmd)
        }
        other => Err(PluginError::SpawnFailed(format!(
            "Unsupported runtime type: '{}'",
            other
        ))),
    }
}

/// Supervises an already-spawned child process to completion while streaming progress events.
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
                kill_process_tree(pid);
            }
            Err(PluginError::Timeout(timeout_secs))
        }
        Ok(Err(io_err)) => Err(PluginError::Io(io_err.to_string())),
        Ok(Ok(status)) => {
            let stderr_output = stderr_handle.await.unwrap_or_default();
            if status.success() {
                read_and_validate_result(&result_path, &output_dir)
            } else if result_path.exists() {
                // Plugin may have written a structured failure execution_result.json before exit 1
                read_and_validate_result(&result_path, &output_dir)
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

/// Reads the plugin output file and validates it against the execution result schema.
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
    let val: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        PluginError::Serialization(format!(
            "Failed to parse result artifact at {:?}: {}",
            result_path, e
        ))
    })?;

    let job_id = val["execution_id"].as_str().unwrap_or_default().to_string();
    let status = val["status"].as_str().unwrap_or("failure").to_string();
    let execution_time_ms = val["execution_time_ms"].as_u64().unwrap_or(0);

    let metrics = val["metrics"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    let mut artifacts: HashMap<String, VerifiedArtifactDto> = HashMap::new();
    if let Some(arts) = val["artifacts"].as_object() {
        for (key, art) in arts {
            let fp_str = art["file_path"].as_str().unwrap_or_default();
            let fp = if Path::new(fp_str).is_absolute() {
                PathBuf::from(fp_str)
            } else {
                output_dir.join(fp_str)
            };

            if !fp.exists() {
                return Err(PluginError::ArtifactMissing(format!(
                    "Artifact '{}' file not found at: {:?}",
                    key, fp
                )));
            }

            // Sandbox security check: ensure artifact resides strictly within output_dir
            match (output_dir.canonicalize(), fp.canonicalize()) {
                (Ok(canonical_dir), Ok(canonical_art)) => {
                    if !canonical_art.starts_with(&canonical_dir) {
                        return Err(PluginError::SecurityViolation(format!(
                            "Artifact '{}' escapes sandboxed output directory: {:?}",
                            key, fp
                        )));
                    }
                }
                _ => {
                    return Err(PluginError::SecurityViolation(format!(
                        "Failed to verify sandbox canonical boundaries for artifact '{}': {:?}",
                        key, fp
                    )));
                }
            }

            let format_str = art["format"].as_str().unwrap_or_default().to_string();
            let bounds = art["bounds"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|b| b.as_f64()).collect::<Vec<f64>>());

            artifacts.insert(
                key.clone(),
                VerifiedArtifactDto {
                    file_path: fp.to_string_lossy().into_owned(),
                    format: format_str,
                    bounds,
                },
            );
        }
    }

    let error = val["error"].as_str().map(|s| s.to_string());

    Ok(StandardJobResultDto {
        job_id,
        execution_time_ms,
        status,
        metrics,
        artifacts,
        error,
    })
}

// ---------------------------------------------------------------------------
// 7. HELPER — unique job ID generator
// ---------------------------------------------------------------------------

/// Generates a unique, time-stamped execution ID.
///
/// Format: `exec_<YYYYMMDD>_<uuid_v4_short>`
///
/// **Pure function** — only uses system time and UUID RNG.
pub(crate) fn generate_job_id() -> String {
    let date = Utc::now().format("%Y%m%d");
    let uid = &Uuid::new_v4().to_string()[..8];
    format!("exec_{}_{}", date, uid)
}

// ---------------------------------------------------------------------------
// 8. UNIT TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_job_id_format() {
        let id = generate_job_id();
        assert!(
            id.starts_with("exec_"),
            "job_id must start with 'exec_': {id}"
        );
        let parts: Vec<&str> = id.splitn(3, '_').collect();
        assert_eq!(parts.len(), 3, "job_id must have 3 segments: {id}");
        assert_eq!(parts[1].len(), 8, "date segment must be 8 digits: {id}");
    }

    #[test]
    fn test_parse_progress_line_valid() {
        let line = r#"PROGRESS: {"job_id":"exec_20260916_abc","percent":45,"stage":"evaluating"}"#;
        let result = parse_progress_line(line);
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.percent, 45);
        assert_eq!(p.stage, "evaluating");
        assert_eq!(p.job_id, "exec_20260916_abc");
    }

    #[test]
    fn test_parse_progress_line_lowercase() {
        let line = r#"progress: {"job_id":"exec_20260916_abc","percent":60,"stage":"masking"}"#;
        let result = parse_progress_line(line);
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.percent, 60);
        assert_eq!(p.stage, "masking");
    }

    #[test]
    fn test_parse_progress_line_non_progress() {
        let line = "INFO: Loading image from /tmp/drone_001.jpg";
        assert!(parse_progress_line(line).is_none());
    }

    #[test]
    fn test_parse_progress_line_malformed_json() {
        let line = "PROGRESS: {broken json}";
        assert!(parse_progress_line(line).is_none());
    }

    #[test]
    fn test_should_emit_progress_first_update_always_emits() {
        let next = JobProgress {
            job_id: "exec_test".to_string(),
            percent: 10,
            stage: "preprocessing".to_string(),
        };
        assert!(should_emit_progress(
            None,
            &next,
            std::time::Instant::now(),
            std::time::Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_should_emit_progress_suppresses_duplicate_within_interval() {
        let last_at = std::time::Instant::now();
        let same = JobProgress {
            job_id: "exec_test".to_string(),
            percent: 45,
            stage: "evaluating".to_string(),
        };
        let now = last_at; // zero elapsed time, well within the interval
        assert!(!should_emit_progress(
            Some((45, "evaluating", last_at)),
            &same,
            now,
            std::time::Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_should_emit_progress_emits_on_changed_stage_even_within_interval() {
        let last_at = std::time::Instant::now();
        let changed = JobProgress {
            job_id: "exec_test".to_string(),
            percent: 46,
            stage: "saving_mask".to_string(),
        };
        assert!(should_emit_progress(
            Some((45, "evaluating", last_at)),
            &changed,
            last_at,
            std::time::Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_should_emit_progress_emits_after_interval_elapses_even_if_unchanged() {
        let last_at = std::time::Instant::now();
        let same = JobProgress {
            job_id: "exec_test".to_string(),
            percent: 45,
            stage: "evaluating".to_string(),
        };
        let later = last_at + std::time::Duration::from_millis(250);
        assert!(should_emit_progress(
            Some((45, "evaluating", last_at)),
            &same,
            later,
            std::time::Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_build_execution_payload_conforms_to_schema() {
        let req = StartJobRequestDto {
            plugin_id: "rgb-vegetation-exg".to_string(),
            session_id: Some("session_001".to_string()),
            target_image_path: "/data/images/drone_01.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            aoi: None,
            parameters: HashMap::from([("threshold".to_string(), serde_json::json!(0.35))]),
        };
        let output_dir = Path::new("/tmp/test_output");
        let payload = build_execution_payload("exec_test_123", output_dir, &req).unwrap();

        assert_eq!(payload.execution_id, "exec_test_123");
        assert_eq!(payload.session_id, Some("session_001".to_string()));
        assert_eq!(payload.target.granularity, "single_image");
        assert_eq!(payload.target.image_path, "/data/images/drone_01.jpg");
        assert_eq!(payload.target.mime_type, "image/jpeg");
        assert_eq!(
            payload.parameters.get("threshold"),
            Some(&serde_json::json!(0.35))
        );
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
    fn test_execution_gating_unregistered_plugin_rejected() {
        let registry = crate::plugin_manager::registry::PluginRegistry::new();
        let err = verify_plugin_executable("nonexistent-plugin", &registry).unwrap_err();
        assert_eq!(err.code(), "PLUGIN_UNAVAILABLE");
    }

    #[test]
    fn test_execution_gating_disabled_plugin_rejected() {
        let mut registry = crate::plugin_manager::registry::PluginRegistry::new();
        let bundle = crate::plugin_manager::models::PluginBundle {
            install_path: PathBuf::from("/mock/test"),
            manifest: crate::plugin_manager::models::PluginManifest {
                schema: None,
                manifest_version: 1,
                metadata: crate::plugin_manager::models::PluginMetadata {
                    id: "disabled-plugin".to_string(),
                    name: "Disabled".to_string(),
                    version: "1.0.0".to_string(),
                    author: None,
                    license: None,
                    description: "Desc".to_string(),
                    category: None,
                    tags: vec![],
                    min_core_version: None,
                },
                runtime: crate::plugin_manager::models::PluginRuntime {
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
            inputs_spec: crate::plugin_manager::models::InputsSpec {
                granularity: "single_image".to_string(),
                supported_mime_types: vec!["image/jpeg".to_string()],
                requires_metadata: vec![],
                optional_metadata: vec![],
                supports_aoi: false,
                aoi_format: None,
            },
            inputs_raw: serde_json::json!({}),
            parameters_raw: serde_json::json!({}),
            outputs_raw: serde_json::json!({}),
            enabled: false,
            source: "builtin".to_string(),
            health_status: "unknown".to_string(),
        };
        registry.register(bundle);

        let err = verify_plugin_executable("disabled-plugin", &registry).unwrap_err();
        assert_eq!(err.code(), "PLUGIN_UNAVAILABLE");
    }

    #[test]
    fn test_execution_gating_enabled_plugin_accepted() {
        let mut registry = crate::plugin_manager::registry::PluginRegistry::new();
        let bundle = crate::plugin_manager::models::PluginBundle {
            install_path: PathBuf::from("/mock/test"),
            manifest: crate::plugin_manager::models::PluginManifest {
                schema: None,
                manifest_version: 1,
                metadata: crate::plugin_manager::models::PluginMetadata {
                    id: "enabled-plugin".to_string(),
                    name: "Enabled".to_string(),
                    version: "1.0.0".to_string(),
                    author: None,
                    license: None,
                    description: "Desc".to_string(),
                    category: None,
                    tags: vec![],
                    min_core_version: None,
                },
                runtime: crate::plugin_manager::models::PluginRuntime {
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
            inputs_spec: crate::plugin_manager::models::InputsSpec {
                granularity: "single_image".to_string(),
                supported_mime_types: vec!["image/jpeg".to_string()],
                requires_metadata: vec![],
                optional_metadata: vec![],
                supports_aoi: false,
                aoi_format: None,
            },
            inputs_raw: serde_json::json!({}),
            parameters_raw: serde_json::json!({}),
            outputs_raw: serde_json::json!({}),
            enabled: true,
            source: "builtin".to_string(),
            health_status: "unknown".to_string(),
        };
        registry.register(bundle);

        let res = verify_plugin_executable("enabled-plugin", &registry);
        assert!(res.is_ok());
    }
}
