#![allow(unused_imports)]

//! Aerial Image & Project Manager — Image Metadata Extraction (Module 1.3).
//!
//! Re-exports the metadata extraction API so consumers can write
//! `use crate::modules::module_01::{extract_metadata, MetadataError};`.
//!
//! RGB import exposes [`image_importer::import_images`] plus the shared
//! helpers [`find_name_conflicts`], [`find_duplicate`], and [`detect_format`].
//!
//! Reading a DNG from another module goes through [`read_dng`], which keeps
//! the TIFF/IFD parsing inside this module, and [`read_dng_preview`], which
//! returns the JPEG a DNG already stores so a browser can display it.
//!
//! `unused_imports` is allowed at module level because these re-exports are
//! not yet consumed anywhere. The extraction pipeline (`detect_format`,
//! `extract_metadata`, `read_dng`, the EXIF/XMP parsers), the DNG preview
//! reader and the import helpers are implemented (see
//! [`metadata_extractor`], [`dng_preview`] and [`image_importer`]).

pub mod dng_preview;
pub mod error;
pub mod flight_session;
pub mod image_importer;
pub mod metadata_extractor;

pub(crate) use dng_preview::read_dng_preview;
pub use error::{MetadataError, SessionError};
pub use flight_session::{
    assign_image_to_session, count_images_by_project, create_project, create_session,
    delete_project, delete_session, get_image, get_images_by_session, get_project, get_session,
    get_sessions_by_project, image_from_metadata, init_schema, insert_image, list_projects,
    migrate_image_schema, recalculate_session_date_range, update_project, update_session_name,
    update_session_status,
};
pub(crate) use image_importer::{
    find_duplicate, find_name_conflicts, flag_incomplete_metadata, import_images, NameFolding,
};
pub(crate) use metadata_extractor::{
    describe_completeness, detect_format, extract_metadata, missing_required_fields, read_dng,
    to_plugin_metadata, MetadataCompleteness, REQUIRED_METADATA_FIELDS,
};
