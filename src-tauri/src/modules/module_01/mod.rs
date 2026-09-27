#![allow(unused_imports)]

//! Aerial Image & Project Manager — Image Metadata Extraction (Module 1.3).
//!
//! Re-exports the metadata extraction API so consumers can write
//! `use crate::modules::module_01::{extract_metadata, MetadataError};`.
//!
//! RGB import exposes only [`image_importer::import_images`].
//!
//! `unused_imports` is allowed at module level because these re-exports are
//! not yet consumed anywhere — the functions behind them are `unimplemented!()`
//! stubs (see [`metadata_extractor`] and [`image_importer`]).

pub mod error;
pub mod image_importer;
pub mod metadata_extractor;

pub use error::MetadataError;
pub(crate) use image_importer::import_images;
pub(crate) use metadata_extractor::{
    describe_completeness, extract_metadata, to_plugin_metadata, MetadataCompleteness,
};
