//! Copy-on-import: the impure half of RGB image import.
//!
//! [`crate::modules::module_01::image_importer::import_images`] decides *what*
//! to import without touching the disk or the database. This module applies
//! that plan: it scans the picked files, copies each accepted image into the
//! app-owned library folder, reads metadata from **the copy**, records the
//! row, and reports per-file failures without aborting the batch.
//!
//! ```text
//! scan_sources                     (read sources, hash them)
//!   -> import_images + known_hashes_for_project  (pure plan)
//!   -> copy_verified per file      (hash-checked atomic copy)
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
//! * A copied file whose bytes no longer match the planned SHA-256 is
//!   discarded ([`ImportError::SourceChanged`]): a file that changed between
//!   planning and copying is never recorded under the old identity.
//! * An existing destination is never overwritten
//!   ([`ImportError::DestinationExists`]).
//! * One bad file never aborts the rest of the batch.
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
    ContentHash, ImageFormat, ImageMetadata, ImportCandidate, ImportError, ImportReport,
    ImportRequest, RejectedFile, SessionTarget,
};
use crate::models::Image;
use crate::plugin_manager::error::CommandError;
use crate::plugin_manager::payload::ImageMetadata as PluginImageMetadata;

use super::error::SessionError;
use super::flight_session::{
    get_image, get_session, image_from_metadata, insert_image, known_hashes_for_project,
    recalculate_session_date_range, with_immediate_transaction,
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
///   query and for the insert transaction; copying and hashing run with the
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
/// files that cannot be copied or recorded are reported in the result's
/// `rejected` list, not as an error.
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
    let folding = folding_for_os();
    let known = {
        let conn = lock_db(db)?;
        known_hashes_for_project(&conn, session.project_id)?
    };
    let existing_paths = list_existing(&dest_dir)?;
    let mut report = import_images(&request, &known, &existing_paths, folding)?;
    report.rejected = rejected;

    import_planned(
        db,
        &dest_dir,
        session_id,
        &request,
        report,
        folding,
        &mut on_progress,
    )
}

/// Copies and records every candidate the plan approved.
///
/// Splits the result back into [`ImportReport`] fields: image ids become the
/// persisted ids (not candidate ids), copy failures become `rejected`, and
/// unpaired DNGs are collected in `not_analysis_ready`.
fn import_planned(
    db: &Mutex<Connection>,
    dest_dir: &Path,
    session_id: Uuid,
    request: &ImportRequest,
    report: ImportReport,
    folding: NameFolding,
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
    let pairs = pair_raw_siblings(&request.candidates, folding);
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
    let mut rejected = std::mem::take(&mut report.rejected);
    let mut imported_ids = Vec::new();
    let mut not_analysis_ready = Vec::new();
    // Candidate id -> persisted image id, so the report can hand the UI ids
    // that `get_analysis_input` accepts.
    let mut image_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for candidate in work {
        let partner = partner_of
            .get(candidate.id.as_str())
            .and_then(|id| candidates.get(id))
            .copied();
        let rejected_before = rejected.len();

        let outcome = match partner {
            Some(partner) => import_pair(
                db,
                dest_dir,
                session_id,
                candidate,
                partner,
                &mut image_ids,
                &mut rejected,
            ),
            None => import_single(
                db,
                dest_dir,
                session_id,
                candidate,
                &mut image_ids,
                &mut rejected,
            )
            .map(|imported| vec![imported]),
        };

        // `done` counts candidates, not items: a stem pair is two files, so
        // progress stays coherent even when only one half lands in
        // `rejected`.
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
        done += rejected.len() - rejected_before;
    }

    report.session_id = session_id.to_string();
    report.imported_image_ids = imported_ids;
    report.not_analysis_ready = not_analysis_ready;
    report.rejected = rejected;
    remap_duplicate_ids(&mut report.duplicates, &image_ids);
    remap_duplicate_ids(&mut report.pending_duplicates, &image_ids);
    Ok(report)
}

/// One copied, recorded file.
struct ImportedFile {
    image_id: String,
    dest: String,
    not_analysis_ready: bool,
}

/// Copies and records one standalone image.
fn import_single(
    db: &Mutex<Connection>,
    dest_dir: &Path,
    session_id: Uuid,
    candidate: &ImportCandidate,
    image_ids: &mut std::collections::HashMap<String, String>,
    rejected: &mut Vec<RejectedFile>,
) -> Result<ImportedFile, ()> {
    let (format, copy) = match copy_candidate(candidate, dest_dir) {
        Ok((format, copy)) => (format, copy),
        Err(err) => {
            rejected.push(reject(candidate, err));
            return Err(());
        }
    };
    let metadata = metadata_from_copy(&copy.dest, format);
    let image_id = Uuid::new_v4();
    let ready = format == ImageFormat::Jpeg;
    let image = build_image(
        image_id, session_id, &copy, candidate, &metadata, None, ready,
    );
    if let Err(err) = record_images(db, session_id, std::slice::from_ref(&image)) {
        remove_copy(&copy.dest);
        rejected.push(RejectedFile {
            source_path: candidate.source_path.clone(),
            reason: ImportError::CopyFailed {
                path: candidate.source_path.clone(),
                reason: err.to_string(),
            },
        });
        return Err(());
    }
    image_ids.insert(candidate.id.clone(), image_id.to_string());
    Ok(ImportedFile {
        image_id: image_id.to_string(),
        dest: copy.dest.display().to_string(),
        not_analysis_ready: !ready,
    })
}

/// Copies and records a JPEG/DNG stem pair in one insertion.
///
/// The JPEG is inserted first, then the DNG, both linked through
/// `raw_sibling_id`. If the DNG cannot be copied, the JPEG is still imported
/// alone (it is analysis-ready by itself); if the JPEG cannot be copied, the
/// DNG is imported alone as not-analysis-ready.
fn import_pair(
    db: &Mutex<Connection>,
    dest_dir: &Path,
    session_id: Uuid,
    first: &ImportCandidate,
    second: &ImportCandidate,
    image_ids: &mut std::collections::HashMap<String, String>,
    rejected: &mut Vec<RejectedFile>,
) -> Result<Vec<ImportedFile>, ()> {
    let (jpeg, dng) = match classify_file(first) {
        Ok(ImageFormat::Jpeg) => (first, second),
        Ok(ImageFormat::Dng) => (second, first),
        Err(err) => {
            rejected.push(reject(first, err));
            return Err(());
        }
    };
    // Defensive: the planner only approves JPEG/DNG candidates, but pairing
    // is name-based, so re-check the partner's classification.
    if classify_file(jpeg) != Ok(ImageFormat::Jpeg) || classify_file(dng) != Ok(ImageFormat::Dng) {
        rejected.push(reject(
            dng,
            ImportError::UnsupportedFormat {
                path: dng.source_path.clone(),
                extension: extension_of(&dng.file_name),
            },
        ));
        return Err(());
    }

    let jpeg_copy = match copy_candidate(jpeg, dest_dir) {
        Ok((_, copy)) => copy,
        Err(err) => {
            rejected.push(reject(jpeg, err));
            // The raw file can still be imported on its own, but it will not
            // be analysis-ready. Fall back to a standalone DNG import.
            return import_single(db, dest_dir, session_id, dng, image_ids, rejected)
                .map(|imported| vec![imported]);
        }
    };
    let dng_copy = match copy_candidate(dng, dest_dir) {
        Ok((_, copy)) => copy,
        Err(err) => {
            rejected.push(reject(dng, err));
            // JPEG alone: still a complete, analysis-ready image.
            let metadata = metadata_from_copy(&jpeg_copy.dest, ImageFormat::Jpeg);
            let jpeg_id = Uuid::new_v4();
            let image = build_image(jpeg_id, session_id, &jpeg_copy, jpeg, &metadata, None, true);
            if let Err(err) = record_images(db, session_id, std::slice::from_ref(&image)) {
                remove_copy(&jpeg_copy.dest);
                rejected.push(RejectedFile {
                    source_path: jpeg.source_path.clone(),
                    reason: ImportError::CopyFailed {
                        path: jpeg.source_path.clone(),
                        reason: err.to_string(),
                    },
                });
                return Err(());
            }
            image_ids.insert(jpeg.id.clone(), jpeg_id.to_string());
            return Ok(vec![ImportedFile {
                image_id: jpeg_id.to_string(),
                dest: jpeg_copy.dest.display().to_string(),
                not_analysis_ready: false,
            }]);
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
            session_id,
            &jpeg_copy,
            jpeg,
            &jpeg_metadata,
            Some(dng_id),
            true,
        ),
        build_image(
            dng_id,
            session_id,
            &dng_copy,
            dng,
            &dng_metadata,
            Some(jpeg_id),
            true,
        ),
    ];
    if let Err(err) = record_images(db, session_id, &images) {
        remove_copy(&jpeg_copy.dest);
        remove_copy(&dng_copy.dest);
        rejected.push(RejectedFile {
            source_path: jpeg.source_path.clone(),
            reason: ImportError::CopyFailed {
                path: jpeg.source_path.clone(),
                reason: err.to_string(),
            },
        });
        return Err(());
    }

    image_ids.insert(jpeg.id.clone(), jpeg_id.to_string());
    image_ids.insert(dng.id.clone(), dng_id.to_string());
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

/// Copies one source into `dest_dir`, verifying its SHA-256 on the way.
///
/// # Purity
///
/// Impure — reads the source and writes a temporary file, which is renamed
/// into place only after the hash matches.
///
/// # Errors
///
/// [`ImportError::CopyFailed`] for any I/O failure,
/// [`ImportError::SourceChanged`] when the bytes hash differently from
/// `candidate.content_hash` (nothing is left behind), and
/// [`ImportError::DestinationExists`] when a file already occupies the
/// destination.
fn copy_candidate(
    candidate: &ImportCandidate,
    dest_dir: &Path,
) -> Result<(ImageFormat, CopiedFile), ImportError> {
    let format = classify_file(candidate)?;
    let file_name =
        dest_basename(&candidate.file_name).ok_or_else(|| ImportError::InvalidFileName {
            path: candidate.source_path.clone(),
        })?;
    fs::create_dir_all(dest_dir).map_err(|err| ImportError::CopyFailed {
        path: candidate.source_path.clone(),
        reason: format!("could not create '{}': {err}", dest_dir.display()),
    })?;

    let mut temp = NamedTempFile::new_in(dest_dir).map_err(|err| ImportError::CopyFailed {
        path: candidate.source_path.clone(),
        reason: format!("could not create a temporary file: {err}"),
    })?;
    let mut source =
        fs::File::open(&candidate.source_path).map_err(|err| ImportError::CopyFailed {
            path: candidate.source_path.clone(),
            reason: err.to_string(),
        })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    let mut file_size = 0u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|err| ImportError::CopyFailed {
                path: candidate.source_path.clone(),
                reason: err.to_string(),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        temp.write_all(&buffer[..read])
            .map_err(|err| ImportError::CopyFailed {
                path: candidate.source_path.clone(),
                reason: err.to_string(),
            })?;
        file_size += read as u64;
    }
    temp.as_file()
        .sync_all()
        .map_err(|err| ImportError::CopyFailed {
            path: candidate.source_path.clone(),
            reason: err.to_string(),
        })?;

    let content_hash = hex(&hasher.finalize());
    if !content_hash.eq_ignore_ascii_case(&candidate.content_hash.0) {
        // Dropping `temp` removes the partial copy.
        return Err(ImportError::SourceChanged {
            path: candidate.source_path.clone(),
        });
    }

    let dest = dest_dir.join(file_name);
    let persisted = temp.persist_noclobber(&dest).map_err(|err| {
        if err.error.kind() == std::io::ErrorKind::AlreadyExists {
            ImportError::DestinationExists {
                path: dest.display().to_string(),
            }
        } else {
            ImportError::CopyFailed {
                path: candidate.source_path.clone(),
                reason: err.error.to_string(),
            }
        }
    })?;
    drop(persisted);

    // Canonicalize after the rename so the database stores the same absolute
    // path the rest of the app will open, even when the library root is a
    // symlink or a relative test override.
    let canonical = fs::canonicalize(&dest).map_err(|err| {
        // Do not keep a copy the database will never reference.
        let _ = fs::remove_file(&dest);
        ImportError::CopyFailed {
            path: candidate.source_path.clone(),
            reason: err.to_string(),
        }
    })?;
    Ok((
        format,
        CopiedFile {
            dest: canonical,
            content_hash,
            file_size,
        },
    ))
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

/// Inserts the records and recalculates the session range in one transaction.
fn record_images(
    db: &Mutex<Connection>,
    session_id: Uuid,
    images: &[Image],
) -> Result<(), SessionError> {
    let conn = lock_db(db)?;
    with_immediate_transaction(&conn, |conn| {
        for image in images {
            insert_image(conn, image)?;
        }
        recalculate_session_date_range(conn, session_id)
    })
}

/// Removes a copy after a failed insert, best effort.
///
/// The error the caller reports is the database failure, not a cleanup
/// failure; an undeletable copy is still better than hiding the real cause.
fn remove_copy(path: &Path) {
    let _ = fs::remove_file(path);
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
        .map_err(|err| SessionError::Storage(format!("database lock poisoned: {err}")))
}

/// Scans picked files or folders into ready-to-plan candidates.
///
/// Each successful entry carries the 16-byte header, the SHA-256 of the whole
/// file, and a fresh candidate id. Unreadable files become per-file
/// rejections instead of failing the batch.
fn scan_sources(
    sources: &[String],
    from_folder: bool,
) -> Vec<Result<ImportCandidate, RejectedFile>> {
    let mut entries: Vec<Result<ImportCandidate, RejectedFile>> = Vec::new();
    let files: Vec<PathBuf> = if from_folder {
        let mut files = Vec::new();
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
        files
    } else {
        sources.iter().map(PathBuf::from).collect()
    };

    for path in files {
        entries.push(scan_file(&path));
    }
    entries
}

fn is_import_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg" | "dng")
    )
}

/// Reads one file's header and content hash into a candidate.
fn scan_file(path: &Path) -> Result<ImportCandidate, RejectedFile> {
    let source_path = path.display().to_string();
    let fail = |reason: String| RejectedFile {
        source_path: source_path.clone(),
        reason: ImportError::CopyFailed {
            path: source_path.clone(),
            reason,
        },
    };
    let mut file = fs::File::open(path).map_err(|err| fail(err.to_string()))?;
    let mut header = Vec::with_capacity(HEADER_BYTES);
    Read::by_ref(&mut file)
        .take(HEADER_BYTES as u64)
        .read_to_end(&mut header)
        .map_err(|err| fail(err.to_string()))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|err| fail(err.to_string()))?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| fail(err.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(ImportCandidate {
        id: Uuid::new_v4().to_string(),
        source_path,
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        header,
        content_hash: ContentHash(hex(&hasher.finalize())),
    })
}

/// Full paths already in the session folder, for name-conflict detection.
fn list_existing(dest_dir: &Path) -> Result<Vec<String>, SessionError> {
    if !dest_dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(dest_dir).map_err(|err| {
        SessionError::Storage(format!("cannot read '{}': {err}", dest_dir.display()))
    })?;
    let mut paths: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().display().to_string())
        .collect();
    paths.sort();
    Ok(paths)
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
        (Some("dng"), Some(sibling_id)) => {
            let sibling = get_image(conn, sibling_id)?;
            Ok(plugin_projection(&sibling))
        }
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
        create_session, get_images_by_session, init_schema,
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
    fn existing_destination_is_never_overwritten() {
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

        assert!(report.imported_image_ids.is_empty());
        assert_eq!(report.name_conflicts.len(), 1);
        assert_eq!(
            fs::read(&existing).expect("existing"),
            b"not the same bytes"
        );
        let conn = db.lock().expect("db lock");
        assert!(get_images_by_session(&conn, session_id).unwrap().is_empty());
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
    fn source_that_changed_after_planning_leaves_no_file() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let library = tempfile::tempdir().expect("library");
        let source = write_jpeg(source_dir.path(), "DJI_0006.JPG");
        let dest_dir = library.path().join("dest");
        let candidate = match scan_sources(&sources(std::slice::from_ref(&source)), false).remove(0)
        {
            Ok(candidate) => candidate,
            Err(rejection) => panic!("scan rejected: {rejection:?}"),
        };
        // Simulate the source changing between hashing and copying.
        fs::write(&source, b"different bytes now").expect("rewrite source");

        let error = copy_candidate(&candidate, &dest_dir).expect_err("hash mismatch");
        assert!(matches!(error, ImportError::SourceChanged { .. }));
        assert!(!dest_dir.join("DJI_0006.JPG").exists());
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
        // A row already owns the destination path although no file exists:
        // the insert hits the UNIQUE constraint after the copy succeeded.
        let conflict = Image {
            id: Uuid::new_v4(),
            session_id,
            file_path: dest.display().to_string(),
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
            ImportError::CopyFailed { .. }
        ));
        assert!(!dest.exists());
        let conn = db.lock().expect("db lock");
        assert_eq!(get_images_by_session(&conn, session_id).unwrap().len(), 1);
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
