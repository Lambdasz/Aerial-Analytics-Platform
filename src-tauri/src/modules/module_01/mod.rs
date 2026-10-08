#![allow(unused_imports)]

//! Aerial Image & Project Manager — Image Metadata Extraction (Module 1.3).
//!
//! Re-exports the metadata extraction API so consumers can write
//! `use crate::modules::module_01::{extract_metadata, MetadataError};`.
//!
//! RGB import exposes [`image_importer::import_images`] plus the shared
//! helpers [`find_name_conflicts`], [`find_duplicate`], and [`detect_format`].
//!
//! `unused_imports` is allowed at module level because these re-exports are
//! not yet consumed anywhere. [`detect_format`] and the import helpers are
//! implemented; the rest of the extraction pipeline is still stubbed (see
//! [`metadata_extractor`] and [`image_importer`]).

pub mod error;
pub mod flight_session;
pub mod image_importer;
pub mod metadata_extractor;

pub use error::{MetadataError, SessionError};
pub use flight_session::{
    assign_image_to_session, create_session, delete_session, get_session, get_sessions_by_project,
    init_schema, recalculate_session_date_range, update_session_name, update_session_status,
};
pub(crate) use image_importer::{find_duplicate, find_name_conflicts, import_images};
pub(crate) use metadata_extractor::{
    describe_completeness, detect_format, extract_metadata, to_plugin_metadata,
    MetadataCompleteness,
};
