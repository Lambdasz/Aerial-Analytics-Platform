use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Dng,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportMode {
    Singular,
    Batch,
}

/// SHA-256 of the raw file bytes, lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionTarget {
    Existing { id: String },
    New { id: String, label: String },
}

/// One import operation. `mode` is how the user picked files, not a per-file property.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub mode: ImportMode,
    pub session: SessionTarget,
    pub candidates: Vec<ImportCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCandidate {
    pub id: String,
    pub source_path: String,
    pub file_name: String,
    /// First ≥16 bytes of the file. Used only for JPEG/DNG magic sniff, not EXIF.
    pub header: Vec<u8>,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameConflict {
    pub dest_name: String,
    /// Incoming source paths that want `dest_name`.
    pub sources: Vec<String>,
    /// Path already in the session folder with `dest_name`. `None` if only incoming files collide.
    pub existing_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateFlag {
    pub source_path: String,
    pub existing_image_id: String,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedFile {
    pub source_path: String,
    pub reason: ImportError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub session_id: String,
    pub imported_image_ids: Vec<String>,
    pub duplicates: Vec<DuplicateFlag>,
    pub rejected: Vec<RejectedFile>,
    pub name_conflicts: Vec<NameConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportError {
    NoImagesFound,
    SessionNotFound {
        session_id: String,
    },
    /// Extension is not jpeg/jpg/dng (or missing). `extension` has no leading dot.
    UnsupportedFormat {
        path: String,
        extension: String,
    },
    /// Extension is jpeg/dng but `header` magic does not match.
    MagicMismatch {
        path: String,
    },
}
