#![allow(dead_code)]

//! RGB image import (Module 1).
//!
//! [`import_images`] is the entry point. [`find_duplicate`] is a shared
//! helper other modules call through the `module_01` re-exports. [`flag_incomplete_metadata`] is the second phase:
//! after [`import_images`] decides what gets imported, the I/O layer extracts
//! metadata for each imported id (impure) and calls it to flag images with
//! missing required fields. Flagged images stay imported — they are reported
//! as "incomplete metadata", never rejected.
//! None of these functions read the disk. I/O fills [`ImportCandidate`]
//! (header, hash, id) before they run, and applies [`ImportReport`] afterwards.

use std::collections::{hash_map::Entry, BTreeMap, HashMap};
use std::path::Path;

use unicode_normalization::UnicodeNormalization;

use crate::models::image::ImageFormat;
use crate::models::image::{
    ContentHash, DuplicateFlag, ImageMetadata, ImportCandidate, ImportError, ImportReport,
    ImportRequest, IncompleteMetadataFlag, RejectedFile, SessionTarget,
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
pub(crate) fn classify_file(candidate: &ImportCandidate) -> Result<ImageFormat, ImportError> {
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
            MetadataError::Io(_)
            | MetadataError::MalformedExif(_)
            | MetadataError::MalformedXmp(_)
            | MetadataError::PreviewUnavailable { .. },
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
pub(crate) fn dest_basename(name: &str) -> Option<&str> {
    let base = basename_of(name);
    if base.is_empty() || base == "." || base == ".." {
        None
    } else {
        Some(base)
    }
}

pub(crate) fn extension_of(file_name: &str) -> String {
    basename_of(file_name)
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim_start_matches('.').to_ascii_lowercase())
        .unwrap_or_default()
}

/// How the destination filesystem decides that two file names are the same
/// file. The caller knows the target; the choice is per platform because the
/// filesystems genuinely differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameFolding {
    /// Names are the same only when byte-identical (most Linux filesystems).
    Exact,
    /// NTFS: names that differ only by case are the same (per-character
    /// simple upper-casing, as the volume's `$UpCase` table does). No
    /// Unicode normalization: composed and decomposed accents are distinct
    /// files.
    Ntfs,
    /// APFS/HFS+ in their default case-insensitive mode: names that differ
    /// only by case or by canonical (NFC/NFD) form are the same. Case folding
    /// is simple, per character, so `ß` stays distinct from `ss`.
    Apfs,
}

/// The name-folding policy of the filesystem this build writes to.
///
/// The import destination is the app's library folder inside the OS
/// app-data directory, which sits on the platform's default filesystem:
/// NTFS on Windows, APFS on macOS, and a case-sensitive Linux filesystem
/// elsewhere (where `Exact` is the only safe choice, since the build cannot
/// know whether the data directory is on ext4, btrfs, or a case-insensitive
/// mount).
///
/// # Purity
///
/// Pure, total (never fails).
pub(crate) fn folding_for_os() -> NameFolding {
    if cfg!(target_os = "windows") {
        NameFolding::Ntfs
    } else if cfg!(target_os = "macos") {
        NameFolding::Apfs
    } else {
        NameFolding::Exact
    }
}

/// Pairs `.jpg`/`.jpeg` candidates with the `.dng` of the same stem.
///
/// # Purity
///
/// Pure. Does not read the disk.
///
/// # Returns
///
/// `(jpeg_candidate_id, dng_candidate_id)` for every stem that has exactly
/// one JPEG and one DNG, ordered by folded stem. A stem with two JPEGs or
/// two DNGs is ambiguous and produces no pair: guessing would attach the
/// wrong raw file to the wrong preview. `folding` decides which stems count
/// as the same name, matching the destination filesystem.
pub(crate) fn pair_raw_siblings(
    candidates: &[ImportCandidate],
    folding: NameFolding,
) -> Vec<(String, String)> {
    #[derive(Default)]
    struct Group<'a> {
        jpgs: Vec<&'a ImportCandidate>,
        dngs: Vec<&'a ImportCandidate>,
    }

    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for candidate in candidates {
        let Some(base) = dest_basename(&candidate.file_name) else {
            continue;
        };
        let Some((stem, extension)) = base.rsplit_once('.') else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let group = groups.entry(fold_name(stem, folding)).or_default();
        match extension.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => group.jpgs.push(candidate),
            "dng" => group.dngs.push(candidate),
            _ => {}
        }
    }

    groups
        .into_values()
        .filter_map(
            |group| match (group.jpgs.as_slice(), group.dngs.as_slice()) {
                ([jpeg], [dng]) => Some((jpeg.id.clone(), dng.id.clone())),
                _ => None,
            },
        )
        .collect()
}

/// Maps a character through its single-character case mapping, leaving it
/// unchanged when the mapping expands to several characters (`ß`, `İ`).
fn simple_case(c: char, mapping: impl Fn(char) -> String) -> char {
    let mapped = mapping(c);
    let mut chars = mapped.chars();
    match (chars.next(), chars.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

/// The comparison key of a basename under `folding`.
fn fold_name(base: &str, folding: NameFolding) -> String {
    match folding {
        NameFolding::Exact => base.to_string(),
        NameFolding::Ntfs => base
            .chars()
            .map(|c| simple_case(c, |c| c.to_uppercase().collect()))
            .collect(),
        NameFolding::Apfs => base
            .nfd()
            .map(|c| simple_case(c, |c| c.to_lowercase().collect()))
            .collect(),
    }
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

/// Composes classification and duplicate checks into an [`ImportReport`].
///
/// Pure. Does not copy files, query SQLite, or touch the clock. `candidates`
/// are already prepared by the I/O layer, and the caller that owns the
/// database connection checks that an `Existing` session id is real — this
/// function has no session list, so it never produces
/// [`ImportError::SessionNotFound`].
///
/// Duplicate scope is the project. Persisted `known` hashes are checked
/// first, then the batch itself: the first claimant of a content (batch
/// order) is its representative and later identical files are duplicates of
/// it.
///
/// Destination names are not decided here: the I/O layer picks the first
/// free name when it persists each staged copy, so two files with the same
/// basename both import (the second as `name (2).ext`) and a race between
/// two imports is resolved at persist time rather than guessed at plan time.
///
/// A candidate with an empty `content_hash` is rejected with
/// [`ImportError::MissingContentHash`], and persisted `known` entries with an
/// empty hash are ignored: an empty hash means the content was never hashed,
/// and treating it as a value would make unrelated files duplicates of each
/// other. Names that yield no basename (empty, `.`, `..`, trailing
/// separator) are rejected with [`ImportError::InvalidFileName`].
///
/// # Errors
///
/// [`ImportError::NoImagesFound`] if `candidates` is empty. A batch where
/// every file is rejected or duplicated is still `Ok` — per-file failures
/// live in `rejected` and `duplicates`, not this `Err`.
pub(crate) fn import_images(
    request: &ImportRequest,
    known: &[(String, ContentHash)],
) -> Result<ImportReport, ImportError> {
    if request.candidates.is_empty() {
        return Err(ImportError::NoImagesFound);
    }
    let session_id = match &request.session {
        SessionTarget::Existing { id } | SessionTarget::New { id, .. } => id.clone(),
    };

    let mut duplicates = Vec::new();
    let mut rejected = Vec::new();
    let mut representatives = Vec::new();

    // Hash → image id, seeded from the persisted images (first entry wins).
    // Batch candidates register themselves as they are accepted, so the
    // first claimant of a content wins and later identical files are
    // duplicates of it.
    let mut known_hashes: HashMap<&str, &str> =
        HashMap::with_capacity(known.len() + request.candidates.len());
    for (image_id, hash) in known {
        // An empty stored hash means "never hashed", not a shared content.
        if hash.0.is_empty() {
            continue;
        }
        known_hashes
            .entry(hash.0.as_str())
            .or_insert(image_id.as_str());
    }

    let mut claimed: HashMap<&str, &str> = HashMap::with_capacity(request.candidates.len());
    for candidate in &request.candidates {
        if dest_basename(&candidate.file_name).is_none() {
            rejected.push(RejectedFile {
                source_path: candidate.source_path.clone(),
                reason: ImportError::InvalidFileName {
                    path: candidate.source_path.clone(),
                },
            });
            continue;
        }
        if candidate.content_hash.0.is_empty() {
            rejected.push(RejectedFile {
                source_path: candidate.source_path.clone(),
                reason: ImportError::MissingContentHash {
                    path: candidate.source_path.clone(),
                },
            });
            continue;
        }
        if let Err(reason) = classify_file(candidate) {
            rejected.push(RejectedFile {
                source_path: candidate.source_path.clone(),
                reason,
            });
            continue;
        }

        let hash = candidate.content_hash.0.as_str();
        let existing = known_hashes
            .get(hash)
            .copied()
            .or_else(|| claimed.get(hash).copied());
        if let Some(existing_image_id) = existing {
            duplicates.push(DuplicateFlag {
                source_path: candidate.source_path.clone(),
                existing_image_id: existing_image_id.to_string(),
                content_hash: candidate.content_hash.clone(),
            });
            continue;
        }
        if let Entry::Vacant(entry) = claimed.entry(hash) {
            entry.insert(candidate.id.as_str());
            representatives.push(candidate.clone());
        }
    }

    Ok(ImportReport {
        session_id,
        imported_image_ids: representatives
            .iter()
            .map(|candidate| candidate.id.clone())
            .collect(),
        duplicates,
        rejected,
        not_analysis_ready: Vec::new(),
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
/// import_images(request, known)
///   -> I/O copies files + extracts metadata per imported id (impure)
///   -> flag_incomplete_metadata(imported_candidates, metadata)
/// ```
///
/// `metadata` maps `ImportCandidate.id` to its extracted [`ImageMetadata`].
/// A candidate with **no entry** was never extracted (the I/O layer skipped
/// it): completeness cannot be judged, so no flag is emitted. This keeps
/// skipped files from being reported as "incomplete metadata". When extraction was attempted but failed
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
    fn identical_files_sharing_a_name_are_duplicates_not_a_conflict() {
        // The same photo picked from two folders: no rename prompt, the
        // second is a plain duplicate of the first.
        let request = import_request(vec![
            candidate_named("a", "/in/a/x.jpg", "x.jpg", "H"),
            candidate_named("b", "/in/b/x.jpg", "x.jpg", "H"),
        ]);
        let report = import_images(&request, &[]).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["a".to_string()]);
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].source_path, "/in/b/x.jpg");
        assert_eq!(report.duplicates[0].existing_image_id, "a");
    }

    #[test]
    fn known_duplicate_with_conflicting_name_is_a_duplicate() {
        let request = import_request(vec![candidate_named("a", "/in/a.jpg", "x.jpg", "H")]);
        let known = vec![("old".to_string(), ContentHash("H".to_string()))];
        let report = import_images(&request, &known).expect("import runs");
        assert!(report.imported_image_ids.is_empty());
        assert_eq!(report.duplicates[0].existing_image_id, "old");
    }

    #[test]
    fn first_claimant_wins_without_conflicts() {
        let request = import_request(vec![
            candidate_named("x", "/in/x.jpg", "x.jpg", "H"),
            candidate_named("y", "/in/y.jpg", "y.jpg", "H"),
        ]);
        let report = import_images(&request, &[]).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["x".to_string()]);
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].existing_image_id, "x".to_string());
    }

    #[test]
    fn identical_later_file_is_a_duplicate_even_with_a_conflicting_name() {
        // The earlier claimant owns the hash. A (conflicting name, identical
        // content) is a duplicate of B, so it is neither held back nor a
        // conflict candidate.
        let request = import_request(vec![
            candidate_named("b", "/in/b.jpg", "y.jpg", "H"),
            candidate_named("a", "/in/a.jpg", "x.jpg", "H"),
        ]);
        let report = import_images(&request, &[]).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["b".to_string()]);
        assert_eq!(report.duplicates.len(), 1);
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
        let report = import_images(&request, &[]).expect("import runs");
        // Only the well-named file imports; invalid names never reach
        // conflict or duplicate detection.
        assert_eq!(report.imported_image_ids, vec!["ok".to_string()]);
        assert!(report.duplicates.is_empty());
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

    #[test]
    fn empty_content_hash_is_rejected_not_deduplicated() {
        let request = import_request(vec![
            candidate_named("a", "/in/a.jpg", "a.jpg", ""),
            candidate_named("b", "/in/b.jpg", "b.jpg", ""),
            candidate_named("c", "/in/c.jpg", "c.jpg", "H"),
        ]);
        let report = import_images(&request, &[]).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["c".to_string()]);
        assert!(report.duplicates.is_empty());
        assert_eq!(
            report
                .rejected
                .iter()
                .map(|r| r.reason.clone())
                .collect::<Vec<_>>(),
            vec![
                ImportError::MissingContentHash {
                    path: "/in/a.jpg".to_string()
                },
                ImportError::MissingContentHash {
                    path: "/in/b.jpg".to_string()
                },
            ]
        );
    }

    #[test]
    fn empty_known_hash_does_not_match_anything() {
        let request = import_request(vec![candidate_named("a", "/in/a.jpg", "a.jpg", "H")]);
        let known = vec![("old".to_string(), ContentHash(String::new()))];
        let report = import_images(&request, &known).expect("import runs");
        assert_eq!(report.imported_image_ids, vec!["a".to_string()]);
        assert!(report.duplicates.is_empty());
    }

    #[test]
    fn pairs_same_stem_jpeg_and_dng() {
        let candidates = vec![
            candidate_named("j", "/in/DJI_1.JPG", "DJI_1.JPG", "h1"),
            candidate_named("d", "/in/DJI_1.DNG", "DJI_1.DNG", "h2"),
            candidate_named("lonely", "/in/DJI_2.JPG", "DJI_2.JPG", "h3"),
        ];
        assert_eq!(
            pair_raw_siblings(&candidates, NameFolding::Exact),
            vec![("j".to_string(), "d".to_string())]
        );
    }

    #[test]
    fn pairing_respects_folding_and_ignores_ambiguous_stems() {
        let candidates = vec![
            candidate_named("upper", "/in/IMG_1.JPG", "IMG_1.JPG", "h1"),
            candidate_named("lower", "/in/img_1.dng", "img_1.dng", "h2"),
            candidate_named("a", "/in/two.jpg", "two.jpg", "h3"),
            candidate_named("b", "/in/two.jpeg", "two.jpeg", "h4"),
            candidate_named("c", "/in/two.dng", "two.dng", "h5"),
        ];
        assert!(pair_raw_siblings(&candidates, NameFolding::Exact).is_empty());
        assert_eq!(
            pair_raw_siblings(&candidates, NameFolding::Ntfs),
            vec![("upper".to_string(), "lower".to_string())],
            "two JPEGs for one stem stay unpaired"
        );
    }

    #[test]
    fn pairing_requires_the_same_stem_not_just_the_same_folder() {
        let candidates = vec![
            candidate_named("j", "/in/A.JPG", "A.JPG", "h1"),
            candidate_named("d", "/in/B.DNG", "B.DNG", "h2"),
        ];
        assert!(pair_raw_siblings(&candidates, NameFolding::Exact).is_empty());
    }

    #[test]
    fn folding_for_os_matches_the_platform_default() {
        let expected = if cfg!(target_os = "windows") {
            NameFolding::Ntfs
        } else if cfg!(target_os = "macos") {
            NameFolding::Apfs
        } else {
            NameFolding::Exact
        };
        assert_eq!(folding_for_os(), expected);
    }
}
