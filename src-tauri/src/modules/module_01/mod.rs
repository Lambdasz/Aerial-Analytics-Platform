#![allow(unused_imports)]

//! Aerial Image & Project Manager — Image Metadata Extraction (Module 1.3).
//!
//! Re-exports the metadata extraction API so consumers can write
//! `use crate::modules::module_01::{extract_metadata, MetadataError};`.
//!
//! `unused_imports` is allowed at module level because these re-exports are
//! not yet consumed anywhere — the functions behind them are `unimplemented!()`
//! stubs (see [`functions`]).

pub mod error;
pub mod functions;

pub use error::MetadataError;
pub use functions::{
    describe_completeness, detect_format, dms_to_decimal, extract_metadata, merge_tags, parse_exif,
    parse_exif_datetime, parse_xmp_dji, read_dimensions, to_plugin_metadata, GpsCoordinate,
    MetadataCompleteness, PixelDimensions, RawExifTags, RawXmpTags,
};
