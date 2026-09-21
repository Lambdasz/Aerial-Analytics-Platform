//! Tauri IPC commands for plugin payload validation and result processing.
//!
//! These commands are registered in [`crate::lib::run`] via
//! `invoke_handler(tauri::generate_handler![...])` and consumed by the
//! Plugin Manager frontend component.

use super::payload::{assemble_payload, preflight_check, ExecutionPayload, InputsRequirement};
use super::result_handler::{
    parse_execution_result, record_run_history, ExecutionResult, RunHistoryEntry,
};
use std::path::PathBuf;

/// Validates a plugin execution payload against input requirements and
/// writes `payload.json` to the output directory.
///
/// # Arguments
///
/// * `payload` — The execution payload containing image metadata, parameters,
///   and optional AOI.
/// * `inputs_req` — The plugin's declared input requirements (`inputs.json`).
/// * `output_dir` — Absolute path to the sandboxed output directory.
///
/// # Returns
///
/// On success, returns the absolute path to the written `payload.json` file.
///
/// # Errors
///
/// Returns a `String` error if pre-flight validation fails (e.g. missing GPS,
/// unsupported MIME type) or if writing the payload file fails.
#[tauri::command]
pub fn validate_and_create_payload(
    payload: ExecutionPayload,
    inputs_req: InputsRequirement,
    output_dir: String,
) -> Result<String, String> {
    preflight_check(&payload.image, &inputs_req)?;

    let output_path = PathBuf::from(&output_dir);
    let payload_path = assemble_payload(&payload, &output_path)?;
    Ok(payload_path.to_string_lossy().into_owned())
}

/// Parses the plugin's `result.json` output and records a run history entry.
///
/// # Arguments
///
/// * `output_dir` — Absolute path to the plugin's output directory containing
///   `result.json`.
/// * `history_file_path` — Absolute path to the run history JSON file.
/// * `plugin_id` — Identifier of the plugin that was executed.
///
/// # Returns
///
/// On success, returns the parsed [`ExecutionResult`].
///
/// # Errors
///
/// Returns a `String` error if `result.json` is missing or malformed, or if
/// writing the history entry fails.
#[tauri::command]
pub fn process_execution_result(
    output_dir: String,
    history_file_path: String,
    plugin_id: String,
) -> Result<ExecutionResult, String> {
    let output_path = PathBuf::from(&output_dir);

    let result = parse_execution_result(&output_path)?;

    let history_entry = RunHistoryEntry {
        timestamp: chrono::Local::now().to_rfc3339(),
        plugin_id,
        status: result.status.clone(),
        output_dir: output_dir.clone(),
        output_files: result.output_files.clone(),
    };

    let history_path = PathBuf::from(&history_file_path);
    record_run_history(&history_path, history_entry)?;
    Ok(result)
}
