#![allow(dead_code)]

//! RGB image import (Module 1).
//!
//! [`import_images`] is the entry point. [`find_name_conflicts`] and
//! [`find_duplicate`] are shared helpers other modules call through the
//! `module_01` re-exports. [`flag_incomplete_metadata`] is the second phase:
//! after [`import_images`] decides what gets imported, the I/O layer extracts
//! metadata for each imported id (impure) and calls it to flag images with
//! missing required fields. Flagged images stay imported — they are reported
//! as "incomplete metadata", never rejected.
//! None of these functions read the disk. I/O fills [`ImportCandidate`]
//! (header, hash, id) before they run, and applies [`ImportReport`] afterwards.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::models::image::ImageFormat;
use crate::models::image::{
    ContentHash, DuplicateFlag, ImageMetadata, ImportCandidate, ImportError, ImportReport,
    ImportRequest, IncompleteMetadataFlag, NameConflict, RejectedFile, SessionTarget,
};
use crate::modules::module_01::error::MetadataError;
use crate::modules::module_01::metadata_extractor::{detect_format, missing_required_fields};

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
        // NotImplemented is propagated distinctly: an unfinished backend
        // must never be misreported as a user file problem. Unreachable
        // from `detect_format` today (it never returns this variant), but
        // the arm keeps the conversion honest if that ever changes.
        Err(MetadataError::NotImplemented { .. }) => Err(ImportError::NotImplemented {
            path: candidate.source_path.clone(),
        }),
        // Defensive fallback: detect_format's documented contract says it never
        // returns these variants, but that contract isn't enforced by the type
        // system. Treat it as an unsupported format rather than panicking.
        Err(
            MetadataError::Io(_) | MetadataError::MalformedExif(_) | MetadataError::MalformedXmp(_),
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
/// first. Intra-batch hashes are folded only after name conflicts: every
/// pending candidate registers its hash (first claimant in batch order
/// wins), including candidates held back by a name conflict. A later file
/// with identical content is flagged as a duplicate of the held-back id
/// rather than imported — conflict resolution happens in a later import
/// call that cannot re-run this batch's dedup, so holding the hashes back
/// would silently admit two copies of the same image once the user renames
/// and imports the conflicted files.
///
/// A name conflict holds a candidate back: it is not listed in
/// `imported_image_ids` until the frontend resolves the conflict. Rejected
/// and duplicate files are never conflict candidates, because they are never
/// copied. `dest_name` in the report is a basename; `.`, `..`, empty names,
/// and trailing-separator names are rejected with
/// [`ImportError::InvalidFileName`] instead of imported.
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
                reason: ImportError::InvalidFileName {
                    path: candidate.source_path.clone(),
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
        // Check against hashes claimed by earlier batch-mates (held-back
        // candidates included), then claim this hash if still unclaimed, so
        // a file identical to a conflict-held file is flagged rather than
        // imported. The conflict hold-back below still applies afterwards.
        let duplicate_of = find_duplicate(&candidate, &seen);
        if duplicate_of.is_none() {
            seen.push((candidate.id.clone(), candidate.content_hash.clone()));
        }
        if conflicting.contains(candidate.source_path.as_str()) {
            continue;
        }
        if let Some(flag) = duplicate_of {
            duplicates.push(flag);
        } else {
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

/// Flags successfully imported candidates whose extracted metadata is
/// missing required fields ("incomplete metadata").
///
/// Pure, total (never fails).
///
/// # Composition
///
/// Second phase of the import pipeline, after [`import_images`]:
///
/// ```text
/// import_images(request, known, existing_paths)
///   -> I/O copies files + extracts metadata per imported id (impure)
///   -> flag_incomplete_metadata(imported_candidates, metadata)
/// ```
///
/// `metadata` maps `ImportCandidate.id` to its extracted [`ImageMetadata`].
/// A candidate with **no entry** was never extracted (extraction unavailable
/// — e.g. `extract_metadata` is still a stub): completeness cannot be judged,
/// so no flag is emitted. This keeps a stubbed pipeline from reporting every
/// import as "incomplete metadata". When extraction was attempted but failed
/// (corrupt or unreadable metadata), the I/O layer records
/// [`ImageMetadata::default()`] instead — every required field is then
/// missing, and the candidate is flagged with all of them.
/// Flagged images stay imported; completeness never moves an id out of
/// `imported_image_ids` into `rejected` or `duplicates`.
///
/// # Returns
///
/// One [`IncompleteMetadataFlag`] per imported candidate lacking at least
/// one required field, in the same order as `imported`. Fully described
/// candidates, and candidates with no metadata entry, produce no flag.
pub(crate) fn flag_incomplete_metadata(
    imported: &[ImportCandidate],
    metadata: &HashMap<String, ImageMetadata>,
) -> Vec<IncompleteMetadataFlag> {
    imported
        .iter()
        .filter_map(|candidate| {
            let extracted = metadata.get(&candidate.id)?;
            let missing = missing_required_fields(extracted);
            if missing.is_empty() {
                None
            } else {
                Some(IncompleteMetadataFlag {
                    candidate_id: candidate.id.clone(),
                    source_path: candidate.source_path.clone(),
                    missing,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::module_01::metadata_extractor::REQUIRED_METADATA_FIELDS;

    fn candidate(id: &str) -> ImportCandidate {
        ImportCandidate {
            id: id.to_string(),
            source_path: format!("/photos/{id}.jpg"),
            file_name: format!("{id}.jpg"),
            header: vec![0xFF, 0xD8, 0xFF, 0xE0],
            content_hash: ContentHash(format!("hash-{id}")),
        }
    }

    fn candidate_named(
        id: &str,
        source_path: &str,
        file_name: &str,
        hash: &str,
    ) -> ImportCandidate {
        ImportCandidate {
            id: id.to_string(),
            source_path: source_path.to_string(),
            file_name: file_name.to_string(),
            header: vec![0xFF, 0xD8, 0xFF, 0xE0],
            content_hash: ContentHash(hash.to_string()),
        }
    }

    fn import_request(candidates: Vec<ImportCandidate>) -> ImportRequest {
        ImportRequest {
            from_folder: false,
            session: SessionTarget::New {
                id: "session-1".to_string(),
                label: "Session".to_string(),
            },
            candidates,
        }
    }

    fn complete_metadata() -> ImageMetadata {
        ImageMetadata {
            gps_latitude: Some(-7.5),
            gps_longitude: Some(110.0),
            gps_altitude_m: Some(120.0),
            date_time_original: "2026-09-18T07:12:00".parse().ok(),
            width: Some(4000),
            height: Some(3000),
            ..ImageMetadata::default()
        }
    }

    #[test]
    fn flags_only_incomplete_in_import_order() {
        let imported = vec![candidate("a"), candidate("b"), candidate("c")];
        let metadata: HashMap<String, ImageMetadata> = [
            ("a".to_string(), complete_metadata()),
            (
                "b".to_string(),
                ImageMetadata {
                    date_time_original: None,
                    ..complete_metadata()
                },
            ),
            // "c" has no entry: never extracted (unknown) — skipped, not flagged.
        ]
        .into_iter()
        .collect();

        let flags = flag_incomplete_metadata(&imported, &metadata);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].candidate_id, "b");
        assert_eq!(flags[0].source_path, "/photos/b.jpg");
        assert_eq!(flags[0].missing, vec!["date_time_original".to_string()]);
    }

    #[test]
    fn absent_metadata_means_unknown_not_flagged() {
        // Stub-era scenario: extraction unavailable, so the map is empty.
        // Nothing is reported "incomplete" — completeness cannot be judged.
        let imported = vec![candidate("a"), candidate("b")];
        assert!(flag_incomplete_metadata(&imported, &HashMap::new()).is_empty());
    }

    #[test]
    fn default_metadata_flags_every_required_field() {
        // Corrupt/unreadable convention: the I/O layer records
        // `ImageMetadata::default()` when extraction was attempted but
        // failed, so the candidate is flagged with all required fields.
        let imported = vec![candidate("a")];
        let metadata: HashMap<String, ImageMetadata> =
            [("a".to_string(), ImageMetadata::default())]
                .into_iter()
                .collect();
        let flags = flag_incomplete_metadata(&imported, &metadata);
        assert_eq!(flags.len(), 1);
        assert_eq!(
            flags[0].missing,
            REQUIRED_METADATA_FIELDS
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_when_all_complete_or_no_imports() {
        let imported = vec![candidate("a")];
        let metadata: HashMap<String, ImageMetadata> = [("a".to_string(), complete_metadata())]
            .into_iter()
            .collect();
        assert!(flag_incomplete_metadata(&imported, &metadata).is_empty());
        assert!(flag_incomplete_metadata(&[], &HashMap::new()).is_empty());
    }

    #[test]
    fn held_back_hash_still_dedups_later_identical_file() {
        // Reviewer scenario: A and C collide on x.jpg (also colliding with
        // the existing x.jpg) while B carries identical content under a
        // free name. B must be flagged as a duplicate of held-back A, not
        // imported — otherwise resolving the conflict later admits two
        // copies of hash H into the project.
        let request = import_request(vec![
            candidate_named("a", "/in/a.jpg", "x.jpg", "H"),
            candidate_named("b", "/in/b.jpg", "y.jpg", "H"),
            candidate_named("c", "/in/c.jpg", "x.jpg", "H"),
        ]);
        let report =
            import_images(&request, &[], &["/session/x.jpg".to_string()]).expect("import runs");
        assert!(report.imported_image_ids.is_empty());
        assert_eq!(
            report.duplicates,
            vec![DuplicateFlag {
                source_path: "/in/b.jpg".to_string(),
                existing_image_id: "a".to_string(),
                content_hash: ContentHash("H".to_string()),
            }]
        );
        assert_eq!(report.name_conflicts.len(), 1);
        assert_eq!(
            report.name_conflicts[0].sources,
            vec!["/in/a.jpg".to_string(), "/in/c.jpg".to_string()]
        );
    }

    #[test]
    fn first_claimant_wins_without_conflicts() {
        let request = import_request(vec![
            candidate_named("x", "/in/x.jpg", "x.jpg", "H"),
            candidate_named("y", "/in/y.jpg", "y.jpg", "H"),
        ]);
        let report = import_images(&request, &[], &[]).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["x".to_string()]);
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].existing_image_id, "x".to_string());
    }

    #[test]
    fn conflict_later_does_not_block_earlier_import() {
        // Order matters: the earlier claimant owns the hash. B (free name)
        // imports normally; A (conflicting name, identical content) is held
        // back without a duplicate flag.
        let request = import_request(vec![
            candidate_named("b", "/in/b.jpg", "y.jpg", "H"),
            candidate_named("a", "/in/a.jpg", "x.jpg", "H"),
        ]);
        let report =
            import_images(&request, &[], &["/session/x.jpg".to_string()]).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["b".to_string()]);
        assert!(report.duplicates.is_empty());
        assert_eq!(report.name_conflicts.len(), 1);
    }

    #[test]
    fn conflicts_group_case_insensitively_first_name_wins() {
        let candidates = vec![
            candidate_named("a", "/in/a.jpg", "Photo.JPG", "H1"),
            candidate_named("b", "/in/b.jpg", "photo.jpg", "H2"),
        ];
        let conflicts = find_name_conflicts(&candidates, &[]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].dest_name, "Photo.JPG");
        assert_eq!(
            conflicts[0].sources,
            vec!["/in/a.jpg".to_string(), "/in/b.jpg".to_string()]
        );
        assert_eq!(conflicts[0].existing_path, None);
    }

    #[test]
    fn conflicts_prefer_existing_path_casing() {
        let candidates = vec![candidate_named("a", "/in/a.jpg", "keep.jpg", "H1")];
        let conflicts = find_name_conflicts(&candidates, &["/session/Keep.JPG".to_string()]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].dest_name, "Keep.JPG");
        assert_eq!(
            conflicts[0].existing_path,
            Some("/session/Keep.JPG".to_string())
        );
    }

    #[test]
    fn no_conflict_for_unique_or_unmatched_existing() {
        let candidates = vec![
            candidate_named("a", "/in/a.jpg", "a.jpg", "H1"),
            candidate_named("b", "/in/b.jpg", "b.jpg", "H2"),
        ];
        // Unrelated existing paths are not conflicts.
        let conflicts = find_name_conflicts(&candidates, &["/session/other.jpg".to_string()]);
        assert!(conflicts.is_empty());
        assert!(find_name_conflicts(&[], &["/session/x.jpg".to_string()]).is_empty());
    }

    #[test]
    fn conflicts_skip_invalid_basenames() {
        let candidates = vec![
            candidate_named("empty", "/in/a.jpg", "", "H1"),
            candidate_named("dotdot", "/in/b.jpg", "..", "H2"),
            candidate_named("ok", "/in/c.jpg", "c.jpg", "H3"),
        ];
        assert!(find_name_conflicts(&candidates, &[]).is_empty());
    }

    #[test]
    fn invalid_file_names_rejected_with_dedicated_reason() {
        let request = import_request(vec![
            candidate_named("empty", "/in/a.jpg", "", "H1"),
            candidate_named("dot", "/in/b.jpg", ".", "H2"),
            candidate_named("dotdot", "/in/c.jpg", "..", "H3"),
            candidate_named("trailing", "/in/d.jpg", "photos/", "H4"),
            candidate_named("ok", "/in/e.jpg", "e.jpg", "H5"),
        ]);
        let report = import_images(&request, &[], &[]).expect("import runs");
        // Only the well-named file imports; invalid names never reach
        // conflict or duplicate detection.
        assert_eq!(report.imported_image_ids, vec!["ok".to_string()]);
        assert!(report.duplicates.is_empty());
        assert!(report.name_conflicts.is_empty());
        assert_eq!(
            report
                .rejected
                .iter()
                .map(|r| r.reason.clone())
                .collect::<Vec<_>>(),
            vec![
                ImportError::InvalidFileName {
                    path: "/in/a.jpg".to_string(),
                },
                ImportError::InvalidFileName {
                    path: "/in/b.jpg".to_string(),
                },
                ImportError::InvalidFileName {
                    path: "/in/c.jpg".to_string(),
                },
                ImportError::InvalidFileName {
                    path: "/in/d.jpg".to_string(),
                },
            ]
        );
    }
}
