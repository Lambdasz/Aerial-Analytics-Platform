//! Backward-compatibility shim for execution result parsing and run history recording.
//!
//! Maintained as a compatibility bridge for Module 9 (Area & Plot Analytics).
//! Canonical execution result handling is performed by [`crate::plugin_manager::executor::get_job_result`],
//! which returns [`crate::plugin_manager::models::StandardJobResultDto`].
//!
//! ## Purity Boundary
//!
//! Data models and `From` conversions are strictly pure and perform no I/O or
//! mutation. [`parse_execution_result`] and [`record_run_history`] are impure
//! filesystem I/O boundaries that read and write JSON files on disk.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Analytical execution result parsed from `result.json` written by the plugin.
///
/// Retained as a compatibility contract for Module 9 (`plot_api.rs`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionResult {
    pub status: String,
    pub metrics: serde_json::Value,
    pub output_files: Vec<String>,
    pub error_message: Option<String>,
}

impl From<&crate::plugin_manager::models::StandardJobResultDto> for ExecutionResult {
    /// Converts a borrowed [`StandardJobResultDto`](crate::plugin_manager::models::StandardJobResultDto) into a legacy [`ExecutionResult`].
    ///
    /// # Purity
    /// Pure transformation over immutable input; performs no I/O or side effects.
    fn from(dto: &crate::plugin_manager::models::StandardJobResultDto) -> Self {
        Self {
            status: dto.status.clone(),
            metrics: serde_json::Value::Object(
                dto.metrics
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
            output_files: dto
                .artifacts
                .values()
                .map(|a| a.file_path.clone())
                .collect(),
            error_message: dto.error.clone(),
        }
    }
}

impl From<crate::plugin_manager::models::StandardJobResultDto> for ExecutionResult {
    /// Converts an owned [`StandardJobResultDto`](crate::plugin_manager::models::StandardJobResultDto) into a legacy [`ExecutionResult`].
    ///
    /// # Purity
    /// Pure transformation; delegates to [`From<&StandardJobResultDto>`](From::from) with no I/O or side effects.
    fn from(dto: crate::plugin_manager::models::StandardJobResultDto) -> Self {
        Self::from(&dto)
    }
}

impl From<&ExecutionResult> for crate::plugin_manager::models::StandardJobResultDto {
    /// Converts a borrowed legacy [`ExecutionResult`] into a canonical [`StandardJobResultDto`](crate::plugin_manager::models::StandardJobResultDto).
    ///
    /// # Purity
    /// Pure transformation over immutable input; performs no I/O or side effects.
    fn from(res: &ExecutionResult) -> Self {
        let metrics_map = match &res.metrics {
            serde_json::Value::Object(map) => {
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            }
            _ => std::collections::HashMap::new(),
        };

        let artifacts_map = res
            .output_files
            .iter()
            .enumerate()
            .map(|(idx, path)| {
                (
                    format!("artifact_{idx}"),
                    crate::plugin_manager::models::VerifiedArtifactDto {
                        file_path: path.clone(),
                        format: "application/octet-stream".to_string(),
                        bounds: None,
                    },
                )
            })
            .collect();

        Self {
            job_id: String::new(),
            execution_time_ms: 0,
            status: res.status.clone(),
            metrics: metrics_map,
            artifacts: artifacts_map,
            error: res.error_message.clone(),
        }
    }
}

impl From<ExecutionResult> for crate::plugin_manager::models::StandardJobResultDto {
    /// Converts an owned legacy [`ExecutionResult`] into a canonical [`StandardJobResultDto`](crate::plugin_manager::models::StandardJobResultDto).
    ///
    /// # Purity
    /// Pure transformation; delegates to [`From<&ExecutionResult>`](From::from) with no I/O or side effects.
    fn from(res: ExecutionResult) -> Self {
        Self::from(&res)
    }
}

/// Historical record entry capturing execution outcome and output file paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunHistoryEntry {
    pub timestamp: String,
    pub plugin_id: String,
    pub status: String,
    pub output_dir: String,
    pub output_files: Vec<String>,
}

/// Reads and parses `result.json` from `output_dir` into an [`ExecutionResult`].
///
/// # Purity
/// Impure filesystem I/O: checks for and reads `result.json` from `output_dir`.
pub fn parse_execution_result(output_dir: &Path) -> Result<ExecutionResult, String> {
    let result_file = output_dir.join("result.json");
    if !result_file.exists() {
        return Err("result.json not found in output directory.".to_string());
    }

    let content = fs::read_to_string(&result_file)
        .map_err(|e| format!("Failed to read result.json: {}", e))?;

    let result: ExecutionResult = serde_json::from_str(&content)
        .map_err(|e| format!("result.json contains invalid JSON: {}", e))?;

    if result.status == "error" {
        let msg = result
            .error_message
            .unwrap_or_else(|| "Unknown plugin error.".to_string());
        return Err(format!("Plugin execution failed: {}", msg));
    }

    Ok(result)
}

/// Appends a [`RunHistoryEntry`] to the JSON run history file at `history_file_path`.
///
/// # Purity
/// Impure filesystem I/O: reads existing history JSON if present and writes updated JSON to disk.
pub fn record_run_history(history_file_path: &Path, entry: RunHistoryEntry) -> Result<(), String> {
    let mut history: Vec<RunHistoryEntry> = if history_file_path.exists() {
        let content = fs::read_to_string(history_file_path)
            .map_err(|e| format!("Failed to read run history: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| Vec::new())
    } else {
        Vec::new()
    };

    history.push(entry);

    let json_data = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("Failed to serialise run history JSON: {}", e))?;

    fs::write(history_file_path, json_data)
        .map_err(|e| format!("Failed to write run history file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manager::models::{StandardJobResultDto, VerifiedArtifactDto};
    use std::collections::HashMap;

    #[test]
    fn test_result_dto_roundtrip_conversions() {
        let mut metrics = HashMap::new();
        metrics.insert("ndvi_mean".to_string(), serde_json::json!(0.42));
        let mut artifacts = HashMap::new();
        artifacts.insert(
            "mask".to_string(),
            VerifiedArtifactDto {
                file_path: "/tmp/out/mask.png".to_string(),
                format: "image/png".to_string(),
                bounds: Some(vec![-6.2, 106.8, -6.1, 106.9]),
            },
        );

        let dto = StandardJobResultDto {
            job_id: "job-1".to_string(),
            execution_time_ms: 1200,
            status: "success".to_string(),
            metrics,
            artifacts,
            error: None,
        };

        let legacy_borrowed = ExecutionResult::from(&dto);
        let legacy_owned = ExecutionResult::from(dto.clone());
        assert_eq!(legacy_borrowed, legacy_owned);
        assert_eq!(legacy_borrowed.status, "success");
        assert_eq!(
            legacy_borrowed.metrics["ndvi_mean"],
            serde_json::json!(0.42)
        );
        assert_eq!(
            legacy_borrowed.output_files,
            vec!["/tmp/out/mask.png".to_string()]
        );

        let back_borrowed = StandardJobResultDto::from(&legacy_borrowed);
        let back_owned = StandardJobResultDto::from(legacy_borrowed);
        assert_eq!(back_borrowed.status, "success");
        assert_eq!(
            back_borrowed.metrics.get("ndvi_mean"),
            Some(&serde_json::json!(0.42))
        );
        assert_eq!(
            back_borrowed
                .artifacts
                .get("artifact_0")
                .map(|a| a.file_path.as_str()),
            Some("/tmp/out/mask.png")
        );
        assert_eq!(back_owned.job_id, "");
        assert_eq!(back_owned.execution_time_ms, 0);

        let non_object = ExecutionResult {
            status: "failed".to_string(),
            metrics: serde_json::Value::Null,
            output_files: vec![],
            error_message: Some("boom".to_string()),
        };
        let dto_non_obj = StandardJobResultDto::from(&non_object);
        assert!(dto_non_obj.metrics.is_empty());
        assert_eq!(dto_non_obj.error.as_deref(), Some("boom"));
    }

    #[test]
    fn test_parse_execution_result_handles_valid_missing_corrupt_and_error_states() {
        let temp_dir = tempfile::tempdir().unwrap();
        let out_dir = temp_dir.path();

        let missing_err = parse_execution_result(out_dir).unwrap_err();
        assert!(missing_err.contains("not found"));

        fs::write(out_dir.join("result.json"), "{ invalid json").unwrap();
        let json_err = parse_execution_result(out_dir).unwrap_err();
        assert!(json_err.contains("invalid JSON"));

        fs::write(
            out_dir.join("result.json"),
            r#"{"status":"error","metrics":{},"output_files":[],"error_message":"GPU OOM"}"#,
        )
        .unwrap();
        let plugin_err = parse_execution_result(out_dir).unwrap_err();
        assert!(plugin_err.contains("GPU OOM"));

        fs::write(
            out_dir.join("result.json"),
            r#"{"status":"error","metrics":{},"output_files":[],"error_message":null}"#,
        )
        .unwrap();
        let unknown_err = parse_execution_result(out_dir).unwrap_err();
        assert!(unknown_err.contains("Unknown plugin error"));

        fs::write(
            out_dir.join("result.json"),
            r#"{"status":"success","metrics":{"coverage":0.75},"output_files":["overlay.png"],"error_message":null}"#,
        )
        .unwrap();
        let ok = parse_execution_result(out_dir).unwrap();
        assert_eq!(ok.status, "success");
        assert_eq!(ok.metrics["coverage"], serde_json::json!(0.75));
        assert_eq!(ok.output_files, vec!["overlay.png".to_string()]);
    }

    #[test]
    fn test_record_run_history_creates_appends_and_recovers_from_corrupt_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let history_path = temp_dir.path().join("history.json");

        let e1 = RunHistoryEntry {
            timestamp: "2026-11-26T00:00:00Z".to_string(),
            plugin_id: "p1".to_string(),
            status: "success".to_string(),
            output_dir: "/tmp/1".to_string(),
            output_files: vec!["a.png".to_string()],
        };
        let e2 = RunHistoryEntry {
            timestamp: "2026-11-26T00:01:00Z".to_string(),
            plugin_id: "p2".to_string(),
            status: "failed".to_string(),
            output_dir: "/tmp/2".to_string(),
            output_files: vec![],
        };

        record_run_history(&history_path, e1.clone()).unwrap();
        record_run_history(&history_path, e2.clone()).unwrap();

        let loaded: Vec<RunHistoryEntry> =
            serde_json::from_str(&fs::read_to_string(&history_path).unwrap()).unwrap();
        assert_eq!(loaded, vec![e1.clone(), e2]);

        // Corrupt JSON recovers as empty list before pushing new entry
        fs::write(&history_path, "not-json").unwrap();
        record_run_history(&history_path, e1.clone()).unwrap();
        let recovered: Vec<RunHistoryEntry> =
            serde_json::from_str(&fs::read_to_string(&history_path).unwrap()).unwrap();
        assert_eq!(recovered, vec![e1.clone()]);

        // Missing parent directory returns error
        let missing_parent = temp_dir.path().join("no_such_dir").join("history.json");
        let write_err = record_run_history(&missing_parent, e1).unwrap_err();
        assert!(write_err.contains("Failed to write run history file"));
    }
}
