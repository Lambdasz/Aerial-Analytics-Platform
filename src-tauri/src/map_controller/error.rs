#![allow(dead_code)]

//! Error types for the Map Controller module (Module 3).

use thiserror::Error;

/// Domain error for map controller operations.
///
/// Maps to [`CommandError`](crate::plugin_manager::error::CommandError) for
/// transmission to the frontend via Tauri IPC.
#[derive(Error, Debug)]
pub enum MapControllerError {
    /// The provided geometry failed validation (e.g. self-intersecting ring,
    /// invalid coordinate bounds).
    #[error("geometri tidak valid: {0}")]
    InvalidGeometry(String),

    /// An I/O operation failed while reading or writing map data.
    #[error("gagal I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl MapControllerError {
    /// Returns a short error code for client-side programmatic matching.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidGeometry(_) => "INVALID_GEOMETRY",
            Self::Io(_) => "IO_ERROR",
        }
    }
}

impl From<MapControllerError> for crate::plugin_manager::error::CommandError {
    fn from(err: MapControllerError) -> Self {
        crate::plugin_manager::error::CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}
