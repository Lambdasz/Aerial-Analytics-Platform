//! # Domain 3 (Pure Core): Execution DTOs, Gating, Payload & Result Parsing
//!
//! **Role**: Pure functional core for plugin execution DTOs, execution gating,
//! payload construction, progress line parsing/throttling, cancellation
//! kill-target guards, and JSON result/artifact parsing.
//!
//! ## Purity Boundary
//!
//! This module is strictly pure. It MUST NOT import or invoke any side-effectful
//! primitives (`std::fs`, `tokio`, `rusqlite`, `chrono::Utc::now()`,
//! `uuid::Uuid::new_v4()`, `tauri`, `crate::AppState`, `libc`, or `eprintln!`).
//! All subprocess spawning, filesystem I/O, IPC emission, process signaling,
//! and Tauri state synchronization live in [`super::executor`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::error::PluginError;
use super::models::{ExecutionPayload, ExecutionTarget, StandardJobResultDto, VerifiedArtifactDto};
use super::registry_core::PluginRegistry;

// ---------------------------------------------------------------------------
// 1. DATA TRANSFER OBJECTS & EXECUTION TARGET / GUARD TYPES
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
    /// Optional decoded metadata (resolution, GPS, altitude, timestamp) conforming to `execution_payload.schema.json`.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
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

/// Resolved plugin execution metadata discovered from manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginExecutionTarget {
    pub plugin_dir: PathBuf,
    pub entrypoint: String,
    pub runtime_type: String,
    pub timeout_seconds: u64,
}

/// What a cancellation is allowed to signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KillTarget {
    /// Signal only the process itself.
    Process,
    /// Signal the whole process group (the child is its leader).
    Group,
    /// Do not signal anything; the reason is logged.
    Refuse(&'static str),
}

// ---------------------------------------------------------------------------
// 2. PURE EXECUTION HELPERS
// ---------------------------------------------------------------------------

/// Execution gating predicate verifying that a plugin is both registered in the
/// registry and currently enabled.
///
/// # Purity
/// Pure deterministic lookup over `&PluginRegistry`; zero I/O, zero mutation.
pub(crate) fn verify_plugin_executable(
    plugin_id: &str,
    registry: &PluginRegistry,
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

/// Verifies plugin eligibility and extracts its [`PluginExecutionTarget`]
/// metadata from the registry.
///
/// # Purity
/// Pure deterministic projection over `&PluginRegistry`; zero I/O, zero mutation.
pub(crate) fn extract_execution_target(
    plugin_id: &str,
    registry: &PluginRegistry,
) -> Result<PluginExecutionTarget, PluginError> {
    verify_plugin_executable(plugin_id, registry)?;
    let bundle = registry
        .plugins
        .get(plugin_id)
        .ok_or_else(|| PluginError::PluginUnavailable(plugin_id.to_string()))?;
    Ok(PluginExecutionTarget {
        plugin_dir: bundle.install_path.clone(),
        entrypoint: bundle.manifest.runtime.entrypoint.clone(),
        runtime_type: bundle.manifest.runtime.runtime_type.clone(),
        timeout_seconds: bundle
            .manifest
            .execution
            .as_ref()
            .map(|e| e.timeout_seconds)
            .unwrap_or(60),
    })
}

/// Constructs the `ExecutionPayload` struct for a given job request.
///
/// Caller is responsible for serializing and writing the result to disk.
///
/// # Purity
/// Pure deterministic transformation over borrowed inputs; zero I/O, zero side effects.
pub(crate) fn build_execution_payload(
    job_id: &str,
    output_dir: &Path,
    req: &StartJobRequestDto,
) -> Result<ExecutionPayload, PluginError> {
    let target = ExecutionTarget {
        granularity: "single_image".to_string(),
        image_path: req.target_image_path.clone(),
        mime_type: req.mime_type.clone(),
        metadata: req.metadata.clone(),
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
/// Plugins signal progress by writing to stdout:
/// ```text
/// PROGRESS: {"job_id":"exec_...", "percent": 45, "stage": "evaluating_index"}
/// ```
///
/// # Purity
/// Pure string parsing; zero I/O, zero side effects.
pub(crate) fn parse_progress_line(line: &str) -> Option<JobProgress> {
    let trimmed = line.trim();
    let json_slice = trimmed
        .strip_prefix("PROGRESS:")
        .or_else(|| trimmed.strip_prefix("progress:"))?;
    serde_json::from_str::<JobProgress>(json_slice.trim()).ok()
}

/// Decides whether a newly parsed progress update should be emitted.
///
/// Emits when the `(percent, stage)` pair differs from the last emitted update,
/// or when at least `min_interval` has elapsed since the last emission —
/// whichever comes first. This keeps a chatty plugin from flooding the shared
/// `jobs_tracker` write-lock and the Tauri IPC channel with redundant or
/// high-frequency progress lines.
///
/// # Purity
/// Pure deterministic predicate over explicit timestamps (`now` is passed in);
/// zero I/O, zero locking, zero clock reads.
pub(crate) fn should_emit_progress(
    last: Option<(u8, &str, Instant)>,
    next: &JobProgress,
    now: Instant,
    min_interval: Duration,
) -> bool {
    match last {
        None => true,
        Some((last_pct, last_stage, last_at)) => {
            (last_pct, last_stage) != (next.percent, next.stage.as_str())
                || now.duration_since(last_at) >= min_interval
        }
    }
}

/// Decides how a cancellation may signal `pid`.
///
/// Guards the cases that can take down more than the plugin: PID 0 and 1, our
/// own process, and a process group that is not actually the child's.
///
/// # Purity
/// Pure deterministic decision function over integer PIDs; zero I/O or OS signal calls.
///
/// # Complexity
/// Time and Space: `O(1)`.
pub(crate) fn kill_target(pid: u32, group_of_pid: Option<u32>, own_pid: u32) -> KillTarget {
    if pid <= 1 {
        return KillTarget::Refuse("reserved pid");
    }
    if pid == own_pid {
        return KillTarget::Refuse("would signal the app itself");
    }
    match group_of_pid {
        Some(group) if group == pid => KillTarget::Group,
        _ => KillTarget::Process,
    }
}

// ---------------------------------------------------------------------------
// 3. PURE JSON RESULT & PARALLEL ARTIFACT PARSING
// ---------------------------------------------------------------------------

/// Parses a plugin's raw `result.json` payload into a [`StandardJobResultDto`],
/// resolving relative artifact paths against `output_dir` cleanly via lazy iteration.
///
/// Disk existence and canonical sandbox boundary checks remain in the
/// imperative shell (`executor.rs`).
///
/// # Purity
/// Pure deterministic JSON parsing and lazy artifact path resolution;
/// zero filesystem I/O, zero clock reads, zero side effects.
///
/// # Complexity
/// Time and Space: `O(s + m + a)` in raw JSON length `s`, metric entries `m`, and artifact entries `a`.
pub(crate) fn parse_result_json(
    raw_json: &str,
    result_path: &Path,
    output_dir: &Path,
) -> Result<StandardJobResultDto, PluginError> {
    let val: serde_json::Value = serde_json::from_str(raw_json).map_err(|e| {
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

    let artifacts: HashMap<String, VerifiedArtifactDto> = val["artifacts"]
        .as_object()
        .map(|arts| {
            arts.iter()
                .map(|(key, art)| {
                    let fp_str = art["file_path"].as_str().unwrap_or_default();
                    let fp = if Path::new(fp_str).is_absolute() {
                        PathBuf::from(fp_str)
                    } else {
                        output_dir.join(fp_str)
                    };
                    let format_str = art["format"].as_str().unwrap_or_default().to_string();
                    let bounds = art["bounds"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|b| b.as_f64()).collect::<Vec<f64>>());

                    (
                        key.clone(),
                        VerifiedArtifactDto {
                            file_path: fp.to_string_lossy().into_owned(),
                            format: format_str,
                            bounds,
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();

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
// 4. PURE UNIT TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod kill_guard_tests {
    use super::{kill_target, KillTarget};

    #[test]
    fn refuses_reserved_and_own_pids() {
        assert_eq!(
            kill_target(0, Some(0), 4242),
            KillTarget::Refuse("reserved pid")
        );
        assert_eq!(
            kill_target(1, Some(1), 4242),
            KillTarget::Refuse("reserved pid")
        );
        assert_eq!(
            kill_target(4242, Some(4242), 4242),
            KillTarget::Refuse("would signal the app itself")
        );
    }

    #[test]
    fn signals_group_only_when_child_is_leader() {
        assert_eq!(kill_target(1234, Some(1234), 4242), KillTarget::Group);
        assert_eq!(kill_target(1234, Some(77), 4242), KillTarget::Process);
        assert_eq!(kill_target(1234, None, 4242), KillTarget::Process);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::{
        InputsSpec, PluginBundle, PluginExecutionConfig, PluginManifest, PluginMetadata,
        PluginRuntime,
    };

    fn make_bundle(id: &str, enabled: bool, timeout_seconds: Option<u64>) -> PluginBundle {
        PluginBundle {
            install_path: PathBuf::from("/mock/test"),
            manifest: PluginManifest {
                schema: None,
                manifest_version: 1,
                metadata: PluginMetadata {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: "1.0.0".to_string(),
                    author: None,
                    license: None,
                    description: "Desc".to_string(),
                    category: None,
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
                execution: timeout_seconds.map(|t| PluginExecutionConfig {
                    timeout_seconds: t,
                    provides_output_dir_arg: true,
                    concurrency: "sequential".to_string(),
                    gpu: None,
                }),
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
            parameters_raw: serde_json::json!({}),
            outputs_raw: serde_json::json!({}),
            enabled,
            source: "builtin".to_string(),
            health_status: "unknown".to_string(),
            custom_parameters: serde_json::json!({}),
        }
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
            Instant::now(),
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_should_emit_progress_suppresses_duplicate_within_interval() {
        let last_at = Instant::now();
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
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_should_emit_progress_emits_on_changed_stage_even_within_interval() {
        let last_at = Instant::now();
        let changed = JobProgress {
            job_id: "exec_test".to_string(),
            percent: 46,
            stage: "saving_mask".to_string(),
        };
        assert!(should_emit_progress(
            Some((45, "evaluating", last_at)),
            &changed,
            last_at,
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn test_should_emit_progress_emits_after_interval_elapses_even_if_unchanged() {
        let last_at = Instant::now();
        let same = JobProgress {
            job_id: "exec_test".to_string(),
            percent: 45,
            stage: "evaluating".to_string(),
        };
        let later = last_at + Duration::from_millis(250);
        assert!(should_emit_progress(
            Some((45, "evaluating", last_at)),
            &same,
            later,
            Duration::from_millis(200)
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
            metadata: Some(serde_json::json!({ "resolution": [4000, 3000] })),
        };
        let output_dir = Path::new("/tmp/test_output");
        let payload = build_execution_payload("exec_test_123", output_dir, &req).unwrap();

        assert_eq!(payload.execution_id, "exec_test_123");
        assert_eq!(payload.session_id, Some("session_001".to_string()));
        assert_eq!(payload.target.granularity, "single_image");
        assert_eq!(payload.target.image_path, "/data/images/drone_01.jpg");
        assert_eq!(payload.target.mime_type, "image/jpeg");
        assert_eq!(
            payload.target.metadata,
            Some(serde_json::json!({ "resolution": [4000, 3000] }))
        );
        assert_eq!(
            payload.parameters.get("threshold"),
            Some(&serde_json::json!(0.35))
        );
    }

    #[test]
    fn test_execution_gating_unregistered_plugin_rejected() {
        let registry = PluginRegistry::new();
        let err = verify_plugin_executable("nonexistent-plugin", &registry).unwrap_err();
        assert_eq!(err.code(), "PLUGIN_UNAVAILABLE");
    }

    #[test]
    fn test_execution_gating_disabled_plugin_rejected() {
        let mut registry = PluginRegistry::new();
        registry.register(make_bundle("disabled-plugin", false, None));

        let err = verify_plugin_executable("disabled-plugin", &registry).unwrap_err();
        assert_eq!(err.code(), "PLUGIN_UNAVAILABLE");
    }

    #[test]
    fn test_execution_gating_enabled_plugin_accepted() {
        let mut registry = PluginRegistry::new();
        registry.register(make_bundle("enabled-plugin", true, None));

        let res = verify_plugin_executable("enabled-plugin", &registry);
        assert!(res.is_ok());
    }

    #[test]
    fn test_extract_execution_target_returns_defaults_and_custom_timeout() {
        let mut registry = PluginRegistry::new();
        registry.register(make_bundle("default-timeout", true, None));
        registry.register(make_bundle("custom-timeout", true, Some(180)));

        let default_target = extract_execution_target("default-timeout", &registry).unwrap();
        assert_eq!(default_target.plugin_dir, PathBuf::from("/mock/test"));
        assert_eq!(default_target.entrypoint, "main.py");
        assert_eq!(default_target.runtime_type, "python");
        assert_eq!(default_target.timeout_seconds, 60);

        let custom_target = extract_execution_target("custom-timeout", &registry).unwrap();
        assert_eq!(custom_target.timeout_seconds, 180);
    }

    #[test]
    fn test_parse_result_json_single_and_multi_artifact_with_relative_and_absolute_paths() {
        let out_dir = Path::new("/tmp/sandbox/exec_1");
        let result_path = out_dir.join("result.json");
        let abs_overlay = PathBuf::from("/tmp/sandbox/exec_1/sub/overlay.geojson");
        let abs_overlay_str = abs_overlay.to_string_lossy().into_owned();

        let raw = serde_json::json!({
            "execution_id": "exec_1",
            "status": "success",
            "execution_time_ms": 245,
            "metrics": {
                "coverage_pct": 68.4,
                "tree_count": 42
            },
            "artifacts": {
                "mask": {
                    "file_path": "mask.png",
                    "format": "image/png",
                    "bounds": [116.85, -1.24, 116.86, -1.23]
                },
                "overlay": {
                    "file_path": abs_overlay_str,
                    "format": "application/geo+json",
                    "bounds": [116.85, "bad_coord", -1.23]
                }
            },
            "error": null
        })
        .to_string();

        let parsed = parse_result_json(&raw, &result_path, out_dir).unwrap();
        assert_eq!(parsed.job_id, "exec_1");
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.execution_time_ms, 245);
        assert_eq!(parsed.error, None);
        assert_eq!(
            parsed.metrics.get("coverage_pct"),
            Some(&serde_json::json!(68.4))
        );
        assert_eq!(
            parsed.metrics.get("tree_count"),
            Some(&serde_json::json!(42))
        );
        assert_eq!(parsed.artifacts.len(), 2);

        let mask = parsed.artifacts.get("mask").unwrap();
        assert_eq!(
            mask.file_path,
            out_dir.join("mask.png").to_string_lossy().as_ref()
        );
        assert_eq!(mask.format, "image/png");
        assert_eq!(mask.bounds, Some(vec![116.85, -1.24, 116.86, -1.23]));

        let overlay = parsed.artifacts.get("overlay").unwrap();
        assert_eq!(overlay.file_path, abs_overlay.to_string_lossy().as_ref());
        assert_eq!(overlay.format, "application/geo+json");
        assert_eq!(overlay.bounds, Some(vec![116.85, -1.23]));
    }

    #[test]
    fn test_parse_result_json_missing_optional_fields_use_defaults() {
        let out_dir = Path::new("/tmp/sandbox/exec_defaults");
        let result_path = out_dir.join("result.json");
        let raw = r#"{"error": "partial output"}"#;

        let parsed = parse_result_json(raw, &result_path, out_dir).unwrap();
        assert_eq!(parsed.job_id, "");
        assert_eq!(parsed.status, "failure");
        assert_eq!(parsed.execution_time_ms, 0);
        assert!(parsed.metrics.is_empty());
        assert!(parsed.artifacts.is_empty());
        assert_eq!(parsed.error, Some("partial output".to_string()));
    }

    #[test]
    fn test_parse_result_json_malformed_json_returns_serialization_error() {
        let out_dir = Path::new("/tmp/sandbox/exec_bad");
        let result_path = out_dir.join("result.json");
        let err = parse_result_json("{not valid json", &result_path, out_dir).unwrap_err();
        assert_eq!(err.code(), "SERIALIZATION_ERROR");
    }
}
