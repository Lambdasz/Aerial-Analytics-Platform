//! Aerial Analytics Platform — desktop application entry point.
//!
//! This binary launches the Tauri application shell defined in
//! [`aerial_analytics_platform_lib`]. On Windows release builds a
//! console window is suppressed via `windows_subsystem`.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aerial_analytics_platform_lib::run()
}
