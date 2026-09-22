//! Shared data models for aerial image metadata (Module 1).
//!
//! Re-exports the most commonly used types so consumers can write
//! `use crate::models::{ImageFormat, ImageMetadata}`.

pub mod metadata;
pub mod session;

#[allow(unused_imports)]
pub use session::{Image, Project, Session, SessionStatus};

pub use metadata::{ImageFormat, ImageMetadata};
