use std::path::PathBuf;
use super::payload::{
    assemble_payload, preflight_check, ExecutionPayload, InputsRequirement,
};
use super::result_handler::{
    parse_execution_result, record_run_history, ExecutionResult, RunHistoryEntry,
};

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