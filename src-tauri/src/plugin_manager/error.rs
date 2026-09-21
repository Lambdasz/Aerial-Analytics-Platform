use serde::Serialize;
use thiserror::Error;

/// Domain error enum for all Plugin Manager operations (Rust Core)
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin '{0}' not found in registry")]
    NotFound(String),

    #[error("Invalid manifest at '{path}': {message}")]
    ManifestInvalid { path: String, message: String },

    #[error("Sub-contract missing or invalid at '{path}': {message}")]
    SubcontractInvalid { path: String, message: String },

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("ZIP archive error: {0}")]
    ArchiveError(String),

    #[error("State persistence error: {0}")]
    StateError(String),

    #[error("Healthcheck failed for '{0}': {1}")]
    HealthcheckFailed(String, String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("I/O error: {0}")]
    Io(String),
}

/// Structured Command Error DTO returned to frontend through Tauri IPC
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl PluginError {
    /// String error code for client-side programmatic matching in TypeScript
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "PLUGIN_NOT_FOUND",
            Self::ManifestInvalid { .. } => "MANIFEST_INVALID",
            Self::SubcontractInvalid { .. } => "SUBCONTRACT_INVALID",
            Self::SecurityViolation(_) => "SECURITY_VIOLATION",
            Self::ArchiveError(_) => "ARCHIVE_ERROR",
            Self::StateError(_) => "STATE_ERROR",
            Self::HealthcheckFailed(_, _) => "HEALTHCHECK_FAILED",
            Self::ExecutionError(_) => "EXECUTION_ERROR",
            Self::Io(_) => "IO_ERROR",
        }
    }
}

impl From<PluginError> for CommandError {
    fn from(err: PluginError) -> Self {
        CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for PluginError {
    fn from(err: std::io::Error) -> Self {
        PluginError::Io(err.to_string())
    }
}

impl From<zip::result::ZipError> for PluginError {
    fn from(err: zip::result::ZipError) -> Self {
        PluginError::ArchiveError(err.to_string())
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        PluginError::StateError(err.to_string())
    }
}
