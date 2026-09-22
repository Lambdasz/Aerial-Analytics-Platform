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
