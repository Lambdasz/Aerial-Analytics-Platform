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
use thiserror::Error;
use uuid::Uuid;

use super::error::CommandError;

// ---------------------------------------------------------------------------
// 1. ERROR TYPE
// ---------------------------------------------------------------------------

/// Domain-level error for the execution supervisor.
/// All public functions return `Result<_, ExecutorError>` — zero panics allowed.
#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Job '{0}' not found in active tracker")]
    JobNotFound(String),

    #[error("Plugin '{0}' is disabled or not registered")]
    PluginUnavailable(String),

    #[error("Execution timed out after {0} seconds")]
    Timeout(u64),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Job '{0}' is not in Completed state")]
    NotCompleted(String),

    #[error("Artifact file not found on disk: {0}")]
    ArtifactMissing(String),

    #[error("Process spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),
}

impl ExecutorError {
    /// String error code for client-side programmatic matching in TypeScript
    pub fn code(&self) -> &'static str {
        match self {
            Self::JobNotFound(_) => "JOB_NOT_FOUND",
            Self::PluginUnavailable(_) => "PLUGIN_UNAVAILABLE",
            Self::Timeout(_) => "EXECUTION_TIMEOUT",
            Self::Io(_) => "IO_ERROR",
            Self::Serde(_) => "SERIALIZATION_ERROR",
            Self::NotCompleted(_) => "JOB_NOT_COMPLETED",
            Self::ArtifactMissing(_) => "ARTIFACT_MISSING",
            Self::SpawnFailed(_) => "SPAWN_FAILED",
            Self::SecurityViolation(_) => "SECURITY_VIOLATION",
        }
    }
}

impl From<ExecutorError> for CommandError {
    fn from(err: ExecutorError) -> Self {
        CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}

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

/// A single verified artifact produced by the plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifiedArtifactDto {
    /// Absolute path to the file on physical disk (already verified to exist).
    pub file_path: String,
    /// MIME type: `"image/png"`, `"application/geo+json"`, `"image/tiff"`, `"text/csv"`.
    pub format: String,
    /// Georeferenced bounding box `[min_lon, min_lat, max_lon, max_lat]` for GIS projection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Vec<f64>>,
}

/// Full analytical output — delivered to Modules 3, 5, 6, 7, 9, 10, 11.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StandardJobResultDto {
    /// Matches the `job_id` of the originating request.
    pub job_id: String,
    /// Wall-clock duration of the subprocess in milliseconds.
    pub execution_time_ms: u64,
    /// Exit status: `"success"` | `"warning"` | `"failure"`.
    pub status: String,
    /// Scalar metrics for dashboards (e.g. `{"vegetation_coverage_pct": 68.4}`).
    pub metrics: HashMap<String, serde_json::Value>,
    /// Verified artifact descriptors with layer visualization hints.
    pub artifacts: HashMap<String, VerifiedArtifactDto>,
    /// Diagnostic message if `status` is `"failure"` or `"warning"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Internal payload written to disk and passed to the plugin via `--input`.
///
/// Matches the `execution_payload.schema.json` canonical schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionPayload {
    pub execution_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Absolute path to the sandboxed output directory.
    pub output_dir: String,
    pub target: ExecutionTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aoi: Option<serde_json::Value>,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Target image descriptor within an `ExecutionPayload`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionTarget {
    /// Matches `inputs.json` granularity: `"single_image"`, `"batch"`, etc.
    pub granularity: String,
    /// Absolute filesystem path to the source image.
    pub image_path: String,
    /// MIME type of the image.
    pub mime_type: String,
    /// Optional decoded metadata (resolution, GPS, altitude, timestamp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// 3. ACTIVE JOB TRACKER (Shared State)
// ---------------------------------------------------------------------------

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
    // 1. Verify target image exists on physical disk
    let target_img = PathBuf::from(&request.target_image_path);
    if !target_img.exists() {
        return Err(CommandError {
            code: "IMAGE_NOT_FOUND".to_string(),
            message: format!("Target image file not found: {:?}", target_img),
        });
    }

    // 2. Locate plugin entrypoint and execution configuration
    let target = find_plugin_info(&request.plugin_id)?;

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
    let output_dir = temp_dir.into_path();

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
    let record = tracker.jobs.get_mut(&job_id).ok_or_else(|| CommandError {
        code: "JOB_NOT_FOUND".to_string(),
        message: format!("Job '{}' not found in active tracker", job_id),
    })?;

    // Abort async supervisor task
    if let Some(ref handle) = record.abort_handle {
        handle.abort();
    }

    // Forcibly terminate child OS process tree
    if let Some(pid) = record.child_pid {
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
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
    let record = tracker.jobs.get(&job_id).ok_or_else(|| CommandError {
        code: "JOB_NOT_FOUND".to_string(),
        message: format!("Job '{}' not found in active tracker", job_id),
    })?;
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
    let record = tracker.jobs.get(&job_id).ok_or_else(|| CommandError {
        code: "JOB_NOT_FOUND".to_string(),
        message: format!("Job '{}' not found in active tracker", job_id),
    })?;

    match &record.status {
        JobStatusDto::Completed { .. } => record.result.clone().ok_or_else(|| CommandError {
            code: "RESULT_MISSING".to_string(),
            message: format!("Job '{}' is completed but result is missing", job_id),
        }),
        _ => Err(CommandError {
            code: "JOB_NOT_COMPLETED".to_string(),
            message: format!("Job '{}' has not completed successfully", job_id),
        }),
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

/// Constructs the `ExecutionPayload` struct for a given job request.
///
/// **Pure function** — no filesystem I/O, no subprocess interaction.
/// Caller is responsible for serializing and writing the result to disk.
pub(crate) fn build_execution_payload(
    job_id: &str,
    output_dir: &Path,
    req: &StartJobRequestDto,
) -> Result<ExecutionPayload, ExecutorError> {
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
    let json_slice = if let Some(rest) = trimmed.strip_prefix("PROGRESS:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("progress:") {
        rest
    } else {
        return None;
    };
    serde_json::from_str::<JobProgress>(json_slice.trim()).ok()
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
pub(crate) fn find_plugin_info(plugin_id: &str) -> Result<PluginExecutionTarget, ExecutorError> {
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

    Err(ExecutorError::PluginUnavailable(plugin_id.to_string()))
}

/// Prepares the cross-platform Tokio process command for the plugin entrypoint.
pub(crate) async fn resolve_executable(
    plugin_dir: &Path,
    entrypoint: &str,
    runtime_type: &str,
    payload_path: &Path,
    result_path: &Path,
) -> Result<tokio::process::Command, ExecutorError> {
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
                return Err(ExecutorError::SpawnFailed(format!(
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
                return Err(ExecutorError::SpawnFailed(format!(
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

            Ok(cmd)
        }
        other => Err(ExecutorError::SpawnFailed(format!(
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
) -> Result<StandardJobResultDto, ExecutorError> {
    use tokio::io::AsyncBufReadExt;
    use tokio::time::{timeout, Duration};

    let child_pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ExecutorError::SpawnFailed("Failed to capture stdout pipe".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ExecutorError::SpawnFailed("Failed to capture stderr pipe".into()))?;

    // Drain stderr in background to prevent buffer deadlock
    let stderr_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = tokio::io::BufReader::new(stderr);
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).into_owned()
    });

    let supervise_fut = async {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(progress) = parse_progress_line(&line) {
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
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/T", "/PID", &pid.to_string()])
                        .output();
                }
                #[cfg(unix)]
                {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                }
            }
            Err(ExecutorError::Timeout(timeout_secs))
        }
        Ok(Err(io_err)) => Err(ExecutorError::Io(io_err)),
        Ok(Ok(status)) => {
            let stderr_output = stderr_handle.await.unwrap_or_default();
            if status.success() {
                read_and_validate_result(&result_path, &output_dir)
            } else if result_path.exists() {
                // Plugin may have written a structured failure execution_result.json before exit 1
                read_and_validate_result(&result_path, &output_dir)
            } else {
                Err(ExecutorError::SpawnFailed(format!(
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
) -> Result<StandardJobResultDto, ExecutorError> {
    if !result_path.exists() {
        return Err(ExecutorError::ArtifactMissing(format!(
            "Result file not found: {:?}",
            result_path
        )));
    }

    let raw = std::fs::read_to_string(result_path)?;
    let val: serde_json::Value = serde_json::from_str(&raw)?;

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
                return Err(ExecutorError::ArtifactMissing(format!(
                    "Artifact '{}' file not found at: {:?}",
                    key, fp
                )));
            }

            // Sandbox security check: ensure artifact resides strictly within output_dir
            match (output_dir.canonicalize(), fp.canonicalize()) {
                (Ok(canonical_dir), Ok(canonical_art)) => {
                    if !canonical_art.starts_with(&canonical_dir) {
                        return Err(ExecutorError::SecurityViolation(format!(
                            "Artifact '{}' escapes sandboxed output directory: {:?}",
                            key, fp
                        )));
                    }
                }
                _ => {
                    return Err(ExecutorError::SecurityViolation(format!(
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
}
