use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Dng,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ImageMetadata {
    // --- geolocation ---
    pub gps_latitude: Option<f64>,   // decimal degrees, S → negative
    pub gps_longitude: Option<f64>,  // decimal degrees, W → negative
    pub gps_altitude_m: Option<f64>, // EXIF; ref 1 → negative, ref missing → positive
    pub absolute_altitude_m: Option<f64>, // XMP DJI (barometer or ellipsoid RTK)

    // --- timestamp ---
    pub date_time_original: Option<NaiveDateTime>,

    // --- dimensions ---
    pub width: Option<u32>, // header file (JPEG) / tag IFD0 (DNG)
    pub height: Option<u32>,

    // --- imagery ---
    pub format: Option<ImageFormat>,
    pub make: Option<String>,
    pub camera_model_name: Option<String>,
    pub exposure_time_s: Option<f64>,
    pub f_number: Option<f32>,
    pub iso: Option<u16>,
    pub focal_length_mm: Option<f32>,
    pub focal_length_35mm: Option<u16>,
    pub flash: Option<u16>,         // raw code (bitfield)
    pub white_balance: Option<u16>, // raw code
    pub metering_mode: Option<u16>, // raw code
    pub exposure_mode: Option<u16>, // raw code
    pub digital_zoom_ratio: Option<f32>,
    pub color_space: Option<u16>, // raw code
    pub orientation: Option<u16>, // raw code

    // --- flight (XMP DJI) ---
    pub relative_altitude_m: Option<f64>,
    pub gimbal_roll_degree: Option<f32>,
    pub gimbal_yaw_degree: Option<f32>,
    pub gimbal_pitch_degree: Option<f32>,
    pub flight_roll_degree: Option<f32>,
    pub flight_yaw_degree: Option<f32>,
    pub flight_pitch_degree: Option<f32>,
    pub flight_x_speed: Option<f32>,
    pub flight_y_speed: Option<f32>,
    pub flight_z_speed: Option<f32>,
}
