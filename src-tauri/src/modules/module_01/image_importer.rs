#![allow(dead_code)]

//! RGB image import (Module 1).
//!
//! [`import_images`] is the entry point. [`find_name_conflicts`] and
//! [`find_duplicate`] are shared helpers other modules call through the
//! `module_01` re-exports.
//! None of these functions read the disk. I/O fills [`ImportCandidate`]
//! (header, hash, id) before they run, and applies [`ImportReport`] afterwards.

use std::collections::{BTreeMap, HashSet};

use crate::models::image::ImageFormat;
use crate::models::image::{
    ContentHash, DuplicateFlag, ImportCandidate, ImportError, ImportReport, ImportRequest,
    NameConflict, RejectedFile, SessionTarget,
};
use crate::modules::module_01::error::MetadataError;
use crate::modules::module_01::metadata_extractor::detect_format;

/// Classifies one candidate by delegating the magic sniff to [`detect_format`].
///
/// # Purity
///
/// Pure. Does not read `source_path`.
///
/// # Errors
///
/// [`ImportError::UnsupportedFormat`] if the extension is missing or not
/// jpeg/jpg/dng. [`ImportError::MagicMismatch`] if the extension is supported
/// but `header` does not match. `path` is `candidate.source_path`.
///
/// Also returns [`ImportError::UnsupportedFormat`] as a defensive fallback if
/// [`detect_format`] returns an `Io`/`MalformedExif`/`MalformedXmp`
/// [`MetadataError`] — variants its own contract says it never produces. In
/// that case `extension` is not necessarily unsupported; the failure is
/// unrelated to the extension check.
fn classify_file(candidate: &ImportCandidate) -> Result<ImageFormat, ImportError> {
    let extension = extension_of(&candidate.file_name);
    match detect_format(&candidate.header, &extension) {
        Ok(format) => Ok(format),
        Err(MetadataError::UnsupportedFormat { extension, .. }) => {
            Err(ImportError::UnsupportedFormat {
                path: candidate.source_path.clone(),
                extension,
            })
        }
        Err(MetadataError::MagicMismatch { .. }) => Err(ImportError::MagicMismatch {
            path: candidate.source_path.clone(),
        }),
        // Defensive fallback: detect_format's documented contract says it never
        // returns these variants, but that contract isn't enforced by the type
        // system. Treat it as an unsupported format rather than panicking.
        Err(
            MetadataError::Io(_)
            | MetadataError::MalformedExif(_)
            | MetadataError::MalformedXmp(_)
            | MetadataError::NotImplemented { .. },
        ) => Err(ImportError::UnsupportedFormat {
            path: candidate.source_path.clone(),
            extension,
        }),
    }
}

/// Basename of a path-style string, splitting on both separators.
fn basename_of(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Destination file name safe to join onto the session folder.
///
/// Strips directories. Rejects empty names, `.`, and `..` so a raw
/// `file_name` cannot escape the session folder.
fn dest_basename(name: &str) -> Option<&str> {
    let base = basename_of(name);
    if base.is_empty() || base == "." || base == ".." {
        None
    } else {
        Some(base)
    }
}

fn extension_of(file_name: &str) -> String {
    basename_of(file_name)
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim_start_matches('.').to_ascii_lowercase())
        .unwrap_or_default()
}

/// Finds destination-basename collisions.
///
/// Compares incoming candidates against each other and against the paths
/// already in the session folder. Matching is case-insensitive on the
/// basename (macOS and Windows filesystems are case-insensitive, and the
/// flatten-copy puts everything in one folder). `dest_name` is the basename
/// only — directories, `.`, and `..` are stripped — and prefers the name
/// already in the session folder when `existing_path` is set. Does not
/// rename; the frontend resolves [`NameConflict`]. Existing paths with no
/// incoming counterpart are not conflicts.
///
/// # Purity
///
/// Pure.
pub(crate) fn find_name_conflicts(
    candidates: &[ImportCandidate],
    existing_paths: &[String],
) -> Vec<NameConflict> {
    struct Group {
        dest_name: String,
        sources: Vec<String>,
        existing_path: Option<String>,
    }

    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for path in existing_paths {
        let Some(base) = dest_basename(path) else {
            continue;
        };
        let group = groups
            .entry(base.to_ascii_lowercase())
            .or_insert_with(|| Group {
                dest_name: String::new(),
                sources: Vec::new(),
                existing_path: None,
            });
        if group.existing_path.is_none() {
            group.existing_path = Some(path.clone());
            group.dest_name = base.to_string();
        }
    }
    for candidate in candidates {
        let Some(base) = dest_basename(&candidate.file_name) else {
            continue;
        };
        let group = groups
            .entry(base.to_ascii_lowercase())
            .or_insert_with(|| Group {
                dest_name: String::new(),
                sources: Vec::new(),
                existing_path: None,
            });
        if group.dest_name.is_empty() {
            group.dest_name = base.to_string();
        }
        group.sources.push(candidate.source_path.clone());
    }

    groups
        .into_values()
        .filter_map(|group| {
            let collides_incoming = group.sources.len() > 1;
            let collides_existing = group.sources.len() == 1 && group.existing_path.is_some();
            (collides_incoming || collides_existing).then_some(NameConflict {
                dest_name: group.dest_name,
                sources: group.sources,
                existing_path: group.existing_path,
            })
        })
        .collect()
}

/// Flags a candidate whose content hash matches an image already in the project.
///
/// `known` is `(image_id, hash)` pairs. Duplicate scope is the project, not
/// the session folder. Returns the first matching pair.
///
/// # Purity
///
/// Pure.
pub(crate) fn find_duplicate(
    candidate: &ImportCandidate,
    known: &[(String, ContentHash)],
) -> Option<DuplicateFlag> {
    known
        .iter()
        .find(|(_, hash)| *hash == candidate.content_hash)
        .map(|(image_id, hash)| DuplicateFlag {
            source_path: candidate.source_path.clone(),
            existing_image_id: image_id.clone(),
            content_hash: hash.clone(),
        })
}

/// Composes classify, name-conflict, and duplicate checks into an [`ImportReport`].
///
/// Pure. Does not copy files, query SQLite, or touch the clock. `from_folder`
/// and `candidates` are already prepared by the I/O layer, and the caller
/// that owns the database connection checks that an `Existing` session id is
/// real — this function has no session list, so it never produces
/// [`ImportError::SessionNotFound`].
///
/// Duplicate scope is the project. Persisted `known` hashes are checked
/// first. Intra-batch hashes are folded only after name conflicts, and only
/// against ids that this call actually imports. A later file is never marked
/// as a duplicate of a candidate this call holds back.
///
/// A name conflict holds a candidate back: it is not listed in
/// `imported_image_ids` until the frontend resolves the conflict. Rejected
/// and duplicate files are never conflict candidates, because they are never
/// copied. `dest_name` in the report is a basename; `.`, `..`, and empty
/// names are rejected instead of imported.
///
/// # Errors
///
/// [`ImportError::NoImagesFound`] if `candidates` is empty. A batch where
/// every file is rejected or duplicated is still `Ok` — per-file failures
/// live in `rejected` and `duplicates`, not this `Err`.
pub(crate) fn import_images(
    request: &ImportRequest,
    known: &[(String, ContentHash)],
    existing_paths: &[String],
) -> Result<ImportReport, ImportError> {
    if request.candidates.is_empty() {
        return Err(ImportError::NoImagesFound);
    }
    let session_id = match &request.session {
        SessionTarget::Existing { id } | SessionTarget::New { id, .. } => id.clone(),
    };

    let mut pending = Vec::new();
    let mut duplicates = Vec::new();
    let mut rejected = Vec::new();

    for candidate in &request.candidates {
        if dest_basename(&candidate.file_name).is_none() {
            rejected.push(RejectedFile {
                source_path: candidate.source_path.clone(),
                reason: ImportError::UnsupportedFormat {
                    path: candidate.source_path.clone(),
                    extension: extension_of(&candidate.file_name),
                },
            });
        } else if let Err(reason) = classify_file(candidate) {
            rejected.push(RejectedFile {
                source_path: candidate.source_path.clone(),
                reason,
            });
        } else if let Some(flag) = find_duplicate(candidate, known) {
            duplicates.push(flag);
        } else {
            pending.push(candidate.clone());
        }
    }

    let name_conflicts = find_name_conflicts(&pending, existing_paths);
    let conflicting: HashSet<&str> = name_conflicts
        .iter()
        .flat_map(|conflict| conflict.sources.iter().map(String::as_str))
        .collect();

    let mut seen: Vec<(String, ContentHash)> = Vec::new();
    let mut imported_image_ids = Vec::new();
    for candidate in pending {
        if conflicting.contains(candidate.source_path.as_str()) {
            continue;
        }
        if let Some(flag) = find_duplicate(&candidate, &seen) {
            duplicates.push(flag);
        } else {
            seen.push((candidate.id.clone(), candidate.content_hash.clone()));
            imported_image_ids.push(candidate.id);
        }
    }

    Ok(ImportReport {
        session_id,
        imported_image_ids,
        duplicates,
        rejected,
        name_conflicts,
    })
}
