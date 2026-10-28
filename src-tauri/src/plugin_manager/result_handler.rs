//! Backward-compatibility shim for execution result parsing and run history recording.
//!
//! Maintained as a compatibility bridge for Module 9 (Area & Plot Analytics).
//! Canonical execution result handling is performed by [`crate::plugin_manager::executor::get_job_result`],
//! which returns [`crate::plugin_manager::models::StandardJobResultDto`].

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
    fn from(dto: crate::plugin_manager::models::StandardJobResultDto) -> Self {
        Self::from(&dto)
    }
}

impl From<&ExecutionResult> for crate::plugin_manager::models::StandardJobResultDto {
    fn from(res: &ExecutionResult) -> Self {
        let metrics_map = match &res.metrics {
            serde_json::Value::Object(map) => {
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            }
            _ => std::collections::HashMap::new(),
        };

        let mut artifacts_map = std::collections::HashMap::new();
        for (idx, path) in res.output_files.iter().enumerate() {
            let key = format!("artifact_{idx}");
            artifacts_map.insert(
                key,
                crate::plugin_manager::models::VerifiedArtifactDto {
                    file_path: path.clone(),
                    format: "application/octet-stream".to_string(),
                    bounds: None,
                },
            );
        }

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
