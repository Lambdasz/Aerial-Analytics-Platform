//! Execution result parsing and run history recording.
//!
//! After a plugin subprocess finishes, [`parse_execution_result`] reads and
//! validates the `result.json` file from the output directory. Successful runs
//! are appended to a persistent history file by [`record_run_history`].

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Analytical execution result parsed from `result.json` written by the plugin.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub status: String,
    pub metrics: serde_json::Value,
    pub output_files: Vec<String>,
    pub error_message: Option<String>,
}

/// Historical record entry capturing execution outcome and output file paths.
#[derive(Debug, Serialize, Deserialize)]
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
