use serde::{Deserialize, Serialize};
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

    #[error("Execution timed out after {0} seconds")]
    Timeout(u64),

    #[error("Job '{0}' not found in active tracker")]
    JobNotFound(String),

    #[error("Plugin '{0}' is disabled or not registered")]
    PluginUnavailable(String),

    #[error("Job '{0}' is not in Completed state")]
    NotCompleted(String),

    #[error("Artifact file not found on disk: {0}")]
    ArtifactMissing(String),

    #[error("Process spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("I/O error: {0}")]
    Io(String),
}

/// Structured Command Error DTO returned to frontend through Tauri IPC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

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
            Self::Timeout(_) => "EXECUTION_TIMEOUT",
            Self::JobNotFound(_) => "JOB_NOT_FOUND",
            Self::PluginUnavailable(_) => "PLUGIN_UNAVAILABLE",
            Self::NotCompleted(_) => "JOB_NOT_COMPLETED",
            Self::ArtifactMissing(_) => "ARTIFACT_MISSING",
            Self::SpawnFailed(_) => "SPAWN_FAILED",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
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
        PluginError::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_error_codes() {
        assert_eq!(
            PluginError::NotFound("test".into()).code(),
            "PLUGIN_NOT_FOUND"
        );
        assert_eq!(
            PluginError::ManifestInvalid {
                path: "p".into(),
                message: "m".into()
            }
            .code(),
            "MANIFEST_INVALID"
        );
        assert_eq!(
            PluginError::SubcontractInvalid {
                path: "p".into(),
                message: "m".into()
            }
            .code(),
            "SUBCONTRACT_INVALID"
        );
        assert_eq!(
            PluginError::SecurityViolation("sec".into()).code(),
            "SECURITY_VIOLATION"
        );
        assert_eq!(
            PluginError::ArchiveError("zip".into()).code(),
            "ARCHIVE_ERROR"
        );
        assert_eq!(
            PluginError::StateError("state".into()).code(),
            "STATE_ERROR"
        );
        assert_eq!(
            PluginError::HealthcheckFailed("p".into(), "fail".into()).code(),
            "HEALTHCHECK_FAILED"
        );
        assert_eq!(
            PluginError::ExecutionError("exec".into()).code(),
            "EXECUTION_ERROR"
        );
        assert_eq!(PluginError::Io("io".into()).code(), "IO_ERROR");
        assert_eq!(PluginError::Timeout(10).code(), "EXECUTION_TIMEOUT");
        assert_eq!(
            PluginError::JobNotFound("job-123".into()).code(),
            "JOB_NOT_FOUND"
        );
        assert_eq!(
            PluginError::PluginUnavailable("plug-1".into()).code(),
            "PLUGIN_UNAVAILABLE"
        );
        assert_eq!(
            PluginError::NotCompleted("job-123".into()).code(),
            "JOB_NOT_COMPLETED"
        );
        assert_eq!(
            PluginError::ArtifactMissing("artifact.png".into()).code(),
            "ARTIFACT_MISSING"
        );
        assert_eq!(
            PluginError::SpawnFailed("spawn".into()).code(),
            "SPAWN_FAILED"
        );
        assert_eq!(
            PluginError::Serialization("bad json".into()).code(),
            "SERIALIZATION_ERROR"
        );
    }

    #[test]
    fn test_serde_json_error_conversion_maps_to_serialization() {
        let bad_json = "{ invalid_json: ";
        let serde_err = serde_json::from_str::<serde_json::Value>(bad_json).unwrap_err();
        let plugin_err: PluginError = serde_err.into();

        assert_eq!(plugin_err.code(), "SERIALIZATION_ERROR");
        assert!(matches!(plugin_err, PluginError::Serialization(_)));
    }

    #[test]
    fn test_command_error_from_plugin_error() {
        let err = PluginError::Timeout(15);
        let cmd_err: CommandError = err.into();

        assert_eq!(cmd_err.code, "EXECUTION_TIMEOUT");
        assert_eq!(cmd_err.message, "Execution timed out after 15 seconds");
        assert_eq!(
            cmd_err.to_string(),
            "[EXECUTION_TIMEOUT]: Execution timed out after 15 seconds"
        );
    }

    #[test]
    fn test_command_error_serde_roundtrip() {
        let original = CommandError {
            code: "EXECUTION_TIMEOUT".to_string(),
            message: "Process timed out".to_string(),
        };

        let json = serde_json::to_string(&original).expect("serialize CommandError");
        let deserialized: CommandError =
            serde_json::from_str(&json).expect("deserialize CommandError");

        assert_eq!(original, deserialized);
    }
}
