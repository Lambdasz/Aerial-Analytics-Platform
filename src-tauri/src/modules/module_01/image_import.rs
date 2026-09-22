#![allow(dead_code)]

//! RGB image import (Module 1).
//!
//! Public stubs over the types in [`crate::models::image_import`].
//! None of these functions read the disk. I/O fills [`ImportCandidate`]
//! (header, hash, id) before they run, and applies [`ImportReport`] afterwards.

use crate::models::image_import::{
    ContentHash, DuplicateFlag, ImageFormat, ImportCandidate, ImportError, ImportReport,
    ImportRequest, NameConflict,
};

/// Classifies one candidate from its extension and header magic.
///
/// # Purity
///
/// Pure. Does not read `source_path`.
///
/// # Errors
///
/// [`ImportError::UnsupportedFormat`] if the extension is missing or not
/// jpeg/jpg/dng. [`ImportError::MagicMismatch`] if the extension is supported
/// but `header` does not match.
pub fn classify_file(candidate: &ImportCandidate) -> Result<ImageFormat, ImportError> {
    let _ = candidate;
    unimplemented!("classify_file: not implemented")
}

/// Finds destination-basename collisions.
///
/// Checks incoming candidates against each other and against names already
/// in the session folder. Does not rename; the frontend resolves
/// [`NameConflict`].
///
/// # Purity
///
/// Pure.
pub fn find_name_conflicts(
    candidates: &[ImportCandidate],
    existing_dest_names: &[String],
) -> Vec<NameConflict> {
    let _ = (candidates, existing_dest_names);
    unimplemented!("find_name_conflicts: not implemented")
}

/// Flags a candidate whose content hash matches an image already in the project.
///
/// `known` is `(image_id, hash)` pairs. Duplicate scope is the project, not
/// the session folder.
///
/// # Purity
///
/// Pure.
pub fn find_duplicate(
    candidate: &ImportCandidate,
    known: &[(String, ContentHash)],
) -> Option<DuplicateFlag> {
    let _ = (candidate, known);
    unimplemented!("find_duplicate: not implemented")
}

/// Composes classify, name-conflict, and duplicate checks into an [`ImportReport`].
///
/// Does not copy files. `from_folder` is already set on `request` by I/O.
///
/// # Errors
///
/// [`ImportError::NoImagesFound`] or [`ImportError::SessionNotFound`] when
/// the request cannot be applied. Per-file format failures belong in
/// `rejected`, not this `Err`.
pub fn import_images(
    request: &ImportRequest,
    known: &[(String, ContentHash)],
    existing_dest_names: &[String],
) -> Result<ImportReport, ImportError> {
    let _ = (request, known, existing_dest_names);
    unimplemented!("import_images: not implemented")
}
