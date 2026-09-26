#![allow(unused_imports)]

//! Aerial Image & Project Manager — Image Metadata Extraction (Module 1.3).
//!
//! Re-exports the metadata extraction API so consumers can write
//! `use crate::modules::module_01::{extract_metadata, MetadataError};`.
//!
//! RGB import stubs are re-exported from [`image_import`]
//! (`classify_file`, `import_images`).
//!
//! `unused_imports` is allowed at module level because these re-exports are
//! not yet consumed anywhere — the functions behind them are `unimplemented!()`
//! stubs (see [`functions`] and [`image_import`]).

pub mod error;
pub mod flight_session;
pub mod functions;
pub mod image_import;

pub use error::{MetadataError, SessionError};
pub use flight_session::{
    assign_image_to_session, create_session, delete_session, get_session, get_sessions_by_project,
    init_schema, recalculate_session_date_range, update_session_name, update_session_status,
};
pub use functions::{
    describe_completeness, detect_format, dms_to_decimal, extract_metadata, merge_tags, parse_exif,
    parse_exif_datetime, parse_xmp_dji, read_dimensions, to_plugin_metadata, GpsCoordinate,
    MetadataCompleteness, PixelDimensions, RawExifTags, RawXmpTags,
};
pub use image_import::{classify_file, find_duplicate, find_name_conflicts, import_images};
