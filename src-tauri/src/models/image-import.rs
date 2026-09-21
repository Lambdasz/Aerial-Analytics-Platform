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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionTarget {
    Existing(String),
    New { id: String, label: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCandidate {
    pub id: String,
    pub source_path: String,
    pub file_name: String,
    pub header: Vec<u8>,
    pub content_hash: ContentHash,
    pub mode: ImportMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameConflict {
    pub dest_name: String,
    pub sources: Vec<String>,
    pub existing: Option<String>,
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
    pub imported: Vec<String>,
    pub duplicates: Vec<DuplicateFlag>,
    pub rejected: Vec<RejectedFile>,
    pub name_conflicts: Vec<NameConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportError {
    NoImagesFound,
    SessionNotFound { session_id: String },
    UnsupportedFormat { path: String, found: String },
    MagicMismatch { path: String },
}
