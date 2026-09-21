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
        return Err("File result.json tidak ditemukan di direktori output.".to_string());
    }

    let content = fs::read_to_string(&result_file)
        .map_err(|e| format!("Gagal membaca result.json: {}", e))?;

    let result: ExecutionResult = serde_json::from_str(&content)
        .map_err(|e| format!("Format JSON result.json tidak valid: {}", e))?;

    if result.status == "error" {
        let msg = result
            .error_message
            .unwrap_or_else(|| "Terjadi kesalahan tidak dikenal pada plugin.".to_string());
        return Err(format!("Plugin mengeksekusi dengan status error: {}", msg));
    }

    Ok(result)
}

pub fn record_run_history(history_file_path: &Path, entry: RunHistoryEntry) -> Result<(), String> {
    let mut history: Vec<RunHistoryEntry> = if history_file_path.exists() {
        let content = fs::read_to_string(history_file_path)
            .map_err(|e| format!("Gagal membaca riwayat: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| Vec::new())
    } else {
        Vec::new()
    };

    history.push(entry);

    let json_data = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("Gagal memformat JSON riwayat: {}", e))?;

    fs::write(history_file_path, json_data)
        .map_err(|e| format!("Gagal menulis berkas riwayat eksekusi: {}", e))?;

    Ok(())
}
