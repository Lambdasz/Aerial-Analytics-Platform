#![allow(dead_code)]

//! Tauri commands for Simple Temporal Change Analysis (Module 10).

/// Runs a comparison between two sessions' analysis results (vegetation
/// coverage, land-cover, tree count) and produces a
/// [`TemporalChangeResult`](super::functions::TemporalChangeResult)
/// and change-area payload.
///
/// **Status**: `todo!()` stub — added so Module 10 has a minimal
/// contract before the deadline, matching the Module 9 (`plot_api`) pattern.
#[tauri::command]
pub fn compare_sessions(
    _baseline_session_id: String,
    _target_session_id: String,
) -> Result<super::functions::TemporalChangeResult, String> {
    todo!("M10: run comparison between two sessions' analysis results")
}

/// Returns the latest computed temporal summary for a project,
/// for Module 11 (dashboard / reporting) to consume.
///
/// **Status**: `todo!()` stub — added so Module 10 has a minimal
/// contract before the deadline, matching the Module 9 (`plot_api`) pattern.
#[tauri::command]
pub fn get_change_summary(
    _project_id: String,
) -> Result<super::functions::TemporalChangeResult, String> {
    todo!("M10: return latest temporal summary for a project")
}
