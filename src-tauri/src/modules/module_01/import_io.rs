//! Copy-on-import: the impure half of RGB image import.
//!
//! [`crate::modules::module_01::image_importer::import_images`] decides *what*
//! to import without touching the disk or the database. This module applies
//! that plan: it stages the picked files (one read each, hashed while the
//! temporary copy is written), renames the accepted ones into the app-owned
//! library folder, reads metadata from **the copy**, records the row, and
//! reports per-file failures without aborting the batch.
//!
//! ```text
//! scan_sources                     (hash only; no copy yet)
//!   -> import_images + known_hashes_for_project  (pure plan)
//!   -> copy approved files         (first free name, one file at a time)
//!   -> metadata from the copy      (JPEG / DNG)
//!   -> one transaction: insert row(s) + recalc session range
//! ```
//!
//! # Why copy
//!
//! A stored image must survive the user moving or deleting the folder they
//! imported from. The row's `file_path` therefore points at the project's
//! copy, never at the original. The source is opened read-only; nothing is
//! moved or modified.
//!
//! # Storage layout
//!
//! ```text
//! <library_root>/projects/<project_id>/sessions/<session_id>/images/<file_name>
//! ```
//!
//! `library_root` is resolved once at startup (see `crate::run`) and lives in
//! `AppState`. `file_path` stores the canonical absolute path of the copy.
//!
//! # Guarantees
//!
//! * A failed copy leaves no file and no row.
//! * A failed insert deletes the copy, so no orphan file remains.
//! * Hashes are computed before any copy. A duplicate is never written.
//!   An approved file is read again while it is copied, and the copy's hash
//!   must still match the plan.
//! * An existing destination is never overwritten
//!   ([`ImportError::DestinationExists`]).
//! * A destination name that is already taken is imported under the first
//!   free ` (n)` suffix instead of being skipped: drone cameras reuse
//!   `DJI_0001.JPG` across flights. A JPEG/DNG pair shares one suffix, and a
//!   name claimed by a concurrent import is retried rather than failed.
//! * Duplicate detection is re-checked inside the insert transaction, so two
//!   imports running at once cannot store the same bytes twice.
//! * One bad file never aborts the rest of the batch.
//! * Picked paths that are not regular files, or whose extension is not
//!   jpeg/dng, are rejected without being opened: importing a FIFO, a device
//!   node, or a huge video never reads it.
//!
//! # DNG handling
//!
//! No DNG is decoded or converted. A `.dng` with a same-stem `.jpg`/`.jpeg`
//! is recorded as a second row linked through `raw_sibling_id`; the JPEG is
//! its analysis input. A DNG with no JPEG is still imported, but
//! `analysis_ready = 0` and its id is listed in
//! [`ImportReport::not_analysis_ready`](crate::models::image::ImportReport::not_analysis_ready),
//! because Module 1 cannot hand raw pixels to a plugin.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use uuid::Uuid;

use crate::models::image::{
    ContentHash, DuplicateFlag, ImageFormat, ImageMetadata, ImportCandidate, ImportError,
    ImportReport, ImportRequest, RejectedFile, SessionTarget,
};
use crate::models::Image;
use crate::plugin_manager::error::CommandError;
use crate::plugin_manager::payload::ImageMetadata as PluginImageMetadata;

use super::error::SessionError;
use super::flight_session::{
    get_image, get_session, image_from_metadata, insert_images_and_recalculate,
    known_hashes_for_project, ExistingContent,
};
use super::image_importer::{
    classify_file, dest_basename, extension_of, folding_for_os, import_images, pair_raw_siblings,
    NameFolding,
};
use super::metadata_extractor::{extract_metadata, read_dng, to_plugin_metadata};

/// Read/write chunk size for hashing and copying.
const COPY_BUFFER_BYTES: usize = 64 * 1024;

/// Bytes needed for the JPEG/DNG magic sniff, matching `detect_format`.
const HEADER_BYTES: usize = 16;

/// Why a whole import run failed (as opposed to one file inside it).
///
/// Per-file problems live in [`ImportReport::rejected`]; this type is only
/// for failures that stop the run before or between files (session lookup,
/// planner rejection, a poisoned database lock).
#[derive(Debug)]
pub(crate) enum ImportFailure {
    /// The plan itself was rejected (e.g. no candidates, unknown session).
    Import(ImportError),
    /// A database or storage operation failed.
    Session(SessionError),
}

impl From<ImportError> for ImportFailure {
    fn from(err: ImportError) -> Self {
        Self::Import(err)
    }
}

impl From<SessionError> for ImportFailure {
    fn from(err: SessionError) -> Self {
        Self::Session(err)
    }
}

impl From<ImportFailure> for CommandError {
    fn from(err: ImportFailure) -> Self {
        match err {
            ImportFailure::Import(err) => CommandError {
                code: err.code().to_string(),
                message: err.to_string(),
            },
            ImportFailure::Session(err) => CommandError {
                code: err.code().to_string(),
                message: err.to_string(),
            },
        }
    }
}

/// The folder a session's copied images live in.
///
/// Pure, total (never fails). `library_root`, `project_id`, and `session_id`
/// are joined as path components; ids are UUID strings produced by this
/// crate, so they cannot climb out of the root.
pub(crate) fn session_images_dir(
    library_root: &Path,
    project_id: &str,
    session_id: &str,
) -> PathBuf {
    library_root
        .join("projects")
        .join(project_id)
        .join("sessions")
        .join(session_id)
        .join("images")
}

/// Runs one copy-on-import over picked files or a picked folder.
///
/// # Arguments
///
/// * `db` — shared SQLite connection. The lock is taken only for the hash
///   query and for the insert transaction; staging and recording run with the
///   lock released.
/// * `library_root` — app-owned image library. The session folder below it is
///   created on demand.
/// * `session_id` — destination session; must exist.
/// * `sources` — absolute paths of picked files, or folders when
///   `from_folder` is `true`.
/// * `from_folder` — scan each source folder (non-recursive) for
///   `.jpg`/`.jpeg`/`.dng` instead of treating sources as files.
/// * `on_progress` — called after each copied file with
///   `(done, total, destination_path)`.
///
/// # Purity
///
/// Impure — reads and writes the filesystem and the database.
///
/// # Errors
///
/// [`ImportFailure::Import`] when the request has no importable candidates
/// ([`ImportError::NoImagesFound`]) or the session is unknown. Individual
/// files that cannot be staged, copied, or recorded are reported in the
/// result's `rejected` list, not as an error.
pub(crate) fn run_import(
    db: &Mutex<Connection>,
    library_root: &Path,
    session_id: Uuid,
    sources: &[String],
    from_folder: bool,
    mut on_progress: impl FnMut(usize, usize, &str),
) -> Result<ImportReport, ImportFailure> {
    let session = {
        let conn = lock_db(db)?;
        get_session(&conn, session_id)?
    };
    let dest_dir = session_images_dir(
        library_root,
        &session.project_id.to_string(),
        &session_id.to_string(),
    );
    fs::create_dir_all(&dest_dir).map_err(|err| {
        SessionError::Storage(format!("could not create '{}': {err}", dest_dir.display()))
    })?;

    // Hash first, copy later. A re-import of a folder already in the project
    // must not write a temporary copy of every duplicate before throwing it
    // away, and holding one open file per image blows the macOS default of
    // 256 file descriptors.
    let mut rejected = Vec::new();
    let mut candidates = Vec::new();
    for entry in scan_sources(sources, from_folder) {
        match entry {
            Ok(candidate) => candidates.push(candidate),
            Err(rejection) => rejected.push(rejection),
        }
    }

    let request = ImportRequest {
        from_folder,
        session: SessionTarget::Existing {
            id: session_id.to_string(),
        },
        candidates,
    };
    let known = {
        let conn = lock_db(db)?;
        known_hashes_for_project(&conn, session.project_id)?
    };
    let mut report = import_images(&request, &known)?;
    let mut all_rejected = rejected;
    all_rejected.append(&mut report.rejected);
    report.rejected = all_rejected;

    import_planned(
        db,
        &dest_dir,
        session_id,
        &request,
        report,
        &mut on_progress,
    )
}

/// Copies and records every candidate the plan approved.
///
/// Splits the result back into [`ImportReport`] fields: image ids become the
/// persisted ids (not candidate ids), persist and record failures become
/// `rejected`, a race-detected duplicate becomes a `duplicates` entry, and
/// unpaired DNGs are collected in `not_analysis_ready`.
fn import_planned(
    db: &Mutex<Connection>,
    dest_dir: &Path,
    session_id: Uuid,
    request: &ImportRequest,
    report: ImportReport,
    on_progress: &mut impl FnMut(usize, usize, &str),
) -> Result<ImportReport, ImportFailure> {
    let mut report = report;
    let candidates: std::collections::HashMap<&str, &ImportCandidate> = request
        .candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect();
    let approved: std::collections::HashSet<&str> = report
        .imported_image_ids
        .iter()
        .map(String::as_str)
        .collect();
    let pairs = pair_raw_siblings(&request.candidates, folding_for_os());
    let mut partner_of: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for (jpeg_id, dng_id) in &pairs {
        if approved.contains(jpeg_id.as_str()) && approved.contains(dng_id.as_str()) {
            partner_of.insert(jpeg_id, dng_id);
            partner_of.insert(dng_id, jpeg_id);
        }
    }

    let work: Vec<&ImportCandidate> = report
        .imported_image_ids
        .iter()
        .filter_map(|id| candidates.get(id.as_str()).copied())
        // A paired DNG is imported together with its JPEG when that JPEG is
        // reached; skip its standalone turn so the pair shares one insertion.
        .filter(|candidate| {
            let is_dng = matches!(classify_file(candidate), Ok(ImageFormat::Dng));
            let paired_dng = is_dng
                && partner_of
                    .get(candidate.id.as_str())
                    .and_then(|id| candidates.get(id))
                    .is_some_and(|partner| matches!(classify_file(partner), Ok(ImageFormat::Jpeg)));
            !paired_dng
        })
        .collect();

    let total = report.imported_image_ids.len();
    let mut done = 0usize;
    let mut imported_ids = Vec::new();
    let mut not_analysis_ready = Vec::new();
    let mut state = RunState {
        image_ids: std::collections::HashMap::new(),
        rejected: std::mem::take(&mut report.rejected),
        duplicates: std::mem::take(&mut report.duplicates),
    };

    for candidate in work {
        let partner = partner_of
            .get(candidate.id.as_str())
            .and_then(|id| candidates.get(id))
            .copied();
        let mut scope = ImportScope {
            db,
            dest_dir,
            session_id,
            state: &mut state,
        };
        let rejected_before = scope.rejected_len();
        let duplicates_before = scope.duplicates_len();
        let outcome = match partner {
            Some(partner) => scope.pair(candidate, partner),
            None => scope.single(candidate).map(|imported| vec![imported]),
        };
        let rejected_added = scope.rejected_len() - rejected_before;
        let duplicates_added = scope.duplicates_len() - duplicates_before;

        // `done` counts candidates, not items: a stem pair is two files, so
        // progress stays coherent even when only one half is rejected or
        // turns out to be a duplicate.
        if let Ok(files) = outcome {
            for imported in &files {
                on_progress(done + 1, total, &imported.dest);
                done += 1;
                imported_ids.push(imported.image_id.clone());
                if imported.not_analysis_ready {
                    not_analysis_ready.push(imported.image_id.clone());
                }
            }
        }
        done += rejected_added;
        done += duplicates_added;
    }

    let RunState {
        image_ids,
        rejected,
        duplicates,
    } = state;
    report.session_id = session_id.to_string();
    report.imported_image_ids = imported_ids;
    report.not_analysis_ready = not_analysis_ready;
    report.rejected = rejected;
    report.duplicates = duplicates;
    remap_duplicate_ids(&mut report.duplicates, &image_ids);
    Ok(report)
}

/// One copied, recorded file.
struct ImportedFile {
    image_id: String,
    dest: String,
    not_analysis_ready: bool,
}

/// The basename a staged file should get, or an invalid-name error.
fn base_name_for(candidate: &ImportCandidate) -> Result<String, ImportError> {
    dest_basename(&candidate.file_name)
        .map(str::to_string)
        .ok_or_else(|| ImportError::InvalidFileName {
            path: candidate.source_path.clone(),
        })
}

/// `base` with ` (n)` inserted before its extension (`n == 1` is `base`).
fn suffixed_name(base: &str, n: u32) -> String {
    if n <= 1 {
        return base.to_string();
    }
    let path = Path::new(base);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    format!("{stem} ({n}){extension}")
}

/// How many suffixes to try before giving up on a destination name.
const MAX_SUFFIX: u32 = 10_000;

/// Accumulated results of a run, carried across helpers.
struct RunState {
    /// Candidate id -> persisted image id.
    image_ids: std::collections::HashMap<String, String>,
    rejected: Vec<RejectedFile>,
    duplicates: Vec<DuplicateFlag>,
}

/// Per-run context for the copy-and-record helpers.
struct ImportScope<'a> {
    db: &'a Mutex<Connection>,
    dest_dir: &'a Path,
    session_id: Uuid,
    state: &'a mut RunState,
}

impl ImportScope<'_> {
    /// Rejections recorded so far, for progress accounting.
    fn rejected_len(&self) -> usize {
        self.state.rejected.len()
    }

    /// Duplicates recorded so far, for progress accounting.
    fn duplicates_len(&self) -> usize {
        self.state.duplicates.len()
    }

    /// Copies and records one standalone image.
    fn single(&mut self, candidate: &ImportCandidate) -> Result<ImportedFile, ()> {
        let base = match base_name_for(candidate) {
            Ok(base) => base,
            Err(err) => {
                self.state.rejected.push(reject(candidate, err));
                return Err(());
            }
        };
        let format = match classify_file(candidate) {
            Ok(format) => format,
            Err(err) => {
                self.state.rejected.push(reject(candidate, err));
                return Err(());
            }
        };
        let copy = match persist_single(candidate, self.dest_dir, &base) {
            Ok(copy) => copy,
            Err(err) => {
                self.state.rejected.push(reject(candidate, err));
                return Err(());
            }
        };
        let metadata = metadata_from_copy(&copy.dest, format);
        let image_id = Uuid::new_v4();
        let ready = format == ImageFormat::Jpeg;
        let image = build_image(
            image_id,
            self.session_id,
            &copy,
            candidate,
            &metadata,
            None,
            ready,
        );
        match record_images(self.db, self.session_id, std::slice::from_ref(&image)) {
            Ok(Recorded::Inserted) => {}
            Ok(Recorded::Duplicate(existing)) => {
                remove_copy(&copy.dest);
                self.state.duplicates.push(DuplicateFlag {
                    source_path: candidate.source_path.clone(),
                    existing_image_id: existing.existing_image_id.to_string(),
                    content_hash: candidate.content_hash.clone(),
                });
                return Err(());
            }
            Err(err) => {
                remove_copy(&copy.dest);
                self.state.rejected.push(RejectedFile {
                    source_path: candidate.source_path.clone(),
                    reason: ImportError::RecordFailed {
                        path: candidate.source_path.clone(),
                        reason: err.to_string(),
                    },
                });
                return Err(());
            }
        }
        self.state
            .image_ids
            .insert(candidate.id.clone(), image_id.to_string());
        Ok(ImportedFile {
            image_id: image_id.to_string(),
            dest: copy.dest.display().to_string(),
            not_analysis_ready: !ready,
        })
    }

    /// Copies and records a JPEG/DNG stem pair in one insertion.
    ///
    /// The JPEG is inserted first, then the DNG, both linked through
    /// `raw_sibling_id`. If the DNG cannot be persisted, the JPEG is still
    /// imported alone (it is analysis-ready by itself); if the JPEG cannot be
    /// persisted, the DNG is imported alone as not-analysis-ready. A pair
    /// where one half turns out to be a duplicate is treated the same way:
    /// the other half is imported on its own.
    fn pair(
        &mut self,
        first: &ImportCandidate,
        second: &ImportCandidate,
    ) -> Result<Vec<ImportedFile>, ()> {
        let (jpeg, dng) = match classify_file(first) {
            Ok(ImageFormat::Jpeg) => (first, second),
            Ok(ImageFormat::Dng) => (second, first),
            Err(err) => {
                self.state.rejected.push(reject(first, err));
                return Err(());
            }
        };
        // Defensive: the planner only approves JPEG/DNG candidates, but
        // pairing is name-based, so re-check the partner's classification.
        if classify_file(jpeg) != Ok(ImageFormat::Jpeg)
            || classify_file(dng) != Ok(ImageFormat::Dng)
        {
            self.state.rejected.push(reject(
                dng,
                ImportError::UnsupportedFormat {
                    path: dng.source_path.clone(),
                    extension: extension_of(&dng.file_name),
                },
            ));
            return Err(());
        }

        let jpeg_base = match base_name_for(jpeg) {
            Ok(base) => base,
            Err(err) => {
                self.state.rejected.push(reject(jpeg, err));
                return self.single(dng).map(|imported| vec![imported]);
            }
        };
        let dng_base = match base_name_for(dng) {
            Ok(base) => base,
            Err(err) => {
                self.state.rejected.push(reject(dng, err));
                return self.single(jpeg).map(|imported| vec![imported]);
            }
        };

        let (jpeg_copy, dng_copy) =
            match persist_pair(jpeg, dng, self.dest_dir, &jpeg_base, &dng_base) {
                Ok(pair) => pair,
                Err(err) => {
                    // `persist_pair` never leaves a half-persisted pair behind.
                    // If the JPEG landed, import it alone; if it did not, the DNG
                    // can still be imported alone as not-analysis-ready.
                    if let Some(copy) = err.copy {
                        self.state.rejected.push(reject(dng, err.error));
                        return self
                            .record_single_copy(&copy, jpeg)
                            .map(|imported| vec![imported]);
                    }
                    self.state.rejected.push(reject(jpeg, err.error));
                    return self.single(dng).map(|imported| vec![imported]);
                }
            };

        // Read both copies, then let the DNG fill whatever tags the JPEG lacks
        // (a JPEG's EXIF is often thinner than the raw file's).
        let mut jpeg_metadata = metadata_from_copy(&jpeg_copy.dest, ImageFormat::Jpeg);
        let dng_metadata = metadata_from_copy(&dng_copy.dest, ImageFormat::Dng);
        fill_missing(&mut jpeg_metadata, &dng_metadata);

        let jpeg_id = Uuid::new_v4();
        let dng_id = Uuid::new_v4();
        let images = [
            build_image(
                jpeg_id,
                self.session_id,
                &jpeg_copy,
                jpeg,
                &jpeg_metadata,
                Some(dng_id),
                true,
            ),
            build_image(
                dng_id,
                self.session_id,
                &dng_copy,
                dng,
                &dng_metadata,
                Some(jpeg_id),
                true,
            ),
        ];
        match record_images(self.db, self.session_id, &images) {
            Ok(Recorded::Inserted) => {}
            Ok(Recorded::Duplicate(existing)) => {
                remove_copy(&jpeg_copy.dest);
                remove_copy(&dng_copy.dest);
                let source = if existing.index == 0 { jpeg } else { dng };
                let survivor = if existing.index == 0 { dng } else { jpeg };
                self.state.duplicates.push(DuplicateFlag {
                    source_path: source.source_path.clone(),
                    existing_image_id: existing.existing_image_id.to_string(),
                    content_hash: source.content_hash.clone(),
                });
                // The other half is unaffected by the duplicate: import it on
                // its own so no content is silently lost.
                return self.single(survivor).map(|imported| vec![imported]);
            }
            Err(err) => {
                remove_copy(&jpeg_copy.dest);
                remove_copy(&dng_copy.dest);
                self.state.rejected.push(RejectedFile {
                    source_path: jpeg.source_path.clone(),
                    reason: ImportError::RecordFailed {
                        path: jpeg.source_path.clone(),
                        reason: err.to_string(),
                    },
                });
                return Err(());
            }
        }

        self.state
            .image_ids
            .insert(jpeg.id.clone(), jpeg_id.to_string());
        self.state
            .image_ids
            .insert(dng.id.clone(), dng_id.to_string());
        Ok(vec![
            ImportedFile {
                image_id: jpeg_id.to_string(),
                dest: jpeg_copy.dest.display().to_string(),
                not_analysis_ready: false,
            },
            ImportedFile {
                image_id: dng_id.to_string(),
                dest: dng_copy.dest.display().to_string(),
                not_analysis_ready: false,
            },
        ])
    }

    /// Records an already-persisted JPEG as a standalone image.
    fn record_single_copy(
        &mut self,
        copy: &CopiedFile,
        candidate: &ImportCandidate,
    ) -> Result<ImportedFile, ()> {
        let metadata = metadata_from_copy(&copy.dest, ImageFormat::Jpeg);
        let image_id = Uuid::new_v4();
        let image = build_image(
            image_id,
            self.session_id,
            copy,
            candidate,
            &metadata,
            None,
            true,
        );
        match record_images(self.db, self.session_id, std::slice::from_ref(&image)) {
            Ok(Recorded::Inserted) => {}
            Ok(Recorded::Duplicate(existing)) => {
                remove_copy(&copy.dest);
                self.state.duplicates.push(DuplicateFlag {
                    source_path: candidate.source_path.clone(),
                    existing_image_id: existing.existing_image_id.to_string(),
                    content_hash: candidate.content_hash.clone(),
                });
                return Err(());
            }
            Err(err) => {
                remove_copy(&copy.dest);
                self.state.rejected.push(RejectedFile {
                    source_path: candidate.source_path.clone(),
                    reason: ImportError::RecordFailed {
                        path: candidate.source_path.clone(),
                        reason: err.to_string(),
                    },
                });
                return Err(());
            }
        }
        self.state
            .image_ids
            .insert(candidate.id.clone(), image_id.to_string());
        Ok(ImportedFile {
            image_id: image_id.to_string(),
            dest: copy.dest.display().to_string(),
            not_analysis_ready: false,
        })
    }
}

/// One verified copy of a source file.
#[derive(Debug)]
struct CopiedFile {
    /// Canonical absolute destination path.
    dest: PathBuf,
    /// SHA-256 of the copied bytes, lowercase hex.
    content_hash: String,
    /// Number of bytes copied.
    file_size: u64,
}

/// A persist failure that still knows whether the other half of a pair made
/// it to disk.
struct PairPersistError {
    error: ImportError,
    /// The persisted JPEG, when the pair failed after that half landed.
    copy: Option<CopiedFile>,
}

/// Copies `candidate` to `name` in `dest_dir`, hashing while it copies.
///
/// The temporary file is closed (`into_temp_path`) before it is renamed, so
/// a batch never holds one open descriptor per image. `persist_noclobber`
/// refuses to replace a name another import just claimed.
///
/// On [`ImportError::DestinationExists`] nothing was left behind, so the
/// caller can retry another suffix. A hash that no longer matches the plan
/// is a copy failure: the source changed between the hash pass and this copy.
fn copy_source_to(
    candidate: &ImportCandidate,
    dest_dir: &Path,
    name: &str,
) -> Result<CopiedFile, ImportError> {
    let source_path = candidate.source_path.clone();
    let dest = dest_dir.join(name);
    let mut temp = NamedTempFile::new_in(dest_dir).map_err(|err| ImportError::CopyFailed {
        path: source_path.clone(),
        reason: format!("could not create a temporary file: {err}"),
    })?;
    let mut source = fs::File::open(&source_path).map_err(|err| ImportError::CopyFailed {
        path: source_path.clone(),
        reason: err.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    let mut file_size = 0u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|err| ImportError::CopyFailed {
                path: source_path.clone(),
                reason: err.to_string(),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        temp.write_all(&buffer[..read])
            .map_err(|err| ImportError::CopyFailed {
                path: source_path.clone(),
                reason: err.to_string(),
            })?;
        file_size += read as u64;
    }
    temp.as_file()
        .sync_all()
        .map_err(|err| ImportError::CopyFailed {
            path: source_path.clone(),
            reason: err.to_string(),
        })?;
    let content_hash = hex(&hasher.finalize());
    if !content_hash.eq_ignore_ascii_case(&candidate.content_hash.0) {
        return Err(ImportError::CopyFailed {
            path: source_path,
            reason: "file changed while it was being imported".to_string(),
        });
    }
    // Close the descriptor before the rename. macOS GUI apps default to 256
    // open files; a drone folder is larger than that.
    let temp_path = temp.into_temp_path();
    if let Err(err) = temp_path.persist_noclobber(&dest) {
        return Err(if err.error.kind() == std::io::ErrorKind::AlreadyExists {
            ImportError::DestinationExists {
                path: dest.display().to_string(),
            }
        } else {
            ImportError::CopyFailed {
                path: source_path,
                reason: err.error.to_string(),
            }
        });
    }

    let canonical = dunce::canonicalize(&dest).map_err(|err| {
        remove_copy(&dest);
        ImportError::CopyFailed {
            path: candidate.source_path.clone(),
            reason: err.to_string(),
        }
    })?;
    Ok(CopiedFile {
        dest: canonical,
        content_hash,
        file_size,
    })
}

/// Copies one source under the first free `name (n)` suffix.
///
/// Names are decided here, not in the plan, so two imports that race for the
/// same suffix do not fail: each tries the next one until a rename succeeds.
fn persist_single(
    candidate: &ImportCandidate,
    dest_dir: &Path,
    base: &str,
) -> Result<CopiedFile, ImportError> {
    let mut last = dest_dir.join(base);
    for suffix in 1..=MAX_SUFFIX {
        let name = suffixed_name(base, suffix);
        last = dest_dir.join(&name);
        match copy_source_to(candidate, dest_dir, &name) {
            Err(ImportError::DestinationExists { .. }) => continue,
            result => return result,
        }
    }
    Err(ImportError::DestinationExists {
        path: last.display().to_string(),
    })
}

/// Moves `from` to `to` without replacing a file that appeared in between.
///
/// `rename` replaces its target. `hard_link` fails if the target exists, so
/// a concurrent import that just claimed `to` keeps its file.
fn move_without_replace(from: &Path, to: &Path) -> Result<(), ImportError> {
    match fs::hard_link(from, to) {
        Ok(()) => {
            remove_copy(from);
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(ImportError::DestinationExists {
                path: to.display().to_string(),
            })
        }
        Err(err) => Err(ImportError::CopyFailed {
            path: from.display().to_string(),
            reason: err.to_string(),
        }),
    }
}

/// Copies a JPEG/DNG pair under one shared suffix.
///
/// The two stems must stay equal, or the link between them breaks: if only
/// one of `DJI_0001.JPG`/`DJI_0001.DNG` needs a suffix, both get it. If the
/// DNG's name is claimed between the two copies, the persisted JPEG is moved
/// with [`move_without_replace`] and the DNG follows it.
fn persist_pair(
    jpeg: &ImportCandidate,
    dng: &ImportCandidate,
    dest_dir: &Path,
    jpeg_base: &str,
    dng_base: &str,
) -> Result<(CopiedFile, CopiedFile), PairPersistError> {
    let mut suffix = 1;
    let jpeg_copy = loop {
        if suffix > MAX_SUFFIX {
            return Err(PairPersistError {
                error: ImportError::DestinationExists {
                    path: dest_dir
                        .join(suffixed_name(jpeg_base, suffix))
                        .display()
                        .to_string(),
                },
                copy: None,
            });
        }
        let name = suffixed_name(jpeg_base, suffix);
        match copy_source_to(jpeg, dest_dir, &name) {
            Ok(copy) => break copy,
            Err(ImportError::DestinationExists { .. }) => suffix += 1,
            Err(error) => {
                return Err(PairPersistError { error, copy: None });
            }
        }
    };

    let mut jpeg_copy = jpeg_copy;
    loop {
        if suffix > MAX_SUFFIX {
            remove_copy(&jpeg_copy.dest);
            return Err(PairPersistError {
                error: ImportError::DestinationExists {
                    path: dest_dir
                        .join(suffixed_name(dng_base, suffix))
                        .display()
                        .to_string(),
                },
                copy: None,
            });
        }
        let name = suffixed_name(dng_base, suffix);
        match copy_source_to(dng, dest_dir, &name) {
            Ok(dng_copy) => return Ok((jpeg_copy, dng_copy)),
            Err(ImportError::DestinationExists { .. }) => loop {
                suffix += 1;
                if suffix > MAX_SUFFIX {
                    remove_copy(&jpeg_copy.dest);
                    return Err(PairPersistError {
                        error: ImportError::DestinationExists {
                            path: dest_dir
                                .join(suffixed_name(dng_base, suffix))
                                .display()
                                .to_string(),
                        },
                        copy: None,
                    });
                }
                let next = dest_dir.join(suffixed_name(jpeg_base, suffix));
                match move_without_replace(&jpeg_copy.dest, &next) {
                    Ok(()) => {
                        jpeg_copy = match dunce::canonicalize(&next) {
                            Ok(dest) => CopiedFile { dest, ..jpeg_copy },
                            Err(err) => {
                                return Err(PairPersistError {
                                    error: ImportError::CopyFailed {
                                        path: file_label(&next),
                                        reason: err.to_string(),
                                    },
                                    copy: Some(jpeg_copy),
                                });
                            }
                        };
                        break;
                    }
                    Err(ImportError::DestinationExists { .. }) => continue,
                    Err(error) => {
                        return Err(PairPersistError {
                            error,
                            copy: Some(jpeg_copy),
                        });
                    }
                }
            },
            Err(error) => {
                return Err(PairPersistError {
                    error,
                    copy: Some(jpeg_copy),
                });
            }
        }
    }
}

/// Reads metadata from a copied file.
///
/// A failed extraction is not a failed import: the image is recorded with
/// default (all-`None`) metadata and shows up as "incomplete metadata". The
/// format is set from the classifier, never from the parser, so a JPEG with
/// unreadable tags is still a JPEG.
fn metadata_from_copy(path: &Path, format: ImageFormat) -> ImageMetadata {
    let extracted = match format {
        ImageFormat::Jpeg => extract_metadata(path),
        ImageFormat::Dng => read_dng(path),
    };
    match extracted {
        Ok(mut metadata) => {
            metadata.format = Some(format);
            metadata
        }
        Err(_) => ImageMetadata {
            format: Some(format),
            ..ImageMetadata::default()
        },
    }
}

/// Fills every absent optional field of `dst` from `src`.
///
/// Pure, total. Present values are never overwritten, so the JPEG's own tags
/// win and only gaps are filled from the DNG.
fn fill_missing(dst: &mut ImageMetadata, src: &ImageMetadata) {
    fn fill<T: Clone>(dst: &mut Option<T>, src: &Option<T>) {
        if dst.is_none() {
            *dst = src.clone();
        }
    }
    fill(&mut dst.gps_latitude, &src.gps_latitude);
    fill(&mut dst.gps_longitude, &src.gps_longitude);
    fill(&mut dst.gps_altitude_m, &src.gps_altitude_m);
    fill(&mut dst.absolute_altitude_m, &src.absolute_altitude_m);
    fill(&mut dst.date_time_original, &src.date_time_original);
    fill(&mut dst.width, &src.width);
    fill(&mut dst.height, &src.height);
    fill(&mut dst.format, &src.format);
    fill(&mut dst.make, &src.make);
    fill(&mut dst.camera_model_name, &src.camera_model_name);
    fill(&mut dst.exposure_time_s, &src.exposure_time_s);
    fill(&mut dst.f_number, &src.f_number);
    fill(&mut dst.iso, &src.iso);
    fill(&mut dst.focal_length_mm, &src.focal_length_mm);
    fill(&mut dst.focal_length_35mm, &src.focal_length_35mm);
    fill(&mut dst.flash, &src.flash);
    fill(&mut dst.white_balance, &src.white_balance);
    fill(&mut dst.metering_mode, &src.metering_mode);
    fill(&mut dst.exposure_mode, &src.exposure_mode);
    fill(&mut dst.digital_zoom_ratio, &src.digital_zoom_ratio);
    fill(&mut dst.color_space, &src.color_space);
    fill(&mut dst.orientation, &src.orientation);
    fill(&mut dst.relative_altitude_m, &src.relative_altitude_m);
    fill(&mut dst.gimbal_roll_degree, &src.gimbal_roll_degree);
    fill(&mut dst.gimbal_yaw_degree, &src.gimbal_yaw_degree);
    fill(&mut dst.gimbal_pitch_degree, &src.gimbal_pitch_degree);
    fill(&mut dst.flight_roll_degree, &src.flight_roll_degree);
    fill(&mut dst.flight_yaw_degree, &src.flight_yaw_degree);
    fill(&mut dst.flight_pitch_degree, &src.flight_pitch_degree);
    fill(&mut dst.flight_x_speed, &src.flight_x_speed);
    fill(&mut dst.flight_y_speed, &src.flight_y_speed);
    fill(&mut dst.flight_z_speed, &src.flight_z_speed);
}

/// Builds the persisted record for one copied file.
fn build_image(
    image_id: Uuid,
    session_id: Uuid,
    copy: &CopiedFile,
    candidate: &ImportCandidate,
    metadata: &ImageMetadata,
    raw_sibling_id: Option<Uuid>,
    analysis_ready: bool,
) -> Image {
    let mut image = image_from_metadata(
        image_id,
        session_id,
        copy.dest.display().to_string(),
        metadata,
    );
    image.source_path = Some(candidate.source_path.clone());
    image.content_hash = Some(copy.content_hash.clone());
    image.file_size = Some(copy.file_size);
    image.raw_sibling_id = raw_sibling_id;
    image.analysis_ready = Some(analysis_ready);
    image
}

/// Outcome of recording one batch.
enum Recorded {
    /// Every row was inserted.
    Inserted,
    /// A row with the same content already exists; nothing was inserted.
    Duplicate(ExistingContent),
}

/// Inserts the records and recalculates the session range in one transaction.
fn record_images(
    db: &Mutex<Connection>,
    session_id: Uuid,
    images: &[Image],
) -> Result<Recorded, SessionError> {
    let conn = lock_db(db)?;
    match insert_images_and_recalculate(&conn, session_id, images)? {
        None => Ok(Recorded::Inserted),
        Some(existing) => Ok(Recorded::Duplicate(existing)),
    }
}

/// Removes a copy after a failed insert.
///
/// Best effort: the caller reports the database failure, not a cleanup
/// failure, but a copy that could not be deleted is logged — a silently
/// orphaned file is how the library grows without anyone noticing.
fn remove_copy(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            eprintln!("warning: could not remove '{}': {err}", file_label(path));
        }
    }
}

/// Removes a session's copied images and prunes now-empty library folders.
///
/// Called after the session's rows are deleted (the database layer never
/// touches the disk). Pruning stops at `library_root`: a folder that still
/// holds another session's files is left alone.
///
/// # Returns
///
/// `true` when the session folder is gone (including "was already absent"),
/// `false` when it could not be removed; the failure is logged.
pub(crate) fn remove_session_files(
    library_root: &Path,
    project_id: &str,
    session_id: &str,
) -> bool {
    let images = session_images_dir(library_root, project_id, session_id);
    if let Err(err) = fs::remove_dir_all(&images) {
        if err.kind() == std::io::ErrorKind::NotFound {
            return true;
        }
        eprintln!("warning: could not remove session image folder: {err}");
        return false;
    }
    // images -> session -> sessions -> project, each removable only if empty.
    for ancestor in images.ancestors().skip(1).take(4) {
        if ancestor == library_root || !ancestor.starts_with(library_root) {
            break;
        }
        if fs::remove_dir(ancestor).is_err() {
            break;
        }
    }
    true
}

/// Minimum age before an unreferenced file is treated as an orphan.
///
/// A copy exists for a moment before its row is committed, and a second app
/// instance may be importing into the same library. Leaving recent files
/// alone keeps the sweep from deleting another process's work in progress.
const ORPHAN_MIN_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Moves copied files that no `image` row references into `.trash/`.
///
/// `referenced` is the absolute copy paths already read from the database, and
/// `known_scopes` is `(project_id, session_id)` pairs of sessions currently in
/// SQLite. The caller loads them and drops the database lock before calling
/// this, so a long sweep does not block session commands.
///
/// # Safety guarantees
///
/// * Only files inside `projects/<p>/sessions/<s>/images/` belonging to a
///   session in `known_scopes` are inspected. Sessions belonging to another
///   database or an older backup are never touched.
/// * Unreferenced files are moved to `<library_root>/.trash/` rather than
///   permanently deleted, so a restored backup or misconfigured path never
///   causes permanent data loss.
/// * If the database has no image rows while known sessions still have files
///   on disk, the sweep skips itself.
/// * Empty directory pruning is restricted strictly to known session paths
///   under `projects/` and never touches arbitrary folders under the library root.
///
/// # Returns
///
/// Number of files moved to trash. `0` when the sweep was skipped.
pub(crate) fn cleanup_orphaned_copies(
    referenced: &[String],
    known_scopes: &std::collections::HashSet<(String, String)>,
    library_root: &Path,
) -> usize {
    cleanup_orphaned_copies_older_than(referenced, known_scopes, library_root, ORPHAN_MIN_AGE)
}

/// [`cleanup_orphaned_copies`] with an explicit age threshold, for tests.
pub(crate) fn cleanup_orphaned_copies_older_than(
    referenced: &[String],
    known_scopes: &std::collections::HashSet<(String, String)>,
    library_root: &Path,
    min_age: std::time::Duration,
) -> usize {
    if !library_root.is_dir() {
        return 0;
    }
    let referenced: std::collections::HashSet<String> = referenced
        .iter()
        .map(|path| normalize_path(Path::new(path)))
        .collect();
    let candidates = library_image_files(library_root, known_scopes);
    if referenced.is_empty() && !candidates.is_empty() {
        eprintln!(
            "warning: skipped the library sweep: the database has no image rows, \
             so unreferenced files were left in place"
        );
        return 0;
    }

    let now = std::time::SystemTime::now();
    let mut moved = 0usize;
    for path in &candidates {
        if referenced.contains(&normalize_path(path)) {
            continue;
        }
        let Ok(metadata) = fs::metadata(path) else {
            continue;
        };
        let old_enough = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= min_age);
        if !old_enough {
            continue;
        }
        match trash_orphan(path, library_root) {
            Ok(()) => moved += 1,
            Err(err) => {
                eprintln!(
                    "warning: could not move '{}' to trash: {err}",
                    file_label(path)
                );
            }
        }
    }
    if moved > 0 {
        prune_session_layout_dirs(library_root, known_scopes);
    }
    moved
}

/// Moves an unreferenced file into `<library_root>/.trash/`.
fn trash_orphan(path: &Path, library_root: &Path) -> std::io::Result<()> {
    let trash_dir = library_root.join(".trash");
    fs::create_dir_all(&trash_dir)?;
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let dest = trash_dir.join(format!("{timestamp}_{file_name}"));
    if fs::rename(path, &dest).is_err() {
        fs::copy(path, &dest)?;
        let _ = fs::remove_file(path);
    }
    Ok(())
}

/// Files sitting in a known session's `images` folder, and nowhere else.
///
/// The layout is `projects/<p_id>/sessions/<s_id>/images/<file>`.
/// Only folders whose `(p_id, s_id)` exists in `known_scopes` are traversed.
fn library_image_files(
    library_root: &Path,
    known_scopes: &std::collections::HashSet<(String, String)>,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(projects) = fs::read_dir(library_root.join("projects")) else {
        return files;
    };
    for project in projects.flatten() {
        if !is_layout_id(&project.file_name()) {
            continue;
        }
        let project_id = project.file_name().to_string_lossy().into_owned();
        let Ok(sessions) = fs::read_dir(project.path().join("sessions")) else {
            continue;
        };
        for session in sessions.flatten() {
            if !is_layout_id(&session.file_name()) {
                continue;
            }
            let session_id = session.file_name().to_string_lossy().into_owned();
            if !known_scopes.contains(&(project_id.clone(), session_id)) {
                // Not in this database — leave untouched.
                continue;
            }
            let Ok(images) = fs::read_dir(session.path().join("images")) else {
                continue;
            };
            for image in images.flatten() {
                let path = image.path();
                if path.is_file() {
                    files.push(path);
                }
            }
        }
    }
    files
}

/// A single path segment that cannot climb out of the library layout.
fn is_layout_id(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

/// The comparison form of a path: canonical when possible, else absolute.
fn normalize_path(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

/// Removes empty layout directories for known scopes under `projects/`.
///
/// Unlike a generic recursive walk, this only prunes directory levels that
/// belong to the known session layout and will never remove arbitrary folders
/// or files outside that structure (such as `.trash/`).
fn prune_session_layout_dirs(
    library_root: &Path,
    known_scopes: &std::collections::HashSet<(String, String)>,
) {
    let projects_root = library_root.join("projects");
    for (project_id, session_id) in known_scopes {
        let session_dir = projects_root
            .join(project_id)
            .join("sessions")
            .join(session_id);
        let _ = fs::remove_dir(session_dir.join("images"));
        let _ = fs::remove_dir(&session_dir);
        let _ = fs::remove_dir(projects_root.join(project_id).join("sessions"));
        let _ = fs::remove_dir(projects_root.join(project_id));
    }
}

fn reject(candidate: &ImportCandidate, reason: ImportError) -> RejectedFile {
    RejectedFile {
        source_path: candidate.source_path.clone(),
        reason,
    }
}

/// Rewrites candidate ids in duplicate flags to the ids that were persisted.
///
/// Persisted `existing_image_id`s (from a previous import) are already image
/// ids and pass through untouched.
fn remap_duplicate_ids(
    flags: &mut [crate::models::image::DuplicateFlag],
    image_ids: &std::collections::HashMap<String, String>,
) {
    for flag in flags {
        if let Some(image_id) = image_ids.get(&flag.existing_image_id) {
            flag.existing_image_id = image_id.clone();
        }
    }
}

fn lock_db(db: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>, SessionError> {
    db.lock()
        .map_err(|err| SessionError::LockPoisoned(err.to_string()))
}

/// Hashes picked files or folders into ready-to-plan candidates.
///
/// Nothing is copied here. Duplicates are decided from these hashes before
/// any byte is written into the library, so re-importing a folder does not
/// copy it twice.
///
/// Picked paths are filtered **before** any read: a path that is not a
/// regular file (a folder, a FIFO, a device node) or whose extension is not
/// jpeg/dng is rejected unopened, and a supported extension with a
/// non-matching header is rejected after the first bytes.
fn scan_sources(
    sources: &[String],
    from_folder: bool,
) -> Vec<Result<ImportCandidate, RejectedFile>> {
    let mut entries: Vec<Result<ImportCandidate, RejectedFile>> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    if from_folder {
        for source in sources {
            match fs::read_dir(source) {
                Ok(dir) => {
                    let mut here: Vec<PathBuf> = dir
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .filter(|path| path.is_file() && is_import_extension(path))
                        .collect();
                    here.sort();
                    files.extend(here);
                }
                Err(err) => entries.push(Err(RejectedFile {
                    source_path: source.clone(),
                    reason: ImportError::CopyFailed {
                        path: source.clone(),
                        reason: format!("cannot read folder: {err}"),
                    },
                })),
            }
        }
    } else {
        for source in sources {
            let path = PathBuf::from(source);
            match fs::metadata(&path) {
                Ok(metadata) if !metadata.is_file() => entries.push(Err(RejectedFile {
                    source_path: source.clone(),
                    reason: ImportError::NotAFile {
                        path: source.clone(),
                    },
                })),
                Ok(_) if !is_import_extension(&path) => entries.push(Err(RejectedFile {
                    source_path: source.clone(),
                    reason: ImportError::UnsupportedFormat {
                        path: source.clone(),
                        extension: extension_of(&file_name_of(&path)),
                    },
                })),
                Ok(_) => files.push(path),
                Err(err) => entries.push(Err(RejectedFile {
                    source_path: source.clone(),
                    reason: ImportError::CopyFailed {
                        path: source.clone(),
                        reason: err.to_string(),
                    },
                })),
            }
        }
    }

    for path in files {
        entries.push(hash_file(&path));
    }
    entries
}

/// A path's file name, or an empty string when it has none.
fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A path reduced to its file name, for logs that must not carry a session id.
fn file_label(path: &Path) -> String {
    file_name_of(path)
}

/// Whether the path's extension is one this module imports.
fn is_import_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "dng")
    )
}

/// Reads one source once, for its header and SHA-256. Does not copy it.
fn hash_file(path: &Path) -> Result<ImportCandidate, RejectedFile> {
    let source_path = path.display().to_string();
    let fail = |reason: ImportError| RejectedFile {
        source_path: source_path.clone(),
        reason,
    };
    let io_fail = |err: std::io::Error| {
        fail(ImportError::CopyFailed {
            path: source_path.clone(),
            reason: err.to_string(),
        })
    };

    let mut file = fs::File::open(path).map_err(io_fail)?;
    let mut header = Vec::with_capacity(HEADER_BYTES);
    Read::by_ref(&mut file)
        .take(HEADER_BYTES as u64)
        .read_to_end(&mut header)
        .map_err(io_fail)?;

    let probe = ImportCandidate {
        id: Uuid::new_v4().to_string(),
        source_path: source_path.clone(),
        file_name: file_name_of(path),
        header,
        content_hash: ContentHash(String::new()),
    };
    if dest_basename(&probe.file_name).is_none() {
        return Err(fail(ImportError::InvalidFileName {
            path: source_path.clone(),
        }));
    }
    if let Err(reason) = classify_file(&probe) {
        return Err(fail(reason));
    }

    let mut hasher = Sha256::new();
    hasher.update(&probe.header);
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(io_fail)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(ImportCandidate {
        content_hash: ContentHash(hex(&hasher.finalize())),
        ..probe
    })
}

/// Lowercase hex encoding of a digest.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Builds the plugin pre-flight projection for a stored image.
///
/// Pure helper on top of the stored record: no disk read, no re-extraction.
pub(crate) fn plugin_projection(image: &Image) -> PluginImageMetadata {
    let metadata = ImageMetadata {
        gps_latitude: image.location_lat,
        gps_longitude: image.location_lon,
        gps_altitude_m: image.gps_altitude_m,
        date_time_original: image.captured_at,
        width: image.width,
        height: image.height,
        format: format_from_db(image.format.as_deref()),
        make: image.make.clone(),
        camera_model_name: image.camera_model.clone(),
        ..ImageMetadata::default()
    };
    to_plugin_metadata(&metadata, Path::new(&image.file_path))
}

fn format_from_db(raw: Option<&str>) -> Option<ImageFormat> {
    match raw {
        Some("jpeg") => Some(ImageFormat::Jpeg),
        Some("dng") => Some(ImageFormat::Dng),
        _ => None,
    }
}

/// Resolves the analysis input for a stored image.
///
/// A JPEG (or a migrated row that predates `analysis_ready`) is its own
/// input. A DNG uses its same-stem JPEG when one is linked; a DNG with no
/// sibling has no analysis input, because Module 1 does not decode raw
/// pixels — it reports [`ImportError::NotAnalysisReady`] instead of handing a
/// plugin a file it cannot read.
///
/// # Purity
///
/// Impure — reads the image row(s).
pub(crate) fn analysis_input(
    conn: &Connection,
    image_id: Uuid,
) -> Result<PluginImageMetadata, ImportFailure> {
    let image = get_image(conn, image_id)?;
    match (image.format.as_deref(), image.raw_sibling_id) {
        (Some("dng"), Some(sibling_id)) => match get_image(conn, sibling_id) {
            Ok(sibling) => Ok(plugin_projection(&sibling)),
            // A dangling sibling means the JPEG is gone; the DNG is just as
            // unusable as one that never had a sibling. Reporting
            // `IMAGE_NOT_FOUND` would name an id the caller never passed.
            Err(SessionError::ImageNotFound { .. }) => Err(ImportError::NotAnalysisReady {
                image_id: image_id.to_string(),
            }
            .into()),
            Err(err) => Err(err.into()),
        },
        (Some("dng"), None) => Err(ImportError::NotAnalysisReady {
            image_id: image_id.to_string(),
        }
        .into()),
        _ if image.analysis_ready == Some(false) => Err(ImportError::NotAnalysisReady {
            image_id: image_id.to_string(),
        }
        .into()),
        _ => Ok(plugin_projection(&image)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::session::SessionStatus;
    use crate::modules::module_01::flight_session::{
        create_session, get_images_by_session, init_schema, insert_image,
    };

    /// Minimal little-endian TIFF with ImageWidth/ImageLength, enough for
    /// `read_dng` to report dimensions without decoding pixels.
    fn tiff_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00]);
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        for (tag, value) in [(256u16, width), (257u16, height)] {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&4u16.to_le_bytes()); // LONG
            bytes.extend_from_slice(&1u32.to_le_bytes()); // count
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0u32.to_le_bytes()); // next IFD
        bytes
    }

    fn write_jpeg(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let image = image::RgbImage::from_fn(8, 8, |x, y| {
            image::Rgb([(x * 30) as u8, (y * 30) as u8, 64])
        });
        let file = std::fs::File::create(&path).expect("create jpeg");
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
        encoder.encode_image(&image).expect("encode synthetic jpeg");
        path
    }

    fn write_jpeg_seeded(dir: &Path, name: &str, seed: u8) -> PathBuf {
        let path = dir.join(name);
        let image = image::RgbImage::from_fn(8, 8, |x, y| {
            image::Rgb([seed.wrapping_add((x * 3) as u8), (y * 5) as u8, 64])
        });
        let file = std::fs::File::create(&path).expect("create jpeg");
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
        encoder.encode_image(&image).expect("encode synthetic jpeg");
        path
    }

    fn image_row(session_id: Uuid, file_path: &str, hash: &str) -> Image {
        Image {
            id: Uuid::new_v4(),
            session_id,
            file_path: file_path.to_string(),
            location_lat: None,
            location_lon: None,
            captured_at: None,
            gps_altitude_m: None,
            width: None,
            height: None,
            format: Some("jpeg".to_string()),
            make: None,
            camera_model: None,
            source_path: None,
            content_hash: Some(hash.to_string()),
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: Some(true),
        }
    }

    fn write_dng(dir: &Path, name: &str, width: u32, height: u32) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, tiff_with_dimensions(width, height)).expect("write synthetic dng");
        path
    }

    fn test_db() -> Mutex<Connection> {
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("schema");
        Mutex::new(conn)
    }

    fn session(db: &Mutex<Connection>) -> Uuid {
        let conn = db.lock().expect("db lock");
        create_session(&conn, Uuid::new_v4(), Some("Session".to_string()))
            .expect("session")
            .id
    }

    fn sources(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    }

    #[test]
    fn copies_into_library_and_survives_source_removal() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_jpeg(source_dir.path(), "DJI_0001.JPG");

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(std::slice::from_ref(&source)),
            false,
            |_, _, _| {},
        )
        .expect("import");

        assert_eq!(report.imported_image_ids.len(), 1);
        assert!(report.rejected.is_empty(), "{:?}", report.rejected);
        let conn = db.lock().expect("db lock");
        let images = get_images_by_session(&conn, session_id).expect("images");
        assert_eq!(images.len(), 1);
        let stored = &images[0];
        assert_eq!(stored.id.to_string(), report.imported_image_ids[0]);
        assert!(stored.file_path.starts_with(
            fs::canonicalize(library.path())
                .expect("canonical library")
                .to_string_lossy()
                .as_ref()
        ));
        assert!(stored.file_path.ends_with("DJI_0001.JPG"));
        assert_eq!(
            stored.source_path.as_deref(),
            Some(source.to_str().unwrap())
        );
        assert!(stored.content_hash.is_some());
        assert!(stored.analysis_ready == Some(true));
        assert!(std::path::Path::new(&stored.file_path).is_file());

        // The whole point: the original can disappear.
        fs::remove_dir_all(source_dir.path()).expect("remove source");
        assert!(extract_metadata(std::path::Path::new(&stored.file_path)).is_ok());
    }

    #[test]
    fn reimport_is_a_duplicate_and_writes_nothing() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_jpeg(source_dir.path(), "DJI_0002.JPG");

        let first = run_import(
            &db,
            library.path(),
            session_id,
            &sources(std::slice::from_ref(&source)),
            false,
            |_, _, _| {},
        )
        .expect("first import");
        assert_eq!(first.imported_image_ids.len(), 1);

        let second = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[source]),
            false,
            |_, _, _| {},
        )
        .expect("second import");
        assert!(second.imported_image_ids.is_empty());
        assert_eq!(second.duplicates.len(), 1);
        let conn = db.lock().expect("db lock");
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
    }

    #[test]
    fn existing_destination_is_imported_under_a_suffix() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_jpeg(source_dir.path(), "DJI_0003.JPG");

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        fs::create_dir_all(&dest_dir).expect("dest dir");
        let existing = dest_dir.join("DJI_0003.JPG");
        fs::write(&existing, b"not the same bytes").expect("preexisting");

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[source]),
            false,
            |_, _, _| {},
        )
        .expect("import");

        assert_eq!(report.imported_image_ids.len(), 1);
        // The file already there is untouched; the import takes a new name.
        assert_eq!(
            fs::read(&existing).expect("existing"),
            b"not the same bytes"
        );
        assert!(dest_dir.join("DJI_0003 (2).JPG").is_file());
        let conn = db.lock().expect("db lock");
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
    }

    #[test]
    fn dng_without_jpeg_is_imported_but_not_analysis_ready() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_dng(source_dir.path(), "DJI_0004.DNG", 4000, 3000);

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[source]),
            false,
            |_, _, _| {},
        )
        .expect("import");
        assert_eq!(report.imported_image_ids.len(), 1);
        assert_eq!(report.not_analysis_ready, report.imported_image_ids);

        let conn = db.lock().expect("db lock");
        let images = get_images_by_session(&conn, session_id).unwrap();
        assert_eq!(images[0].analysis_ready, Some(false));
        assert_eq!(images[0].raw_sibling_id, None);
        assert_eq!(images[0].width, Some(4000));
        assert_eq!(
            images[0].format.as_deref(),
            Some("dng"),
            "format comes from the classifier, not the parser"
        );
        let error = analysis_input(&conn, images[0].id).expect_err("not ready");
        assert!(matches!(
            error,
            ImportFailure::Import(ImportError::NotAnalysisReady { .. })
        ));
    }

    #[test]
    fn jpeg_dng_pair_is_linked_and_analysis_uses_the_jpeg() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let jpeg = write_jpeg(source_dir.path(), "DJI_0005.JPG");
        let dng = write_dng(source_dir.path(), "DJI_0005.DNG", 4000, 3000);

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[jpeg, dng]),
            false,
            |_, _, _| {},
        )
        .expect("import");
        assert_eq!(report.imported_image_ids.len(), 2);
        assert!(report.not_analysis_ready.is_empty());

        let conn = db.lock().expect("db lock");
        let images = get_images_by_session(&conn, session_id).unwrap();
        assert_eq!(images.len(), 2);
        let jpeg_row = images
            .iter()
            .find(|image| image.format.as_deref() == Some("jpeg"))
            .expect("jpeg row");
        let dng_row = images
            .iter()
            .find(|image| image.format.as_deref() == Some("dng"))
            .expect("dng row");
        assert_eq!(jpeg_row.raw_sibling_id, Some(dng_row.id));
        assert_eq!(dng_row.raw_sibling_id, Some(jpeg_row.id));
        assert_eq!(jpeg_row.analysis_ready, Some(true));
        assert_eq!(dng_row.analysis_ready, Some(true));

        let projection = analysis_input(&conn, dng_row.id).expect("dng input");
        assert_eq!(projection.path, jpeg_row.file_path);
        let jpeg_projection = analysis_input(&conn, jpeg_row.id).expect("jpeg input");
        assert_eq!(jpeg_projection.path, jpeg_row.file_path);
    }

    #[test]
    fn failed_insert_deletes_the_copy() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_jpeg(source_dir.path(), "DJI_0007.JPG");
        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        let dest = dest_dir.join("DJI_0007.JPG");
        // The row stores the canonical path the copy will use. On macOS the
        // temp directory is a symlink, so comparing the raw path misses.
        fs::create_dir_all(&dest_dir).expect("dest dir");
        fs::write(&dest, b"placeholder").expect("placeholder");
        let canonical = dunce::canonicalize(&dest).expect("canonical dest");
        fs::remove_file(&dest).expect("remove placeholder");
        // A row already owns the destination path although no file exists:
        // the insert hits the UNIQUE constraint after the copy succeeded.
        let conflict = Image {
            id: Uuid::new_v4(),
            session_id,
            file_path: canonical.display().to_string(),
            location_lat: None,
            location_lon: None,
            captured_at: None,
            gps_altitude_m: None,
            width: None,
            height: None,
            format: Some("jpeg".to_string()),
            make: None,
            camera_model: None,
            source_path: None,
            content_hash: None,
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: Some(true),
        };
        {
            let conn = db.lock().expect("db lock");
            insert_image(&conn, &conflict).expect("conflicting row");
        }

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[source]),
            false,
            |_, _, _| {},
        )
        .expect("import");
        assert!(report.imported_image_ids.is_empty());
        assert_eq!(report.rejected.len(), 1);
        assert!(matches!(
            report.rejected[0].reason,
            ImportError::RecordFailed { .. }
        ));
        assert!(!canonical.exists());
        let conn = db.lock().expect("db lock");
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
    }

    #[test]
    fn two_same_named_files_in_one_batch_both_import() {
        let first_dir = tempfile::tempdir().expect("first dir");
        let second_dir = tempfile::tempdir().expect("second dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let first = write_jpeg_seeded(first_dir.path(), "DJI_0020.JPG", 1);
        let second = write_jpeg_seeded(second_dir.path(), "DJI_0020.JPG", 200);

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[first, second]),
            false,
            |_, _, _| {},
        )
        .expect("import");

        assert_eq!(report.imported_image_ids.len(), 2);
        assert!(report.rejected.is_empty());
        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        assert!(dest_dir.join("DJI_0020.JPG").is_file());
        assert!(dest_dir.join("DJI_0020 (2).JPG").is_file());
    }

    #[test]
    fn record_images_rejects_a_duplicate_that_appeared_after_planning() {
        let db = test_db();
        let session_id = session(&db);
        {
            let conn = db.lock().expect("db lock");
            insert_image(&conn, &image_row(session_id, "/lib/first.jpg", "H")).expect("row");
        }

        let later = image_row(session_id, "/lib/second.jpg", "H");
        match record_images(&db, session_id, std::slice::from_ref(&later)) {
            Ok(Recorded::Duplicate(existing)) => {
                assert_eq!(existing.index, 0);
            }
            Ok(Recorded::Inserted) => panic!("the duplicate slipped through"),
            Err(err) => panic!("record failed: {err}"),
        }
        let conn = db.lock().expect("db lock");
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
    }

    #[test]
    fn pair_with_an_unpersistable_jpeg_still_imports_the_dng() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let jpeg = write_jpeg(source_dir.path(), "DJI_0021.JPG");
        let dng = write_dng(source_dir.path(), "DJI_0021.DNG", 4000, 3000);

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        fs::create_dir_all(&dest_dir).expect("dest dir");

        // Hash both, then delete the JPEG source so its copy fails.
        let jpeg_candidate = hash_file(&jpeg).expect("hash jpeg");
        let dng_candidate = hash_file(&dng).expect("hash dng");
        fs::remove_file(&jpeg).expect("remove jpeg source");
        let mut state = RunState {
            image_ids: std::collections::HashMap::new(),
            rejected: Vec::new(),
            duplicates: Vec::new(),
        };
        {
            let mut scope = ImportScope {
                db: &db,
                dest_dir: &dest_dir,
                session_id,
                state: &mut state,
            };
            let imported = scope
                .pair(&jpeg_candidate, &dng_candidate)
                .expect("dng alone");
            assert_eq!(imported.len(), 1);
            assert!(imported[0].not_analysis_ready);
        }
        assert_eq!(state.rejected.len(), 1);
        let conn = db.lock().expect("db lock");
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
    }

    #[test]
    fn pair_with_an_unpersistable_dng_still_imports_the_jpeg() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let jpeg = write_jpeg(source_dir.path(), "DJI_0022.JPG");
        let dng = write_dng(source_dir.path(), "DJI_0022.DNG", 4000, 3000);

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        fs::create_dir_all(&dest_dir).expect("dest dir");

        let jpeg_candidate = hash_file(&jpeg).expect("hash jpeg");
        let dng_candidate = hash_file(&dng).expect("hash dng");
        fs::remove_file(&dng).expect("remove dng source");
        let mut state = RunState {
            image_ids: std::collections::HashMap::new(),
            rejected: Vec::new(),
            duplicates: Vec::new(),
        };
        {
            let mut scope = ImportScope {
                db: &db,
                dest_dir: &dest_dir,
                session_id,
                state: &mut state,
            };
            let imported = scope
                .pair(&jpeg_candidate, &dng_candidate)
                .expect("jpeg alone");
            assert_eq!(imported.len(), 1);
            assert!(!imported[0].not_analysis_ready);
        }
        assert_eq!(state.rejected.len(), 1);
        let conn = db.lock().expect("db lock");
        let rows = get_images_by_session(&conn, session_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].format.as_deref(), Some("jpeg"));
    }

    #[test]
    fn pair_keeps_one_shared_suffix_when_the_jpeg_name_is_taken() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let jpeg = write_jpeg(source_dir.path(), "DJI_0023.JPG");
        let dng = write_dng(source_dir.path(), "DJI_0023.DNG", 4000, 3000);

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        fs::create_dir_all(&dest_dir).expect("dest dir");
        fs::write(dest_dir.join("DJI_0023.JPG"), b"already here").expect("existing");

        run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[jpeg, dng]),
            false,
            |_, _, _| {},
        )
        .expect("import");

        assert!(dest_dir.join("DJI_0023 (2).JPG").is_file());
        assert!(dest_dir.join("DJI_0023 (2).DNG").is_file());
        assert_eq!(
            fs::read(dest_dir.join("DJI_0023.JPG")).expect("existing"),
            b"already here"
        );
    }

    #[test]
    fn persist_single_tries_the_next_suffix_when_one_is_claimed() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let dest_dir = tempfile::tempdir().expect("dest dir");
        let source = write_jpeg(source_dir.path(), "DJI_0024.JPG");
        let candidate = hash_file(&source).expect("hash");
        fs::write(dest_dir.path().join("DJI_0024.JPG"), b"one").expect("first");
        fs::write(dest_dir.path().join("DJI_0024 (2).JPG"), b"two").expect("second");

        let copy = persist_single(&candidate, dest_dir.path(), "DJI_0024.JPG").expect("persist");
        assert_eq!(
            copy.dest.file_name().and_then(|name| name.to_str()),
            Some("DJI_0024 (3).JPG")
        );
    }

    #[test]
    fn dangling_raw_sibling_is_reported_as_not_ready() {
        let db = test_db();
        let session_id = session(&db);
        let conn = db.lock().expect("db lock");
        let mut dng = image_row(session_id, "/lib/only.dng", "H");
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(Uuid::new_v4());
        dng.analysis_ready = Some(true);
        insert_image(&conn, &dng).expect("dng row");

        let error = analysis_input(&conn, dng.id).expect_err("dangling sibling");
        assert!(matches!(
            error,
            ImportFailure::Import(ImportError::NotAnalysisReady { .. })
        ));
    }

    #[test]
    fn picked_folder_is_not_a_file() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let entries = scan_sources(&sources(&[source_dir.path().to_path_buf()]), false);
        assert!(matches!(
            &entries[0],
            Err(RejectedFile {
                reason: ImportError::NotAFile { .. },
                ..
            })
        ));
    }

    #[test]
    fn sweep_skips_when_the_database_has_no_rows() {
        let library = tempfile::tempdir().expect("library");
        let images = library
            .path()
            .join("projects")
            .join("p1")
            .join("sessions")
            .join("s1")
            .join("images");
        fs::create_dir_all(&images).expect("layout");
        let stray = images.join("keep.JPG");
        fs::write(&stray, b"do not delete").expect("stray");
        // A file outside the layout must never be a candidate either.
        fs::write(library.path().join("notes.txt"), b"note").expect("note");

        let scopes = std::collections::HashSet::from([("p1".to_string(), "s1".to_string())]);
        let removed = cleanup_orphaned_copies_older_than(
            &[],
            &scopes,
            library.path(),
            std::time::Duration::ZERO,
        );
        assert_eq!(removed, 0);
        assert!(stray.is_file());
        assert!(library.path().join("notes.txt").is_file());
    }

    #[test]
    fn sweep_never_touches_unrecorded_sessions() {
        let library = tempfile::tempdir().expect("library");
        let unknown_images = library
            .path()
            .join("projects")
            .join("other_p")
            .join("sessions")
            .join("other_s")
            .join("images");
        fs::create_dir_all(&unknown_images).expect("unknown layout");
        let unrecorded_file = unknown_images.join("DJI_9999.JPG");
        fs::write(&unrecorded_file, b"from another database").expect("unrecorded");

        // The connected database only knows about (p1, s1)
        let scopes = std::collections::HashSet::from([("p1".to_string(), "s1".to_string())]);
        let referenced = vec!["/some/referenced.jpg".to_string()];
        let removed = cleanup_orphaned_copies_older_than(
            &referenced,
            &scopes,
            library.path(),
            std::time::Duration::ZERO,
        );
        assert_eq!(removed, 0);
        assert!(unrecorded_file.is_file());
        assert!(!library.path().join(".trash").exists());
    }

    #[test]
    fn sweep_ignores_files_outside_the_session_layout() {
        let library = tempfile::tempdir().expect("library");
        fs::write(library.path().join("notes.txt"), b"note").expect("note");
        let removed = cleanup_orphaned_copies_older_than(
            &["/some/referenced.jpg".to_string()],
            &std::collections::HashSet::new(),
            library.path(),
            std::time::Duration::ZERO,
        );
        assert_eq!(removed, 0);
        assert!(library.path().join("notes.txt").is_file());
    }

    #[test]
    fn pair_insert_failure_removes_both_copies() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let jpeg = write_jpeg(source_dir.path(), "DJI_0030.JPG");
        let dng = write_dng(source_dir.path(), "DJI_0030.DNG", 4000, 3000);

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        fs::create_dir_all(&dest_dir).expect("dest dir");
        let jpeg_dest = dest_dir.join("DJI_0030.JPG");
        fs::write(&jpeg_dest, b"placeholder").expect("placeholder");
        let canonical = dunce::canonicalize(&jpeg_dest).expect("canonical");
        fs::remove_file(&jpeg_dest).expect("remove placeholder");
        {
            let conn = db.lock().expect("db lock");
            insert_image(
                &conn,
                &image_row(session_id, &canonical.display().to_string(), "other-hash"),
            )
            .expect("conflicting row");
        }

        let report = run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[jpeg, dng]),
            false,
            |_, _, _| {},
        )
        .expect("import");
        assert!(report.imported_image_ids.is_empty());
        assert!(matches!(
            report.rejected.first().map(|file| &file.reason),
            Some(ImportError::RecordFailed { .. })
        ));
        assert!(!canonical.exists());
        assert!(!dest_dir.join("DJI_0030.DNG").exists());
        assert!(!dest_dir.join("DJI_0030 (2).JPG").exists());
        assert!(!dest_dir.join("DJI_0030 (2).DNG").exists());
    }

    #[test]
    fn folder_scan_ignores_other_extensions() {
        let source_dir = tempfile::tempdir().expect("source dir");
        write_jpeg(source_dir.path(), "keep.JPG");
        write_dng(source_dir.path(), "keep.DNG", 10, 10);
        fs::write(source_dir.path().join("notes.txt"), b"ignore me").expect("txt");

        let entries = scan_sources(&sources(&[source_dir.path().to_path_buf()]), true);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(Result::is_ok));
    }

    #[test]
    fn picked_non_image_is_rejected_before_being_read() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let video = source_dir.path().join("flight.mp4");
        fs::write(&video, b"not an image").expect("video");

        let entries = scan_sources(&sources(&[video]), false);
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            Err(RejectedFile {
                reason: ImportError::UnsupportedFormat { .. },
                ..
            }) => {}
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn orphan_sweep_keeps_rows_and_deletes_unreferenced_copies() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_jpeg(source_dir.path(), "DJI_0008.JPG");
        run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[source]),
            false,
            |_, _, _| {},
        )
        .expect("import");

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let dest_dir = session_images_dir(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string(),
        );
        let stray = dest_dir.join("stray.JPG");
        fs::write(&stray, b"ownerless").expect("stray");

        let conn = db.lock().expect("db lock");
        let paths =
            crate::modules::module_01::flight_session::all_image_paths(&conn).expect("paths");
        let scopes =
            crate::modules::module_01::flight_session::known_session_scopes(&conn).expect("scopes");
        let removed = cleanup_orphaned_copies_older_than(
            &paths,
            &scopes,
            library.path(),
            std::time::Duration::ZERO,
        );
        assert_eq!(removed, 1);
        assert!(!stray.exists());
        // Verify the orphan was moved into .trash rather than permanently deleted
        assert!(library.path().join(".trash").is_dir());
        let trash_entries: Vec<_> = fs::read_dir(library.path().join(".trash"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(trash_entries.len(), 1);
        assert!(trash_entries[0]
            .file_name()
            .to_string_lossy()
            .ends_with("stray.JPG"));
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
        let stored = get_images_by_session(&conn, session_id).unwrap();
        assert!(Path::new(&stored[0].file_path).is_file());
    }

    #[test]
    fn removing_session_files_deletes_copies_and_empty_folders() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let db = test_db();
        let session_id = session(&db);
        let source = write_jpeg(source_dir.path(), "DJI_0009.JPG");
        run_import(
            &db,
            library.path(),
            session_id,
            &sources(&[source]),
            false,
            |_, _, _| {},
        )
        .expect("import");

        let project_id = {
            let conn = db.lock().expect("db lock");
            crate::modules::module_01::get_session(&conn, session_id)
                .unwrap()
                .project_id
        };
        let project_dir = library.path().join("projects").join(project_id.to_string());
        assert!(remove_session_files(
            library.path(),
            &project_id.to_string(),
            &session_id.to_string()
        ));
        assert!(!project_dir.exists());
        // The library root itself is never removed.
        assert!(library.path().is_dir());
    }

    #[test]
    fn session_dir_is_pure_and_nested() {
        let dir = session_images_dir(Path::new("/library"), "p1", "s1");
        assert_eq!(
            dir,
            PathBuf::from("/library/projects/p1/sessions/s1/images")
        );
    }

    #[test]
    fn legacy_dng_row_is_not_analysis_ready() {
        let conn = Connection::open_in_memory().expect("db");
        init_schema(&conn).expect("schema");
        let session = create_session(&conn, Uuid::new_v4(), None).expect("session");
        let legacy = Image {
            id: Uuid::new_v4(),
            session_id: session.id,
            file_path: "/originals/DJI.DNG".to_string(),
            location_lat: None,
            location_lon: None,
            captured_at: None,
            gps_altitude_m: None,
            width: None,
            height: None,
            format: Some("dng".to_string()),
            make: None,
            camera_model: None,
            source_path: None,
            content_hash: None,
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: None,
        };
        insert_image(&conn, &legacy).expect("legacy row");
        assert!(matches!(
            analysis_input(&conn, legacy.id),
            Err(ImportFailure::Import(ImportError::NotAnalysisReady { .. }))
        ));
        assert_eq!(session.status, SessionStatus::Active);
    }

    #[test]
    fn metadata_merge_fills_only_missing_fields() {
        let mut jpeg = ImageMetadata {
            format: Some(ImageFormat::Jpeg),
            width: Some(640),
            ..ImageMetadata::default()
        };
        let dng = ImageMetadata {
            format: Some(ImageFormat::Dng),
            width: Some(4000),
            height: Some(3000),
            gps_latitude: Some(-7.5),
            ..ImageMetadata::default()
        };
        fill_missing(&mut jpeg, &dng);
        assert_eq!(jpeg.width, Some(640), "present values are kept");
        assert_eq!(jpeg.height, Some(3000));
        assert_eq!(jpeg.gps_latitude, Some(-7.5));
        assert_eq!(jpeg.format, Some(ImageFormat::Jpeg));
    }
}
