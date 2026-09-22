//! Data model definitions for the Module 1 image import pipeline and
//! extracted image metadata.
//!
//! This file holds two related but distinct families of types:
//! - **Import pipeline types** ([`ContentHash`], [`SessionTarget`],
//!   [`ImportRequest`], [`ImportCandidate`], [`NameConflict`],
//!   [`DuplicateFlag`], [`RejectedFile`], [`ImportReport`],
//!   [`ImportError`]) — describe an in-flight import operation, before any
//!   image has become a persisted record.
//! - **Format/metadata types** ([`ImageFormat`], [`ImageMetadata`]) — the
//!   EXIF/XMP-derived data extracted from a source file during import.
//!
//! Neither family is the canonical persisted image record; that's
//! [`crate::models::session::Image`]. Once an [`ImportCandidate`] is
//! successfully imported, its extracted [`ImageMetadata`] is folded into a
//! `models::session::Image` for storage.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// SHA-256 of the raw file bytes, lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(pub String);

/// Which session an [`ImportRequest`] should import into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionTarget {
    /// Import into a session that already exists.
    Existing { id: String },
    /// Create a new session with `label` as part of this import.
    New { id: String, label: String },
}

/// One import operation over a set of picked files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    /// `true` = user picked a folder (I/O scans jpeg/dng). `false` = user picked image files.
    pub from_folder: bool,
    /// Session this import will land in.
    pub session: SessionTarget,
    /// Files picked (or discovered, if `from_folder`) for this import.
    pub candidates: Vec<ImportCandidate>,
}

/// One file offered for import, with enough pre-read data to classify it
/// without further disk access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCandidate {
    /// Client-generated id used to correlate this candidate with its result.
    pub id: String,
    /// Absolute path to the source file on disk.
    pub source_path: String,
    /// File name as it will appear in the destination session folder.
    pub file_name: String,
    /// First ≥16 bytes of the file. Used only for JPEG/DNG magic sniff, not EXIF.
    pub header: Vec<u8>,
    /// SHA-256 of the full file, used for duplicate detection.
    pub content_hash: ContentHash,
}

/// A destination-basename collision found among incoming candidates or
/// against files already in the session folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameConflict {
    pub dest_name: String,
    /// Incoming source paths that want `dest_name`.
    pub sources: Vec<String>,
    /// Path already in the session folder with `dest_name`. `None` if only incoming files collide.
    pub existing_path: Option<String>,
}

/// A candidate whose content hash matches an image already in the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateFlag {
    pub source_path: String,
    pub existing_image_id: String,
    pub content_hash: ContentHash,
}

/// A candidate that could not be imported, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedFile {
    pub source_path: String,
    pub reason: ImportError,
}

/// Outcome of one [`ImportRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    /// Session the import landed in (created or existing, per [`SessionTarget`]).
    pub session_id: String,
    /// Ids of images successfully imported.
    pub imported_image_ids: Vec<String>,
    /// Candidates skipped because they duplicate an existing project image.
    pub duplicates: Vec<DuplicateFlag>,
    /// Candidates that failed to import, with reasons.
    pub rejected: Vec<RejectedFile>,
    /// Destination-basename collisions found during the import.
    pub name_conflicts: Vec<NameConflict>,
}

/// Reasons an import operation or a single candidate within it can fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportError {
    /// The request contained no candidates.
    NoImagesFound,
    /// `session_id` does not reference an existing session.
    SessionNotFound { session_id: String },
    /// Extension is not jpeg/jpg/dng (or missing). `extension` has no leading dot.
    UnsupportedFormat { path: String, extension: String },
    /// Extension is jpeg/dng but `header` magic does not match.
    MagicMismatch { path: String },
}

/// Supported image file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    /// JPEG compressed image.
    Jpeg,
    /// DNG (Digital Negative) raw image.
    Dng,
}

/// Complete metadata for a single aerial RGB image.
///
/// Fields are populated from EXIF and XMP DJI tags when available.
/// All fields are `Option<T>` because not every image contains every tag
/// (e.g. a non-DJI drone will lack XMP flight data).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImageMetadata {
    // --- geolocation ---
    /// GPS latitude in decimal degrees (south is negative).
    pub gps_latitude: Option<f64>,

    /// GPS longitude in decimal degrees (west is negative).
    pub gps_longitude: Option<f64>,

    /// GPS altitude in metres above sea level from EXIF.
    /// Negative when `GPSAltitudeRef` indicates below sea level.
    pub gps_altitude_m: Option<f64>,

    /// Absolute altitude in metres from XMP DJI tag (barometer or
    /// ellipsoid RTK, depending on drone model).
    pub absolute_altitude_m: Option<f64>,

    // --- timestamp ---
    /// Date and time when the image was captured (EXIF `DateTimeOriginal`).
    pub date_time_original: Option<NaiveDateTime>,

    // --- dimensions ---
    /// Image width in pixels (from JPEG header or DNG IFD0).
    pub width: Option<u32>,

    /// Image height in pixels.
    pub height: Option<u32>,

    // --- imagery ---
    /// Detected image format.
    pub format: Option<ImageFormat>,

    /// Camera manufacturer (e.g. `"DJI"`, `"Sony"`).
    pub make: Option<String>,

    /// Camera model name (e.g. `"Mavic 3"`, `"ILCE-7RM4"`).
    pub camera_model_name: Option<String>,

    /// Exposure time in seconds (e.g. `0.001` for 1/1000 s).
    pub exposure_time_s: Option<f64>,

    /// F-number (aperture) of the lens (e.g. `2.8`).
    pub f_number: Option<f32>,

    /// ISO sensitivity value.
    pub iso: Option<u32>,

    /// Actual focal length in millimetres.
    pub focal_length_mm: Option<f32>,

    /// 35 mm equivalent focal length.
    pub focal_length_35mm: Option<u16>,

    /// Raw flash fire code (bitfield). Meaning is vendor-specific.
    pub flash: Option<u16>,

    /// Raw white balance code. Meaning is vendor-specific.
    pub white_balance: Option<u16>,

    /// Raw metering mode code. Meaning is vendor-specific.
    pub metering_mode: Option<u16>,

    /// Raw exposure mode code. Meaning is vendor-specific.
    pub exposure_mode: Option<u16>,

    /// Digital zoom ratio (1.0 = no zoom).
    pub digital_zoom_ratio: Option<f32>,

    /// Raw color space code (e.g. sRGB = 1).
    pub color_space: Option<u16>,

    /// ExifImageOrientation value (1–8).
    pub orientation: Option<u16>,

    // --- flight (XMP DJI) ---
    /// Relative altitude in metres above take-off point (XMP DJI `RelativeAltitude`).
    pub relative_altitude_m: Option<f64>,

    /// Gimbal roll angle in degrees (XMP DJI `GimbalRollDegree`).
    pub gimbal_roll_degree: Option<f32>,

    /// Gimbal yaw angle in degrees (XMP DJI `GimbalYawDegree`).
    pub gimbal_yaw_degree: Option<f32>,

    /// Gimbal pitch angle in degrees (XMP DJI `GimbalPitchDegree`).
    pub gimbal_pitch_degree: Option<f32>,

    /// Aircraft roll angle in degrees (XMP DJI `FlightRollDegree`).
    pub flight_roll_degree: Option<f32>,

    /// Aircraft yaw angle in degrees (XMP DJI `FlightYawDegree`).
    pub flight_yaw_degree: Option<f32>,

    /// Aircraft pitch angle in degrees (XMP DJI `FlightPitchDegree`).
    pub flight_pitch_degree: Option<f32>,

    /// Aircraft horizontal speed along X axis in m/s (XMP DJI `FlightXSpeed`).
    pub flight_x_speed: Option<f32>,

    /// Aircraft horizontal speed along Y axis in m/s (XMP DJI `FlightYSpeed`).
    pub flight_y_speed: Option<f32>,

    /// Aircraft vertical speed in m/s (XMP DJI `FlightZSpeed`).
    pub flight_z_speed: Option<f32>,
}
