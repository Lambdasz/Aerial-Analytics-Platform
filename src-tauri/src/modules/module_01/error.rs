#![allow(dead_code)]

//! Error types for the Image Metadata Extraction module (Module 1.3).

use thiserror::Error;

/// Domain error for metadata extraction operations.
///
/// Maps to [`CommandError`](crate::plugin_manager::error::CommandError) for
/// transmission to the frontend via Tauri IPC.
#[derive(Error, Debug)]
pub enum MetadataError {
    /// An I/O operation failed while reading the source image file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The file extension or magic bytes do not correspond to a supported
    /// aerial image format (JPEG or DNG).
    #[error("unsupported format for '{path}' (extension: '{extension}')")]
    UnsupportedFormat { path: String, extension: String },

    /// The file extension claims a supported format, but the leading bytes
    /// do not match that format's magic number.
    #[error("magic byte mismatch for '{path}'")]
    MagicMismatch { path: String },

    /// The EXIF segment was present but could not be parsed.
    #[error("malformed EXIF data: {0}")]
    MalformedExif(String),

    /// The XMP DJI packet was present but could not be parsed.
    #[error("malformed XMP data: {0}")]
    MalformedXmp(String),

    /// Extraction is declared but not implemented yet.
    #[error("metadata extraction is not implemented for '{path}'")]
    NotImplemented { path: String },
}

impl MetadataError {
    /// Returns a short error code for client-side programmatic matching.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO_ERROR",
            Self::UnsupportedFormat { .. } => "UNSUPPORTED_FORMAT",
            Self::MagicMismatch { .. } => "MAGIC_MISMATCH",
            Self::MalformedExif(_) => "MALFORMED_EXIF",
            Self::MalformedXmp(_) => "MALFORMED_XMP",
            Self::NotImplemented { .. } => "NOT_IMPLEMENTED",
        }
    }
}

impl From<MetadataError> for crate::plugin_manager::error::CommandError {
    fn from(err: MetadataError) -> Self {
        crate::plugin_manager::error::CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}

/// Domain error for Flight Session Management operations (Module 1.4).
///
/// Maps to [`CommandError`](crate::plugin_manager::error::CommandError) for
/// transmission to the frontend via Tauri IPC.
#[derive(Error, Debug)]
pub enum SessionError {
    /// No session exists with the given id.
    #[error("session '{session_id}' not found")]
    NotFound { session_id: uuid::Uuid },

    /// `assign_image_to_session` was called with an image id that does not
    /// exist in the `image` table.
    #[error("image '{image_id}' not found")]
    ImageNotFound { image_id: uuid::Uuid },

    /// The underlying SQLite operation failed.
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
}

impl SessionError {
    /// Returns a short error code for client-side programmatic matching.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "SESSION_NOT_FOUND",
            Self::ImageNotFound { .. } => "IMAGE_NOT_FOUND",
            Self::Db(_) => "DB_ERROR",
        }
    }
}

impl From<SessionError> for crate::plugin_manager::error::CommandError {
    fn from(err: SessionError) -> Self {
        crate::plugin_manager::error::CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}
