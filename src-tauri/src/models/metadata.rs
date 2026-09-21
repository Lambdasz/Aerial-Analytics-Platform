//! Aerial image metadata models extracted from EXIF and XMP DJI tags.
//!
//! These structs represent the canonical metadata schema used across
//! Module 1 (Image Management), Module 3 (Map Explorer), and the
//! plugin pre-flight validation pipeline.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

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
