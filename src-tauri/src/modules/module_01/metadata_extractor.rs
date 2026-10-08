#![allow(dead_code)]

//! Image Metadata Extraction (Module 1.3).
//!
//! Extracts GPS coordinates, acquisition date, altitude, resolution, and
//! camera/flight tags from EXIF and XMP DJI data embedded in aerial images,
//! populating [`ImageMetadata`](crate::models::ImageMetadata).
//!
//! ## Purity Boundary
//!
//! [`extract_metadata`] is the **only** impure function in this module — it
//! is the sole point that touches the filesystem. Every other function is a
//! pure transformation over bytes and values, so they can be exercised with
//! in-memory fixtures without a real file on disk.
//!
//! [`extract_metadata`] is defined as the composition:
//!
//! ```text
//! read file bytes
//!   -> detect_format(header, extension)
//!   -> read_dimensions(bytes, format)
//!   -> parse_exif(bytes)
//!   -> parse_xmp_dji(bytes)
//!   -> merge_tags(exif, xmp, dimensions, format)
//! ```
//!
//! A missing individual tag is not an error: fields are simply left as
//! `None` in [`ImageMetadata`]. [`MetadataError`] is reserved for cases
//! where the file itself cannot be read or is not a supported image format.
//!
//! ## Module Contracts
//!
//! | Function | Direction | Purpose |
//! |----------|-----------|---------|
//! | [`to_plugin_metadata`] | Module 1 → Module 2 | Pre-flight image metadata for `plugin_manager::payload::preflight_check` |
//! | [`describe_completeness`] | Module 1 → Module 1.5 | Missing-tag report feeding Image Quality Checking |

use crate::models::image::{ImageFormat, ImageMetadata};
use crate::modules::module_01::error::MetadataError;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Raw EXIF tags read from a JPEG APP1 segment or DNG IFD0, prior to being
/// merged into [`ImageMetadata`].
///
/// Field presence mirrors [`ImageMetadata`]'s EXIF-sourced fields; a `None`
/// means the tag was absent from the file, not that parsing failed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct RawExifTags {
    /// Raw `GPSLatitude` + `GPSLatitudeRef` as a signed decimal-degree value.
    pub gps_latitude: Option<f64>,
    /// Raw `GPSLongitude` + `GPSLongitudeRef` as a signed decimal-degree value.
    pub gps_longitude: Option<f64>,
    /// Raw `GPSAltitude` + `GPSAltitudeRef` in metres.
    pub gps_altitude_m: Option<f64>,
    /// Raw `DateTimeOriginal` tag, unparsed (EXIF format `YYYY:MM:DD HH:MM:SS`).
    pub date_time_original_raw: Option<String>,
    /// `Make` tag.
    pub make: Option<String>,
    /// `Model` tag.
    pub camera_model_name: Option<String>,
    /// `ExposureTime` tag in seconds.
    pub exposure_time_s: Option<f64>,
    /// `FNumber` tag.
    pub f_number: Option<f32>,
    /// `ISOSpeedRatings` tag.
    pub iso: Option<u32>,
    /// `FocalLength` tag in millimetres.
    pub focal_length_mm: Option<f32>,
    /// `FocalLengthIn35mmFilm` tag.
    pub focal_length_35mm: Option<u16>,
    /// `Flash` tag (raw bitfield).
    pub flash: Option<u16>,
    /// `WhiteBalance` tag (raw code).
    pub white_balance: Option<u16>,
    /// `MeteringMode` tag (raw code).
    pub metering_mode: Option<u16>,
    /// `ExposureMode` tag (raw code).
    pub exposure_mode: Option<u16>,
    /// `DigitalZoomRatio` tag.
    pub digital_zoom_ratio: Option<f32>,
    /// `ColorSpace` tag (raw code).
    pub color_space: Option<u16>,
    /// `Orientation` tag.
    pub orientation: Option<u16>,
}

/// Raw XMP DJI drone-telemetry tags read from a JPEG APP1 XMP packet.
///
/// `None` for any tag when the file has no DJI XMP packet (e.g. non-DJI
/// drones) or the specific tag is absent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct RawXmpTags {
    /// `drone-dji:AbsoluteAltitude`.
    pub absolute_altitude_m: Option<f64>,
    /// `drone-dji:RelativeAltitude`.
    pub relative_altitude_m: Option<f64>,
    /// `drone-dji:GimbalRollDegree`.
    pub gimbal_roll_degree: Option<f32>,
    /// `drone-dji:GimbalYawDegree`.
    pub gimbal_yaw_degree: Option<f32>,
    /// `drone-dji:GimbalPitchDegree`.
    pub gimbal_pitch_degree: Option<f32>,
    /// `drone-dji:FlightRollDegree`.
    pub flight_roll_degree: Option<f32>,
    /// `drone-dji:FlightYawDegree`.
    pub flight_yaw_degree: Option<f32>,
    /// `drone-dji:FlightPitchDegree`.
    pub flight_pitch_degree: Option<f32>,
    /// `drone-dji:FlightXSpeed`.
    pub flight_x_speed: Option<f32>,
    /// `drone-dji:FlightYSpeed`.
    pub flight_y_speed: Option<f32>,
    /// `drone-dji:FlightZSpeed`.
    pub flight_z_speed: Option<f32>,
}

/// Pixel dimensions read from a JPEG SOF0 segment or DNG IFD0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct PixelDimensions {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// A GPS coordinate expressed as degrees/minutes/seconds with a
/// hemisphere reference, as stored in EXIF `GPSLatitude`/`GPSLongitude`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct GpsCoordinate {
    /// Whole degrees component.
    pub degrees: f64,
    /// Minutes component.
    pub minutes: f64,
    /// Seconds component.
    pub seconds: f64,
    /// Hemisphere reference: `'N'`/`'S'` for latitude, `'E'`/`'W'` for longitude.
    pub reference: char,
}

/// Report of which [`ImageMetadata`] fields could and could not be
/// populated from a source image, consumed by Image Quality Checking (M1.5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct MetadataCompleteness {
    /// Names of fields that were successfully populated.
    pub present: Vec<String>,
    /// Names of fields that are `None` in the extracted metadata.
    pub missing: Vec<String>,
}

/// Extracts complete metadata for a single aerial image from disk.
///
/// # Purity
///
/// Impure — the only function in this module that performs file I/O. It is
/// defined as reading the file's bytes and then composing
/// [`detect_format`], [`read_dimensions`], [`parse_exif`], [`parse_xmp_dji`],
/// and [`merge_tags`] (see module-level docs for the exact pipeline).
///
/// # Arguments
///
/// * `path` — filesystem path to a JPEG or DNG aerial image.
///
/// # Returns
///
/// `Ok(ImageMetadata)` with every field populated that could be read; absent
/// tags are `None`, not an error.
///
/// # Errors
///
/// Returns [`MetadataError`] if the file cannot be read
/// ([`MetadataError::Io`]), is not a supported format
/// ([`MetadataError::UnsupportedFormat`], [`MetadataError::MagicMismatch`]),
/// or has a present-but-corrupt EXIF/XMP segment
/// ([`MetadataError::MalformedExif`], [`MetadataError::MalformedXmp`]).
/// Until the pipeline is implemented this returns
/// [`MetadataError::NotImplemented`] instead of panicking.
pub(crate) fn extract_metadata(path: &Path) -> Result<ImageMetadata, MetadataError> {
    Err(MetadataError::NotImplemented {
        path: path.display().to_string(),
    })
}

/// Determines the image format from the file extension and leading bytes.
///
/// # Purity
///
/// Pure.
///
/// # Arguments
///
/// * `header` — first bytes of the file, sufficient to check a magic number.
/// * `extension` — file extension without the leading dot (e.g. `"jpg"`).
///
/// # Returns
///
/// `Ok(ImageFormat)` matching both the extension and the magic bytes.
///
/// # Errors
///
/// [`MetadataError::UnsupportedFormat`] if the extension is not `jpg`/`jpeg`/`dng`.
/// [`MetadataError::MagicMismatch`] if the extension is supported but `header`
/// does not match its magic number.
///
/// A `.dng` file is a TIFF container, so the sniff accepts either TIFF magic
/// (little- or big-endian). A plain TIFF passes as DNG — a ≥16-byte header
/// cannot tell them apart. `path` in the returned errors is always empty:
/// this function has no path, and callers that have one (e.g.
/// `image_importer::classify_file`) overwrite it.
pub(crate) fn detect_format(header: &[u8], extension: &str) -> Result<ImageFormat, MetadataError> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => {
            if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
                Ok(ImageFormat::Jpeg)
            } else {
                Err(MetadataError::MagicMismatch {
                    path: String::new(),
                })
            }
        }
        "dng" => {
            if header.starts_with(&[0x49, 0x49, 0x2A, 0x00])
                || header.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
            {
                Ok(ImageFormat::Dng)
            } else {
                Err(MetadataError::MagicMismatch {
                    path: String::new(),
                })
            }
        }
        _ => Err(MetadataError::UnsupportedFormat {
            path: String::new(),
            extension,
        }),
    }
}

/// Parses the EXIF segment of an image into [`RawExifTags`].
///
/// # Purity
///
/// Pure.
///
/// # Arguments
///
/// * `bytes` — full contents of the source image file.
///
/// # Returns
///
/// `Ok(RawExifTags)` with every tag that was present; missing tags are
/// `None`.
///
/// # Errors
///
/// [`MetadataError::MalformedExif`] if an EXIF segment is present but its
/// structure cannot be parsed.
/// Byte order of an embedded TIFF header (EXIF in JPEG APP1, or a DNG file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TiffEndian {
    Little,
    Big,
}

fn u16_at(data: &[u8], off: usize, endian: TiffEndian) -> Option<u16> {
    let b = data.get(off..off + 2)?;
    Some(match endian {
        TiffEndian::Little => u16::from_le_bytes([b[0], b[1]]),
        TiffEndian::Big => u16::from_be_bytes([b[0], b[1]]),
    })
}

fn u32_at(data: &[u8], off: usize, endian: TiffEndian) -> Option<u32> {
    let b = data.get(off..off + 4)?;
    Some(match endian {
        TiffEndian::Little => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        TiffEndian::Big => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
    })
}

/// One 12-byte IFD entry. `val_bytes` holds the raw 4-byte value field so
/// inline values (total size <= 4) can be read without endian confusion;
/// `val_u32` holds the same field decoded as an offset when the value is
/// stored out-of-line.
struct IfdEntry {
    tag: u16,
    typ: u16,
    count: u32,
    val_bytes: [u8; 4],
    val_u32: u32,
}

fn type_size(typ: u16) -> Option<u64> {
    match typ {
        1 | 2 | 7 => Some(1), // BYTE, ASCII, UNDEFINED
        3 => Some(2),         // SHORT
        4 | 9 => Some(4),     // LONG, SLONG
        5 | 10 => Some(8),    // RATIONAL, SRATIONAL
        _ => None,
    }
}

fn parse_ifd(
    tiff: &[u8],
    endian: TiffEndian,
    ifd_offset: usize,
) -> Result<Vec<IfdEntry>, MetadataError> {
    let bad = || MetadataError::MalformedExif("truncated IFD".to_string());
    let count = u16_at(tiff, ifd_offset, endian).ok_or_else(bad)? as usize;
    let mut entries = Vec::with_capacity(count.min(64));
    for i in 0..count {
        let base = ifd_offset.checked_add(2 + i * 12).ok_or_else(bad)?;
        let entry_bytes = tiff.get(base..base + 12).ok_or_else(bad)?;
        let tag = u16_at(entry_bytes, 0, endian).ok_or_else(bad)?;
        let typ = u16_at(entry_bytes, 2, endian).ok_or_else(bad)?;
        let cnt = u32_at(entry_bytes, 4, endian).ok_or_else(bad)?;
        let mut val_bytes = [0u8; 4];
        val_bytes.copy_from_slice(&entry_bytes[8..12]);
        let val_u32 = u32_at(entry_bytes, 8, endian).ok_or_else(bad)?;
        entries.push(IfdEntry {
            tag,
            typ,
            count: cnt,
            val_bytes,
            val_u32,
        });
    }
    Ok(entries)
}

fn find_entry(entries: &[IfdEntry], tag: u16) -> Option<&IfdEntry> {
    entries.iter().find(|e| e.tag == tag)
}

/// Raw value bytes for an entry: inline when they fit in 4 bytes,
/// otherwise the slice at the stored offset.
fn entry_data<'a>(tiff: &'a [u8], entry: &'a IfdEntry) -> Option<&'a [u8]> {
    let size = type_size(entry.typ)?;
    let total = u64::from(entry.count).checked_mul(size)?;
    if total == 0 || total > 1_000_000 {
        return None;
    }
    if total <= 4 {
        // Inline: take the first `total` bytes of the value field in file
        // order (endian-independent for BYTE/ASCII/UNDEFINED; decoded
        // below for SHORT/LONG).
        Some(&entry.val_bytes[..total as usize])
    } else {
        let off = entry.val_u32 as usize;
        tiff.get(off..off.checked_add(total as usize)?)
    }
}

fn read_ascii(tiff: &[u8], entries: &[IfdEntry], tag: u16) -> Option<String> {
    let entry = find_entry(entries, tag)?;
    if entry.typ != 2 || entry.count == 0 {
        return None;
    }
    let data = entry_data(tiff, entry)?;
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let s = String::from_utf8_lossy(&data[..end]).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn read_u16_val(tiff: &[u8], entries: &[IfdEntry], endian: TiffEndian, tag: u16) -> Option<u16> {
    let entry = find_entry(entries, tag)?;
    let data = entry_data(tiff, entry)?;
    match entry.typ {
        3 => {
            if data.len() < 2 {
                return None;
            }
            Some(match endian {
                TiffEndian::Little => u16::from_le_bytes([data[0], data[1]]),
                TiffEndian::Big => u16::from_be_bytes([data[0], data[1]]),
            })
        }
        // Some writers emit these as LONG.
        4 => read_u32_val(tiff, entries, endian, tag).and_then(|v| u16::try_from(v).ok()),
        _ => None,
    }
}

fn read_u32_val(tiff: &[u8], entries: &[IfdEntry], endian: TiffEndian, tag: u16) -> Option<u32> {
    let entry = find_entry(entries, tag)?;
    let data = entry_data(tiff, entry)?;
    match entry.typ {
        3 => read_u16_val(tiff, entries, endian, tag).map(u32::from),
        4 => {
            if data.len() < 4 {
                return None;
            }
            Some(match endian {
                TiffEndian::Little => u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
                TiffEndian::Big => u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            })
        }
        _ => None,
    }
}

fn rational_at(tiff: &[u8], endian: TiffEndian, off: usize) -> Option<f64> {
    let num = u32_at(tiff, off, endian)? as f64;
    let den = u32_at(tiff, off + 4, endian)? as f64;
    if den == 0.0 {
        return None;
    }
    let v = num / den;
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

fn read_rational(tiff: &[u8], entries: &[IfdEntry], endian: TiffEndian, tag: u16) -> Option<f64> {
    let entry = find_entry(entries, tag)?;
    if entry.typ != 5 || entry.count == 0 {
        return None;
    }
    let data_off = if u64::from(entry.count) * 8 <= 4 {
        // Never happens for RATIONAL (8 bytes each), kept for completeness.
        return None;
    } else {
        entry.val_u32 as usize
    };
    rational_at(tiff, endian, data_off)
}

fn read_rationals3(
    tiff: &[u8],
    entries: &[IfdEntry],
    endian: TiffEndian,
    tag: u16,
) -> Option<[f64; 3]> {
    let entry = find_entry(entries, tag)?;
    if entry.typ != 5 || entry.count != 3 {
        return None;
    }
    let base = entry.val_u32 as usize;
    Some([
        rational_at(tiff, endian, base)?,
        rational_at(tiff, endian, base + 8)?,
        rational_at(tiff, endian, base + 16)?,
    ])
}

/// Validates the TIFF header and returns `(endian, ifd0_offset)`.
fn tiff_header(tiff: &[u8]) -> Option<(TiffEndian, usize)> {
    let magic = tiff.get(0..4)?;
    let endian = match magic {
        [0x49, 0x49, 0x2A, 0x00] => TiffEndian::Little,
        [0x4D, 0x4D, 0x00, 0x2A] => TiffEndian::Big,
        _ => return None,
    };
    let off = u32_at(tiff, 4, endian)? as usize;
    Some((endian, off))
}

fn is_tiff(bytes: &[u8]) -> bool {
    tiff_header(bytes).is_some()
}

/// Scans JPEG markers for the first APP1 `Exif\0\0` segment and returns the
/// TIFF payload inside it.
///
/// Returns `Ok(None)` when there is no EXIF segment (not an error — a
/// missing tag is `None`). Returns `Err(MalformedExif)` when an EXIF
/// segment is present but truncated, or when the JPEG marker structure
/// itself is truncated.
fn find_exif_tiff(bytes: &[u8]) -> Result<Option<&[u8]>, MetadataError> {
    let malformed = |msg: &str| MetadataError::MalformedExif(msg.to_string());
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return Ok(None);
    }
    let mut pos = 2;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            return Err(malformed("invalid JPEG marker"));
        }
        // Skip padding 0xFF bytes.
        let mut code_pos = pos + 1;
        while code_pos < bytes.len() && bytes[code_pos] == 0xFF {
            code_pos += 1;
        }
        if code_pos >= bytes.len() {
            break;
        }
        let code = bytes[code_pos];
        // Standalone markers without a length field.
        if code == 0xD8 || code == 0xD9 || (0xD0..=0xD7).contains(&code) || code == 0x01 {
            pos = code_pos + 1;
            continue;
        }
        if code_pos + 3 > bytes.len() {
            return Err(malformed("truncated JPEG segment header"));
        }
        let len = u16::from_be_bytes([bytes[code_pos + 1], bytes[code_pos + 2]]) as usize;
        if len < 2 {
            return Err(malformed("invalid JPEG segment length"));
        }
        let data_start = code_pos + 3;
        let data_end = data_start
            .checked_add(len - 2)
            .ok_or_else(|| malformed("overflow"))?;
        if data_end > bytes.len() {
            return Err(malformed("truncated JPEG segment"));
        }
        if code == 0xE1 {
            let seg = &bytes[data_start..data_end];
            if seg.len() > 6 && seg[..6] == [b'E', b'x', b'i', b'f', 0, 0] {
                let tiff = &seg[6..];
                if tiff_header(tiff).is_none() {
                    return Err(malformed("invalid EXIF TIFF header"));
                }
                return Ok(Some(tiff));
            }
        }
        // SOS (start of scan) ends the metadata section.
        if code == 0xDA {
            break;
        }
        pos = data_end;
    }
    Ok(None)
}

fn gps_decimal(
    tiff: &[u8],
    gps: &[IfdEntry],
    endian: TiffEndian,
    ref_tag: u16,
    val_tag: u16,
    negative_refs: [char; 2],
) -> Option<f64> {
    let reference = read_ascii(tiff, gps, ref_tag)?
        .chars()
        .next()
        .unwrap_or('?');
    let dms = read_rationals3(tiff, gps, endian, val_tag)?;
    let coord = GpsCoordinate {
        degrees: dms[0],
        minutes: dms[1],
        seconds: dms[2],
        reference,
    };
    let mut decimal = dms_to_decimal(coord);
    // `dms_to_decimal` already signs S/W; keep the sign only when the ref
    // is one of the expected pair, otherwise treat an unknown ref as
    // positive to avoid flipping on garbage.
    if reference == negative_refs[0] || reference == negative_refs[1] {
        decimal = -decimal.abs();
    } else {
        decimal = decimal.abs();
    }
    if decimal.is_finite() {
        Some(decimal)
    } else {
        None
    }
}

/// Fills [`RawExifTags`] from one TIFF payload (EXIF APP1 contents or a
/// whole DNG file).
fn parse_tiff_tags(tiff: &[u8]) -> Result<RawExifTags, MetadataError> {
    let malformed = |msg: &str| MetadataError::MalformedExif(msg.to_string());
    let (endian, ifd0_off) = tiff_header(tiff).ok_or_else(|| malformed("bad TIFF header"))?;
    if ifd0_off >= tiff.len() {
        return Err(malformed("IFD0 offset out of bounds"));
    }
    let ifd0 = parse_ifd(tiff, endian, ifd0_off)?;
    let mut out = RawExifTags {
        make: read_ascii(tiff, &ifd0, 0x010F),
        camera_model_name: read_ascii(tiff, &ifd0, 0x0110),
        orientation: read_u16_val(tiff, &ifd0, endian, 0x0112),
        ..RawExifTags::default()
    };

    // GPS sub-IFD (tag 0x8825 in IFD0).
    if let Some(gps_off) = read_u32_val(tiff, &ifd0, endian, 0x8825) {
        let gps_off = gps_off as usize;
        if gps_off >= tiff.len() {
            return Err(malformed("GPS IFD offset out of bounds"));
        }
        let gps = parse_ifd(tiff, endian, gps_off)?;
        out.gps_latitude = gps_decimal(tiff, &gps, endian, 1, 2, ['S', 's']);
        out.gps_longitude = gps_decimal(tiff, &gps, endian, 3, 4, ['W', 'w']);
        if let Some(alt) = read_rational(tiff, &gps, endian, 6) {
            // GPSAltitudeRef: 0 = above sea level, 1 = below. The tag is
            // type BYTE in most files, so read the first raw byte rather
            // than going through the SHORT decoder.
            let below = find_entry(&gps, 5).is_some_and(|e| {
                let first = if u64::from(e.count).saturating_mul(type_size(e.typ).unwrap_or(1)) <= 4
                {
                    e.val_bytes[0]
                } else {
                    tiff.get(e.val_u32 as usize).copied().unwrap_or(0)
                };
                first == 1
            });
            out.gps_altitude_m = Some(if below { -alt } else { alt });
        }
    }

    // Exif sub-IFD (tag 0x8769 in IFD0).
    if let Some(exif_off) = read_u32_val(tiff, &ifd0, endian, 0x8769) {
        let exif_off = exif_off as usize;
        if exif_off >= tiff.len() {
            return Err(malformed("Exif IFD offset out of bounds"));
        }
        let exif = parse_ifd(tiff, endian, exif_off)?;
        out.date_time_original_raw = read_ascii(tiff, &exif, 0x9003);
        out.exposure_time_s = read_rational(tiff, &exif, endian, 0x829A);
        out.f_number = read_rational(tiff, &exif, endian, 0x829D).map(|v| v as f32);
        out.iso = read_u16_val(tiff, &exif, endian, 0x8827)
            .map(u32::from)
            .or_else(|| read_u32_val(tiff, &exif, endian, 0x8827));
        out.focal_length_mm = read_rational(tiff, &exif, endian, 0x920A).map(|v| v as f32);
        out.focal_length_35mm = read_u16_val(tiff, &exif, endian, 0xA405);
        out.flash = read_u16_val(tiff, &exif, endian, 0x9209);
        out.white_balance = read_u16_val(tiff, &exif, endian, 0xA403);
        out.metering_mode = read_u16_val(tiff, &exif, endian, 0x9207);
        out.exposure_mode = read_u16_val(tiff, &exif, endian, 0xA402);
        out.digital_zoom_ratio = read_rational(tiff, &exif, endian, 0xA404).map(|v| v as f32);
        out.color_space = read_u16_val(tiff, &exif, endian, 0xA001);
    }

    Ok(out)
}

/// Reads JPEG dimensions by scanning for the first SOF0–SOF3 marker.
fn jpeg_dimensions(bytes: &[u8]) -> Result<PixelDimensions, MetadataError> {
    let malformed = |msg: &str| MetadataError::MalformedExif(msg.to_string());
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return Err(malformed("not a JPEG"));
    }
    let mut pos = 2;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            return Err(malformed("invalid JPEG marker"));
        }
        let mut code_pos = pos + 1;
        while code_pos < bytes.len() && bytes[code_pos] == 0xFF {
            code_pos += 1;
        }
        if code_pos >= bytes.len() {
            break;
        }
        let code = bytes[code_pos];
        if code == 0xD8 || code == 0xD9 || (0xD0..=0xD7).contains(&code) || code == 0x01 {
            pos = code_pos + 1;
            continue;
        }
        if code_pos + 3 > bytes.len() {
            return Err(malformed("truncated JPEG segment header"));
        }
        let len = u16::from_be_bytes([bytes[code_pos + 1], bytes[code_pos + 2]]) as usize;
        if len < 2 {
            return Err(malformed("invalid JPEG segment length"));
        }
        let data_start = code_pos + 3;
        let data_end = data_start
            .checked_add(len - 2)
            .ok_or_else(|| malformed("overflow"))?;
        if data_end > bytes.len() {
            return Err(malformed("truncated JPEG segment"));
        }
        if matches!(code, 0xC0..=0xC3) {
            let seg = &bytes[data_start..data_end];
            if seg.len() < 7 {
                return Err(malformed("truncated SOF segment"));
            }
            let height = u16::from_be_bytes([seg[1], seg[2]]) as u32;
            let width = u16::from_be_bytes([seg[3], seg[4]]) as u32;
            if width == 0 || height == 0 {
                return Err(malformed("invalid SOF dimensions"));
            }
            return Ok(PixelDimensions { width, height });
        }
        if code == 0xDA {
            break;
        }
        pos = data_end;
    }
    Err(malformed("SOF marker not found"))
}

/// Reads DNG/TIFF dimensions from IFD0 ImageWidth (256) / ImageLength (257).
fn tiff_dimensions(tiff: &[u8]) -> Result<PixelDimensions, MetadataError> {
    let malformed = |msg: &str| MetadataError::MalformedExif(msg.to_string());
    let (endian, ifd0_off) = tiff_header(tiff).ok_or_else(|| malformed("bad TIFF header"))?;
    let ifd0 = parse_ifd(tiff, endian, ifd0_off)?;
    let width =
        read_u32_val(tiff, &ifd0, endian, 256).ok_or_else(|| malformed("missing ImageWidth"))?;
    let height =
        read_u32_val(tiff, &ifd0, endian, 257).ok_or_else(|| malformed("missing ImageLength"))?;
    if width == 0 || height == 0 {
        return Err(malformed("invalid TIFF dimensions"));
    }
    Ok(PixelDimensions { width, height })
}

fn parse_exif(bytes: &[u8]) -> Result<RawExifTags, MetadataError> {
    // DNG files are TIFF containers: the whole file is the TIFF payload.
    if is_tiff(bytes) {
        return parse_tiff_tags(bytes);
    }
    // JPEG: look for an APP1 Exif segment. Absence is not an error.
    match find_exif_tiff(bytes)? {
        Some(tiff) => parse_tiff_tags(tiff),
        None => Ok(RawExifTags::default()),
    }
}

/// Parses the XMP DJI drone-telemetry packet of an image into [`RawXmpTags`].
///
/// # Purity
///
/// Pure.
///
/// # Arguments
///
/// * `bytes` — full contents of the source image file.
///
/// # Returns
///
/// `Ok(RawXmpTags)` with every tag that was present. Returns all-`None`
/// fields (not an error) for non-DJI images with no XMP DJI packet.
///
/// # Errors
///
/// [`MetadataError::MalformedXmp`] if an XMP packet is present but is not
/// well-formed XML, or the DJI namespace is present but malformed.
fn parse_xmp_dji(bytes: &[u8]) -> Result<RawXmpTags, MetadataError> {
    let _ = bytes;
    unimplemented!("M1.3: parse the drone-dji XMP namespace")
}

/// Reads pixel dimensions from a JPEG SOF0 segment or DNG IFD0.
///
/// # Purity
///
/// Pure.
///
/// # Arguments
///
/// * `bytes` — full contents of the source image file.
/// * `format` — image format, as returned by [`detect_format`].
///
/// # Returns
///
/// `Ok(PixelDimensions)` with the image's width and height in pixels.
///
/// # Errors
///
/// [`MetadataError::MalformedExif`] if the relevant header cannot be located
/// or parsed for the given `format`.
fn read_dimensions(bytes: &[u8], format: ImageFormat) -> Result<PixelDimensions, MetadataError> {
    match format {
        ImageFormat::Jpeg => jpeg_dimensions(bytes),
        ImageFormat::Dng => tiff_dimensions(bytes),
    }
}

/// Converts a degrees/minutes/seconds GPS coordinate to signed decimal degrees.
///
/// # Purity
///
/// Pure, total (never fails).
///
/// # Arguments
///
/// * `coord` — degrees/minutes/seconds value with a hemisphere reference.
///
/// # Returns
///
/// Decimal degrees, negative when `reference` is `'S'` or `'W'`.
fn dms_to_decimal(coord: GpsCoordinate) -> f64 {
    let decimal = coord.degrees.abs() + coord.minutes.abs() / 60.0 + coord.seconds.abs() / 3_600.0;
    match coord.reference {
        'S' | 's' | 'W' | 'w' => -decimal,
        _ => decimal,
    }
}

/// Parses an EXIF `DateTimeOriginal` string into a [`NaiveDateTime`].
///
/// # Purity
///
/// Pure.
///
/// # Arguments
///
/// * `raw` — EXIF date/time string, format `"YYYY:MM:DD HH:MM:SS"`.
///
/// # Returns
///
/// `Some(NaiveDateTime)` if `raw` matches the expected EXIF format,
/// otherwise `None` (not an error — malformed or missing timestamps are
/// tolerated).
fn parse_exif_datetime(raw: &str) -> Option<NaiveDateTime> {
    let s = raw.trim().trim_matches('\0').trim();
    if s.is_empty() {
        return None;
    }
    // Canonical EXIF form. Anything else (empty, truncated, wrong
    // separators) is tolerated as "no timestamp", not an error.
    NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S").ok()
}

/// Merges parsed EXIF tags, XMP DJI tags, and pixel dimensions into a single
/// [`ImageMetadata`].
///
/// # Purity
///
/// Pure, total (never fails — inputs are already-parsed values).
///
/// # Arguments
///
/// * `exif` — EXIF tags from [`parse_exif`].
/// * `xmp` — XMP DJI tags from [`parse_xmp_dji`].
/// * `dimensions` — pixel dimensions from [`read_dimensions`].
/// * `format` — image format from [`detect_format`].
///
/// # Returns
///
/// A fully assembled [`ImageMetadata`] combining all inputs.
fn merge_tags(
    exif: RawExifTags,
    xmp: RawXmpTags,
    dimensions: PixelDimensions,
    format: ImageFormat,
) -> ImageMetadata {
    ImageMetadata {
        gps_latitude: exif.gps_latitude,
        gps_longitude: exif.gps_longitude,
        gps_altitude_m: exif.gps_altitude_m,
        absolute_altitude_m: xmp.absolute_altitude_m,
        date_time_original: exif
            .date_time_original_raw
            .as_deref()
            .and_then(parse_exif_datetime),
        width: Some(dimensions.width),
        height: Some(dimensions.height),
        format: Some(format),
        make: exif.make,
        camera_model_name: exif.camera_model_name,
        exposure_time_s: exif.exposure_time_s,
        f_number: exif.f_number,
        iso: exif.iso,
        focal_length_mm: exif.focal_length_mm,
        focal_length_35mm: exif.focal_length_35mm,
        flash: exif.flash,
        white_balance: exif.white_balance,
        metering_mode: exif.metering_mode,
        exposure_mode: exif.exposure_mode,
        digital_zoom_ratio: exif.digital_zoom_ratio,
        color_space: exif.color_space,
        orientation: exif.orientation,
        relative_altitude_m: xmp.relative_altitude_m,
        gimbal_roll_degree: xmp.gimbal_roll_degree,
        gimbal_yaw_degree: xmp.gimbal_yaw_degree,
        gimbal_pitch_degree: xmp.gimbal_pitch_degree,
        flight_roll_degree: xmp.flight_roll_degree,
        flight_yaw_degree: xmp.flight_yaw_degree,
        flight_pitch_degree: xmp.flight_pitch_degree,
        flight_x_speed: xmp.flight_x_speed,
        flight_y_speed: xmp.flight_y_speed,
        flight_z_speed: xmp.flight_z_speed,
    }
}

/// Reports which [`ImageMetadata`] fields are present versus missing.
///
/// # Integration
///
/// - **Direction**: Module 1.3 → Module 1.5 (Image Quality Checking)
///
/// # Purity
///
/// Pure, total (never fails).
///
/// # Arguments
///
/// * `metadata` — previously extracted image metadata.
///
/// # Returns
///
/// A [`MetadataCompleteness`] listing present and missing field names, used
/// by Image Quality Checking to flag images with missing metadata.
pub(crate) fn describe_completeness(metadata: &ImageMetadata) -> MetadataCompleteness {
    let _ = metadata;
    unimplemented!("M1.3: walk ImageMetadata's Option fields and bucket by presence")
}

/// Projects [`ImageMetadata`] into the minimal shape Module 2 needs for
/// plugin pre-flight validation.
///
/// # Integration
///
/// - **Direction**: Module 1.3 → Module 2
/// - **Consumer**: [`crate::plugin_manager::payload::preflight_check`]
///
/// # Purity
///
/// Pure, total (never fails).
///
/// # Arguments
///
/// * `metadata` — previously extracted image metadata.
/// * `path` — filesystem path of the source image, echoed into the payload.
///
/// # Returns
///
/// A [`crate::plugin_manager::payload::ImageMetadata`] with `has_gps` set
/// when both latitude and longitude are present, and `format` rendered as
/// a lowercase string (`"jpeg"` / `"dng"`).
///
/// # Module Contract
///
/// This function represents the cross-module contract:
/// **Module 1.3 → Module 2 (pre-flight check)**.
pub(crate) fn to_plugin_metadata(
    metadata: &ImageMetadata,
    path: &Path,
) -> crate::plugin_manager::payload::ImageMetadata {
    let _ = (metadata, path);
    unimplemented!("M1.3: project ImageMetadata into the plugin_manager pre-flight shape")
}

/// Resolves a provider-specific metadata field name to this module's
/// canonical field name using a user-supplied alias table.
///
/// Different image/drone providers label the same logical field with
/// different strings (e.g. `"Tinggi_meter"` vs. this module's canonical
/// `"height_meter"`). The platform does not guess at such matches; the user
/// supplies the alias table explicitly, so a wrong or missing mapping is the
/// user's responsibility, not an inference the platform performed for them.
///
/// # Purity
///
/// Pure, total (never fails).
///
/// # Arguments
///
/// * `aliases` — user-supplied mapping from a provider's raw field name to
///   this module's canonical field name. Keys are matched case-insensitively.
/// * `raw_key` — the field name as it appears in the provider's metadata.
///
/// # Returns
///
/// `Some(canonical_name)` if `raw_key` case-insensitively matches a key in
/// `aliases`, otherwise `None`.
pub(crate) fn resolve_field_alias(
    aliases: &HashMap<String, String>,
    raw_key: &str,
) -> Option<String> {
    let _ = (aliases, raw_key);
    unimplemented!(
        "M1.3: user-provided provider-field-name -> canonical-field-name lookup, case-insensitive"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg_with_sof(width: u16, height: u16) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8];
        // SOF0 segment: length 11, precision 8, h, w.
        v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        v.extend_from_slice(&height.to_be_bytes());
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        v.extend_from_slice(&[0xFF, 0xD9]);
        v
    }

    fn tiff_le_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut v = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        v.extend_from_slice(&2u16.to_le_bytes());
        // ImageWidth 256, LONG, count 1.
        v.extend_from_slice(&256u16.to_le_bytes());
        v.extend_from_slice(&4u16.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&width.to_le_bytes());
        // ImageLength 257.
        v.extend_from_slice(&257u16.to_le_bytes());
        v.extend_from_slice(&4u16.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&height.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // next IFD
        v
    }

    fn tiff_le_with_make(make: &str) -> Vec<u8> {
        let mut s = make.as_bytes().to_vec();
        s.push(0);
        let str_off = 8 + 2 + 12 + 4; // header + count + 1 entry + next
        let mut v = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&0x010Fu16.to_le_bytes()); // Make
        v.extend_from_slice(&2u16.to_le_bytes()); // ASCII
        v.extend_from_slice(&(s.len() as u32).to_le_bytes());
        v.extend_from_slice(&(str_off as u32).to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&s);
        v
    }

    #[test]
    fn dms_signs_hemisphere() {
        let n = GpsCoordinate {
            degrees: 7.0,
            minutes: 30.0,
            seconds: 0.0,
            reference: 'N',
        };
        let s = GpsCoordinate {
            reference: 'S',
            ..n
        };
        assert!((dms_to_decimal(n) - 7.5).abs() < 1e-9);
        assert!((dms_to_decimal(s) + 7.5).abs() < 1e-9);
        let e = GpsCoordinate {
            degrees: 110.0,
            minutes: 0.0,
            seconds: 36.0,
            reference: 'E',
        };
        let w = GpsCoordinate {
            reference: 'W',
            ..e
        };
        assert!((dms_to_decimal(e) - 110.01).abs() < 1e-9);
        assert!((dms_to_decimal(w) + 110.01).abs() < 1e-9);
    }

    #[test]
    fn exif_datetime_valid_and_tolerant() {
        let dt = parse_exif_datetime("2026:09:18 07:12:00").expect("valid EXIF date");
        assert_eq!(dt.to_string(), "2026-09-18 07:12:00");
        assert_eq!(parse_exif_datetime(""), None);
        assert_eq!(parse_exif_datetime("not-a-date"), None);
        assert_eq!(parse_exif_datetime("2026-09-18 07:12:00"), None);
    }

    #[test]
    fn jpeg_sof_dimensions() {
        let bytes = jpeg_with_sof(300, 100);
        let dims = read_dimensions(&bytes, ImageFormat::Jpeg).expect("sof");
        assert_eq!((dims.width, dims.height), (300, 100));
    }

    #[test]
    fn jpeg_without_sof_errors() {
        let bytes = vec![0xFF, 0xD8, 0xFF, 0xD9];
        assert!(read_dimensions(&bytes, ImageFormat::Jpeg).is_err());
    }

    #[test]
    fn tiff_ifd_dimensions() {
        let bytes = tiff_le_with_dimensions(640, 480);
        let dims = read_dimensions(&bytes, ImageFormat::Dng).expect("ifd0");
        assert_eq!((dims.width, dims.height), (640, 480));
    }

    #[test]
    fn parse_exif_absent_is_default_not_error() {
        let bytes = jpeg_with_sof(10, 10);
        let tags = parse_exif(&bytes).expect("no EXIF -> default");
        assert_eq!(tags, RawExifTags::default());
    }

    #[test]
    fn parse_exif_truncated_segment_is_malformed() {
        // SOI + APP1 header claiming more bytes than exist.
        let bytes = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x20, b'E', b'x'];
        assert!(parse_exif(&bytes).is_err());
    }

    #[test]
    fn parse_exif_reads_make_from_tiff() {
        let bytes = tiff_le_with_make("TestCam");
        let tags = parse_exif(&bytes).expect("make");
        assert_eq!(tags.make.as_deref(), Some("TestCam"));
    }

    #[test]
    fn merge_folds_date_dimensions_format() {
        let exif = RawExifTags {
            gps_latitude: Some(-7.5),
            gps_longitude: Some(110.0),
            gps_altitude_m: Some(120.0),
            date_time_original_raw: Some("2026:09:18 07:12:00".to_string()),
            make: Some("DJI".to_string()),
            ..RawExifTags::default()
        };
        let meta = merge_tags(
            exif,
            RawXmpTags::default(),
            PixelDimensions {
                width: 4000,
                height: 3000,
            },
            ImageFormat::Jpeg,
        );
        assert_eq!(meta.gps_latitude, Some(-7.5));
        assert_eq!(meta.gps_altitude_m, Some(120.0));
        assert_eq!(meta.width, Some(4000));
        assert_eq!(meta.height, Some(3000));
        assert_eq!(meta.format, Some(ImageFormat::Jpeg));
        assert_eq!(meta.make.as_deref(), Some("DJI"));
        assert_eq!(
            meta.date_time_original.map(|d| d.to_string()),
            Some("2026-09-18 07:12:00".to_string())
        );
    }
}
