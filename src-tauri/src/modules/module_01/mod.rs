#![allow(unused_imports)]

//! Aerial Image & Project Manager — Image Metadata Extraction (Module 1.3).
//!
//! Re-exports the metadata extraction API so consumers can write
//! `use crate::modules::module_01::{extract_metadata, MetadataError};`.
//!
//! RGB import stubs are re-exported from [`image_importer`]
//! (`classify_file`, `import_images`).
//!
//! `unused_imports` is allowed at module level because these re-exports are
//! not yet consumed anywhere — the functions behind them are `unimplemented!()`
//! stubs (see [`metadata_extractor`] and [`image_importer`]).

pub mod error;
pub mod image_importer;
pub mod metadata_extractor;

pub use error::MetadataError;
pub use image_importer::{classify_file, find_duplicate, find_name_conflicts, import_images};
pub use metadata_extractor::{
    describe_completeness, detect_format, dms_to_decimal, extract_metadata, merge_tags, parse_exif,
    parse_exif_datetime, parse_xmp_dji, read_dimensions, to_plugin_metadata, GpsCoordinate,
    MetadataCompleteness, PixelDimensions, RawExifTags, RawXmpTags,
};
