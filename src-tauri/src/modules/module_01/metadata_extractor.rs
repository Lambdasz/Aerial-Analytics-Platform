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
//! `None` in [`ImageMetadata`]. A malformed XMP DJI packet counts as a
//! missing tag, not a failure — optional telemetry never discards the rest
//! of the extraction. [`MetadataError`] is reserved for cases where the
//! file itself cannot be read, is not a supported image format, or has a
//! present-but-corrupt EXIF segment.
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
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

/// Folds a field name into an alias-table key: NFC-composed, then
/// Unicode-lowercased, so `Élévation` matches `élévation` and composed
/// matches decomposed accents. Deliberately conservative: it only has to make
/// user-typed field names compare sensibly, and is unrelated to how any
/// filesystem compares file names (see `image_importer::NameFolding`).
fn normalize_alias_key(name: &str) -> String {
    name.nfc().collect::<String>().to_lowercase()
}

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
/// or has a present-but-corrupt EXIF segment
/// ([`MetadataError::MalformedExif`]). A malformed XMP DJI packet is not an
/// error: it leaves the flight-telemetry fields `None` like a missing tag.
pub(crate) fn extract_metadata(path: &Path) -> Result<ImageMetadata, MetadataError> {
    let mut file = std::fs::File::open(path)?;
    let extension = path
        .extension()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let mut header = Vec::with_capacity(16);
    file.by_ref().take(16).read_to_end(&mut header)?;
    // detect_format has no path, so its errors carry an empty one; fill in
    // the file that actually failed.
    let format = detect_format(&header, extension).map_err(|err| match err {
        MetadataError::UnsupportedFormat { extension, .. } => MetadataError::UnsupportedFormat {
            path: path.display().to_string(),
            extension,
        },
        MetadataError::MagicMismatch { .. } => MetadataError::MagicMismatch {
            path: path.display().to_string(),
        },
        other => other,
    })?;
    file.seek(SeekFrom::Start(0))?;
    // A JPEG keeps everything this module reads before the scan data, so
    // only that prefix is loaded. A DNG is a TIFF container whose parsers
    // index the whole file, so it is read in full.
    let bytes = match format {
        // `read_jpeg_head` reads byte by byte; buffer so that is not a syscall each.
        ImageFormat::Jpeg => read_jpeg_head(&mut BufReader::with_capacity(1 << 16, &mut file))?,
        ImageFormat::Dng => {
            let mut all = Vec::with_capacity(file.metadata()?.len() as usize + 1);
            file.read_to_end(&mut all)?;
            all
        }
    };
    let dimensions = read_dimensions(&bytes, format)?;
    let exif = parse_exif(&bytes)?;
    // Never fails: a malformed XMP packet leaves flight telemetry unset
    // instead of discarding the EXIF, GPS and dimensions above.
    let xmp = xmp_packet(&bytes, format)
        .map(parse_xmp_dji)
        .unwrap_or_default();
    Ok(merge_tags(exif, xmp, dimensions, format))
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
        3 => u16_at(data, 0, endian),
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
        4 => u32_at(data, 0, endian),
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

fn malformed(msg: &str) -> MetadataError {
    MetadataError::MalformedExif(msg.to_string())
}

fn is_tiff(bytes: &[u8]) -> bool {
    tiff_header(bytes).is_some()
}

/// Markers that stand alone, with no length field: `SOI`, `EOI`, `RSTn`,
/// `TEM`. The one definition both JPEG scanners use.
fn is_standalone_marker(code: u8) -> bool {
    code == 0xD8 || code == 0xD9 || (0xD0..=0xD7).contains(&code) || code == 0x01
}

/// Payload size of a marker segment from its big-endian length field, which
/// counts itself. `None` when the field is below its own 2 bytes.
fn segment_payload_len(length_field: [u8; 2]) -> Option<usize> {
    usize::from(u16::from_be_bytes(length_field)).checked_sub(2)
}

/// Walks the JPEG marker segments after SOI, yielding `(marker, payload)`.
///
/// Padding `0xFF` bytes and standalone markers (`SOI`, `EOI`, `RSTn`, `TEM`)
/// are skipped. The walk ends after the SOS segment (the metadata section is
/// over) or after the first error. Yields nothing when `bytes` is not a JPEG.
fn jpeg_segments(bytes: &[u8]) -> impl Iterator<Item = Result<(u8, &[u8]), MetadataError>> {
    let mut pos = 2;
    let mut done = bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8];
    std::iter::from_fn(move || {
        while !done && pos + 4 <= bytes.len() {
            if bytes[pos] != 0xFF {
                done = true;
                return Some(Err(malformed("invalid JPEG marker")));
            }
            let mut code_pos = pos + 1;
            while code_pos < bytes.len() && bytes[code_pos] == 0xFF {
                code_pos += 1;
            }
            if code_pos >= bytes.len() {
                break;
            }
            let code = bytes[code_pos];
            if is_standalone_marker(code) {
                pos = code_pos + 1;
                continue;
            }
            if code_pos + 3 > bytes.len() {
                done = true;
                return Some(Err(malformed("truncated JPEG segment header")));
            }
            let Some(payload_len) = segment_payload_len([bytes[code_pos + 1], bytes[code_pos + 2]])
            else {
                done = true;
                return Some(Err(malformed("invalid JPEG segment length")));
            };
            let data_start = code_pos + 3;
            let data_end = data_start + payload_len;
            if data_end > bytes.len() {
                done = true;
                return Some(Err(malformed("truncated JPEG segment")));
            }
            // SOS (start of scan) ends the metadata section.
            done = code == 0xDA;
            pos = data_end;
            return Some(Ok((code, &bytes[data_start..data_end])));
        }
        None
    })
}

/// Scans JPEG markers for the first APP1 `Exif\0\0` segment and returns the
/// TIFF payload inside it.
///
/// Returns `Ok(None)` when there is no EXIF segment (not an error — a
/// missing tag is `None`). Returns `Err(MalformedExif)` when an EXIF
/// segment is present but truncated, or when the JPEG marker structure
/// itself is truncated.
fn find_exif_tiff(bytes: &[u8]) -> Result<Option<&[u8]>, MetadataError> {
    for segment in jpeg_segments(bytes) {
        let (code, seg) = segment?;
        if code == 0xE1 && seg.len() > 6 && seg[..6] == [b'E', b'x', b'i', b'f', 0, 0] {
            let tiff = &seg[6..];
            if tiff_header(tiff).is_none() {
                return Err(MetadataError::MalformedExif(
                    "invalid EXIF TIFF header".to_string(),
                ));
            }
            return Ok(Some(tiff));
        }
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
    // A pointer that leads nowhere (GPS data stripped, tag left behind) just
    // leaves the GPS fields unset; it must not discard the rest.
    if let Some(gps) = read_u32_val(tiff, &ifd0, endian, 0x8825)
        .and_then(|off| parse_ifd(tiff, endian, off as usize).ok())
    {
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
    if let Some(exif) = read_u32_val(tiff, &ifd0, endian, 0x8769)
        .and_then(|off| parse_ifd(tiff, endian, off as usize).ok())
    {
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

/// Whether `code` is a start-of-frame marker (SOF0–SOF15 minus DHT `0xC4`,
/// JPG `0xC8` and DAC `0xCC`), so progressive, extended and arithmetic-coded
/// JPEGs report their dimensions too.
fn is_sof_marker(code: u8) -> bool {
    matches!(code, 0xC0..=0xCF) && !matches!(code, 0xC4 | 0xC8 | 0xCC)
}

/// Reads JPEG dimensions by scanning for the first SOF marker.
fn jpeg_dimensions(bytes: &[u8]) -> Result<PixelDimensions, MetadataError> {
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return Err(malformed("not a JPEG"));
    }
    for segment in jpeg_segments(bytes) {
        let (code, seg) = segment?;
        if is_sof_marker(code) {
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
    }
    Err(malformed("SOF marker not found"))
}

/// `(width, height)` of one IFD, `None` when either is absent or zero.
fn ifd_dimensions(tiff: &[u8], entries: &[IfdEntry], endian: TiffEndian) -> Option<(u32, u32)> {
    let width = read_u32_val(tiff, entries, endian, 256)?;
    let height = read_u32_val(tiff, entries, endian, 257)?;
    (width != 0 && height != 0).then_some((width, height))
}

/// Reads DNG/TIFF dimensions of the full-resolution image.
///
/// Many DNGs store a reduced-resolution preview in IFD0
/// (`NewSubfileType` bit 0 set) and the real image in a SubIFD (tag `0x14A`).
/// Candidates are IFD0 plus every readable SubIFD; the largest one that is
/// not flagged reduced-resolution wins, falling back to the largest overall.
fn tiff_dimensions(tiff: &[u8]) -> Result<PixelDimensions, MetadataError> {
    let (endian, ifd0_off) = tiff_header(tiff).ok_or_else(|| malformed("bad TIFF header"))?;
    let ifd0 = parse_ifd(tiff, endian, ifd0_off)?;
    let ifd0_dims = ifd_dimensions(tiff, &ifd0, endian);

    // (reduced_resolution, width, height)
    let mut candidates: Vec<(bool, u32, u32)> = Vec::new();
    let is_reduced = |entries: &[IfdEntry]| {
        read_u32_val(tiff, entries, endian, 254).is_some_and(|kind| kind & 1 == 1)
    };
    if let Some((w, h)) = ifd0_dims {
        candidates.push((is_reduced(&ifd0), w, h));
    }
    if let Some(sub) = find_entry(&ifd0, 0x14A) {
        // Offsets are LONG; a lone offset is stored inline in the value field.
        let offsets: Vec<u32> = match entry_data(tiff, sub) {
            Some(data) if sub.typ == 4 => (0..data.len() / 4)
                .filter_map(|i| u32_at(data, i * 4, endian))
                .collect(),
            _ => Vec::new(),
        };
        for off in offsets {
            let Ok(entries) = parse_ifd(tiff, endian, off as usize) else {
                continue;
            };
            if let Some((w, h)) = ifd_dimensions(tiff, &entries, endian) {
                candidates.push((is_reduced(&entries), w, h));
            }
        }
    }

    let area = |(_, w, h): &(bool, u32, u32)| u64::from(*w) * u64::from(*h);
    let best = candidates
        .iter()
        .filter(|c| !c.0)
        .max_by_key(|c| area(c))
        .or_else(|| candidates.iter().max_by_key(|c| area(c)));
    match best {
        Some(&(_, width, height)) => Ok(PixelDimensions { width, height }),
        // Nothing usable anywhere: report the most specific IFD0 problem.
        None if read_u32_val(tiff, &ifd0, endian, 256).is_none() => {
            Err(malformed("missing ImageWidth"))
        }
        None if read_u32_val(tiff, &ifd0, endian, 257).is_none() => {
            Err(malformed("missing ImageLength"))
        }
        None => Err(malformed("invalid TIFF dimensions")),
    }
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

/// Parses a DJI numeric attribute leniently.
///
/// `None` for an empty, non-numeric or non-finite value (`"n/a"`, text, a
/// truncated write, `nan`/`inf`, which `str::parse` would otherwise accept). The field is left unset instead of failing the extraction.
fn parse_dji_f64(value: &str) -> Option<f64> {
    value.trim().parse().ok().filter(|v: &f64| v.is_finite())
}

fn parse_dji_f32(value: &str) -> Option<f32> {
    value.trim().parse().ok().filter(|v: &f32| v.is_finite())
}

/// Assigns one parsed `drone-dji:Name="value"` attribute to its field.
///
/// Non-numeric values leave the field unset; a valid value already read
/// from the same tag is kept. Unknown attributes (e.g. new firmware tags)
/// are ignored, so newer packets keep parsing.
/// Overwrites `slot` only when `new` parsed; an invalid repeat keeps the old value.
fn keep<T: Copy>(slot: &mut Option<T>, new: Option<T>) {
    *slot = new.or(*slot);
}

fn assign_dji_tag(tags: &mut RawXmpTags, name: &str, value: &str) {
    match name {
        "AbsoluteAltitude" => keep(&mut tags.absolute_altitude_m, parse_dji_f64(value)),
        "RelativeAltitude" => keep(&mut tags.relative_altitude_m, parse_dji_f64(value)),
        "GimbalRollDegree" => keep(&mut tags.gimbal_roll_degree, parse_dji_f32(value)),
        "GimbalYawDegree" => keep(&mut tags.gimbal_yaw_degree, parse_dji_f32(value)),
        "GimbalPitchDegree" => keep(&mut tags.gimbal_pitch_degree, parse_dji_f32(value)),
        "FlightRollDegree" => keep(&mut tags.flight_roll_degree, parse_dji_f32(value)),
        "FlightYawDegree" => keep(&mut tags.flight_yaw_degree, parse_dji_f32(value)),
        "FlightPitchDegree" => keep(&mut tags.flight_pitch_degree, parse_dji_f32(value)),
        "FlightXSpeed" => keep(&mut tags.flight_x_speed, parse_dji_f32(value)),
        "FlightYSpeed" => keep(&mut tags.flight_y_speed, parse_dji_f32(value)),
        "FlightZSpeed" => keep(&mut tags.flight_z_speed, parse_dji_f32(value)),
        _ => {}
    }
}

/// Byte span of the XMP packet, or `None` when the file carries none.
///
/// Only the packet is scanned, so a JPEG comment (or any other stray bytes)
/// containing `drone-dji:` cannot be mistaken for DJI telemetry.
fn xmp_packet_span(text: &str) -> Option<(usize, usize)> {
    let start = text.find("<x:xmpmeta").or_else(|| text.find("<?xpacket"))?;
    let rest = &text[start..];
    let end = rest
        .find("</x:xmpmeta>")
        .map(|i| start + i + "</x:xmpmeta>".len())
        .or_else(|| rest.find("<?xpacket end").map(|i| start + i))
        .unwrap_or(text.len());
    Some((start, end.max(start)))
}

/// Reads a JPEG from its start through the SOS segment header, leaving the
/// entropy-coded scan data (the bulk of the file) unread.
///
/// SOF (dimensions), APP1 Exif and APP1 XMP all precede SOS, so this prefix
/// is all the extractor needs. A truncated or invalid structure is not an
/// error here: whatever was read is returned and [`jpeg_segments`] reports
/// the exact problem.
fn read_jpeg_head<R: Read>(reader: &mut R) -> std::io::Result<Vec<u8>> {
    fn read_byte<R: Read>(reader: &mut R, out: &mut Vec<u8>) -> std::io::Result<Option<u8>> {
        let mut byte = [0u8; 1];
        match reader.read_exact(&mut byte) {
            Ok(()) => {
                out.push(byte[0]);
                Ok(Some(byte[0]))
            }
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
            Err(err) => Err(err),
        }
    }

    let mut out = Vec::new();
    reader.by_ref().take(2).read_to_end(&mut out)?;
    if out != [0xFF, 0xD8] {
        // Not a JPEG start; let the parsers report it from what we have.
        reader.by_ref().take(1 << 16).read_to_end(&mut out)?;
        return Ok(out);
    }
    loop {
        // Marker: 0xFF, optional 0xFF padding, then the code.
        match read_byte(reader, &mut out)? {
            Some(0xFF) => {}
            _ => return Ok(out),
        }
        let code = loop {
            match read_byte(reader, &mut out)? {
                Some(0xFF) => continue,
                Some(code) => break code,
                None => return Ok(out),
            }
        };
        if is_standalone_marker(code) {
            continue;
        }
        let mut len_bytes = Vec::with_capacity(2);
        reader.by_ref().take(2).read_to_end(&mut len_bytes)?;
        out.extend_from_slice(&len_bytes);
        let [hi, lo] = len_bytes[..] else {
            return Ok(out);
        };
        let Some(payload_len) = segment_payload_len([hi, lo]) else {
            return Ok(out);
        };
        let read = reader
            .by_ref()
            .take(payload_len as u64)
            .read_to_end(&mut out)?;
        if read < payload_len || code == 0xDA {
            return Ok(out);
        }
    }
}

/// XMP payload prefix inside a JPEG APP1 segment.
const XMP_APP1_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// The XMP packet bytes of an image, or `None` when it carries none.
///
/// JPEG: the APP1 segment starting with the standard XMP namespace header.
/// DNG: the IFD0 `XMP` tag (`0x02BC`). Only this slice is scanned for DJI
/// telemetry, so `drone-dji:` text elsewhere in the file (a JPEG comment,
/// EXIF strings, scan data) is never mistaken for it. A damaged structure
/// simply yields `None`: optional telemetry must not fail extraction.
fn xmp_packet(bytes: &[u8], format: ImageFormat) -> Option<&[u8]> {
    match format {
        ImageFormat::Jpeg => jpeg_segments(bytes)
            .map_while(Result::ok)
            .find_map(|(code, seg)| {
                (code == 0xE1)
                    .then(|| seg.strip_prefix(XMP_APP1_HEADER))
                    .flatten()
            }),
        ImageFormat::Dng => {
            let (endian, ifd0_off) = tiff_header(bytes)?;
            let ifd0 = parse_ifd(bytes, endian, ifd0_off).ok()?;
            // BYTE/UNDEFINED, one byte per count; a packet never fits inline.
            let entry = find_entry(&ifd0, 0x02BC)?;
            let (off, len) = (entry.val_u32 as usize, entry.count as usize);
            bytes.get(off..off.checked_add(len)?)
        }
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
/// [`RawXmpTags`] with every tag that was present. All-`None` fields (never
/// an error) for non-DJI images with no XMP DJI packet, and for the tags a
/// malformed packet left unread.
///
/// Both serializations are accepted inside the packet:
///
/// * attribute form — `drone-dji:Name="value"` or `'value'`
/// * element form — `<drone-dji:Name>value</drone-dji:Name>`
///
/// An occurrence that is neither (stray text, `xmlns:drone-dji=...`, a
/// value without quotes, a truncated attribute) is skipped, and a
/// non-numeric value leaves its field unset. A quirk in optional telemetry
/// never discards it, so it must never fail this call.
fn parse_xmp_dji(bytes: &[u8]) -> RawXmpTags {
    // XMP packets are UTF-8 text embedded in the file; view the bytes
    // lossily so arbitrary binary cannot break the scan.
    let text = String::from_utf8_lossy(bytes);
    let Some((start, end)) = xmp_packet_span(&text) else {
        return RawXmpTags::default();
    };
    const PREFIX: &str = "drone-dji:";
    let packet = &text[start..end];
    let raw = packet.as_bytes();
    let mut tags = RawXmpTags::default();
    let mut cursor = 0;
    while let Some(found) = packet[cursor..].find(PREFIX) {
        // Every index below is a char boundary: `PREFIX`, the attribute
        // name, whitespace, `=`, the quotes, and the tag markers are all
        // ASCII.
        let at = cursor + found;
        let name_start = at + PREFIX.len();
        let mut i = name_start;
        while matches!(raw.get(i), Some(b) if b.is_ascii_alphanumeric()) {
            i += 1;
        }
        let name = &packet[name_start..i];
        if name.is_empty() {
            cursor = name_start;
            continue;
        }
        let mut j = i;
        while matches!(raw.get(j), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            j += 1;
        }
        match raw.get(j) {
            // Attribute form: name = "value" | 'value'.
            Some(b'=') => {
                let mut k = j + 1;
                while matches!(raw.get(k), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                    k += 1;
                }
                let quote = match raw.get(k) {
                    Some(b'"') => b'"',
                    Some(b'\'') => b'\'',
                    // Unquoted value: not an attribute, skip this occurrence.
                    _ => {
                        cursor = i;
                        continue;
                    }
                };
                let value_start = k + 1;
                let mut k = value_start;
                let value_end = loop {
                    match raw.get(k) {
                        None => break k,
                        Some(b) if *b == quote => break k,
                        _ => k += 1,
                    }
                };
                assign_dji_tag(&mut tags, name, &packet[value_start..value_end]);
                // A truncated value ends at the packet end; keep going.
                cursor = if value_end < raw.len() {
                    value_end + 1
                } else {
                    value_end
                };
            }
            // Element form: <drone-dji:Name>value</drone-dji:Name>.
            Some(b'>') => {
                let text_start = j + 1;
                let close = format!("</{PREFIX}{name}>");
                let value_end = packet[text_start..]
                    .find(&close)
                    .or_else(|| packet[text_start..].find('<'))
                    .unwrap_or(packet.len() - text_start);
                assign_dji_tag(&mut tags, name, &packet[text_start..text_start + value_end]);
                cursor = text_start + value_end;
            }
            // Stray text, namespace declarations, or an attribute this
            // parser does not understand: skip it.
            _ => {
                cursor = i;
                continue;
            }
        }
    }
    tags
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

/// Required [`ImageMetadata`] fields for the "incomplete metadata" import
/// flag (M1-23).
///
/// Covers the acceptance criteria of Image Metadata Extraction (M1-9): GPS
/// coordinates, acquisition date, altitude, and resolution. XMP DJI flight
/// telemetry is deliberately excluded — a non-DJI drone never carries it,
/// so requiring it would flag every such image.
pub(crate) const REQUIRED_METADATA_FIELDS: &[&str] = &[
    "gps_latitude",
    "gps_longitude",
    "gps_altitude_m",
    "date_time_original",
    "width",
    "height",
];

/// One `(name, present)` row per [`ImageMetadata`] field.
///
/// The single source of truth for field names: [`describe_completeness`] and
/// [`missing_required_fields`] both derive from it, so adding a field to
/// [`ImageMetadata`] means editing this table only.
fn field_presence(metadata: &ImageMetadata) -> [(&'static str, bool); 32] {
    [
        ("gps_latitude", metadata.gps_latitude.is_some()),
        ("gps_longitude", metadata.gps_longitude.is_some()),
        ("gps_altitude_m", metadata.gps_altitude_m.is_some()),
        (
            "absolute_altitude_m",
            metadata.absolute_altitude_m.is_some(),
        ),
        ("date_time_original", metadata.date_time_original.is_some()),
        ("width", metadata.width.is_some()),
        ("height", metadata.height.is_some()),
        ("format", metadata.format.is_some()),
        ("make", metadata.make.is_some()),
        ("camera_model_name", metadata.camera_model_name.is_some()),
        ("exposure_time_s", metadata.exposure_time_s.is_some()),
        ("f_number", metadata.f_number.is_some()),
        ("iso", metadata.iso.is_some()),
        ("focal_length_mm", metadata.focal_length_mm.is_some()),
        ("focal_length_35mm", metadata.focal_length_35mm.is_some()),
        ("flash", metadata.flash.is_some()),
        ("white_balance", metadata.white_balance.is_some()),
        ("metering_mode", metadata.metering_mode.is_some()),
        ("exposure_mode", metadata.exposure_mode.is_some()),
        ("digital_zoom_ratio", metadata.digital_zoom_ratio.is_some()),
        ("color_space", metadata.color_space.is_some()),
        ("orientation", metadata.orientation.is_some()),
        (
            "relative_altitude_m",
            metadata.relative_altitude_m.is_some(),
        ),
        ("gimbal_roll_degree", metadata.gimbal_roll_degree.is_some()),
        ("gimbal_yaw_degree", metadata.gimbal_yaw_degree.is_some()),
        (
            "gimbal_pitch_degree",
            metadata.gimbal_pitch_degree.is_some(),
        ),
        ("flight_roll_degree", metadata.flight_roll_degree.is_some()),
        ("flight_yaw_degree", metadata.flight_yaw_degree.is_some()),
        (
            "flight_pitch_degree",
            metadata.flight_pitch_degree.is_some(),
        ),
        ("flight_x_speed", metadata.flight_x_speed.is_some()),
        ("flight_y_speed", metadata.flight_y_speed.is_some()),
        ("flight_z_speed", metadata.flight_z_speed.is_some()),
    ]
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
    let fields = field_presence(metadata);
    let mut present = Vec::with_capacity(fields.len());
    let mut missing = Vec::with_capacity(fields.len());
    for (name, is_present) in fields {
        if is_present {
            present.push(name.to_string());
        } else {
            missing.push(name.to_string());
        }
    }
    MetadataCompleteness { present, missing }
}

/// Lists the [`REQUIRED_METADATA_FIELDS`] absent from already-extracted
/// metadata.
///
/// Pure, total. Empty means the image counts as fully described for import
/// purposes; non-empty feeds [`IncompleteMetadataFlag`](crate::models::image::IncompleteMetadataFlag)
/// via `image_importer::flag_incomplete_metadata`.
pub(crate) fn missing_required_fields(metadata: &ImageMetadata) -> Vec<String> {
    field_presence(metadata)
        .into_iter()
        .filter(|(name, is_present)| REQUIRED_METADATA_FIELDS.contains(name) && !is_present)
        .map(|(name, _)| name.to_string())
        .collect()
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
/// Missing values use the only total mapping the target shape allows:
/// absent width/height become `0`, absent format becomes `""`. That empty
/// format never matches `allowed_formats`, so a missing format fails
/// pre-flight fail-closed when a format list is required.
///
/// # Module Contract
///
/// This function represents the cross-module contract:
/// **Module 1.3 → Module 2 (pre-flight check)**.
pub(crate) fn to_plugin_metadata(
    metadata: &ImageMetadata,
    path: &Path,
) -> crate::plugin_manager::payload::ImageMetadata {
    crate::plugin_manager::payload::ImageMetadata {
        path: path.display().to_string(),
        has_gps: metadata.gps_latitude.is_some() && metadata.gps_longitude.is_some(),
        width: metadata.width.unwrap_or(0),
        height: metadata.height.unwrap_or(0),
        format: metadata
            .format
            .map_or_else(String::new, |f| f.as_str().to_string()),
    }
}

/// A user-supplied alias table, folded once so each lookup is one hash probe.
///
/// Different image/drone providers label the same logical field with
/// different strings (e.g. `"Tinggi_meter"` vs. this module's canonical
/// `"height_meter"`). The platform does not guess at such matches; the user
/// supplies the alias table explicitly, so a wrong or missing mapping is the
/// user's responsibility, not an inference the platform performed for them.
///
/// Keys are folded with [`normalize_alias_key`], so case and accent composition do
/// not matter. If two user keys fold to the same string, the one that sorts
/// first wins, so the result does not depend on `HashMap` iteration order.
#[derive(Debug, Clone, Default)]
pub(crate) struct AliasTable(HashMap<String, String>);

impl AliasTable {
    /// Folds every key of `aliases` once.
    pub(crate) fn new(aliases: &HashMap<String, String>) -> Self {
        let mut sorted: Vec<(&String, &String)> = aliases.iter().collect();
        sorted.sort();
        let mut folded = HashMap::with_capacity(sorted.len());
        for (key, canonical) in sorted {
            folded
                .entry(normalize_alias_key(key))
                .or_insert_with(|| canonical.clone());
        }
        Self(folded)
    }

    /// Resolves a provider-specific field name to this module's canonical
    /// field name.
    ///
    /// # Purity
    ///
    /// Pure, total (never fails).
    ///
    /// # Returns
    ///
    /// `Some(canonical_name)` if `raw_key` matches a key after folding,
    /// otherwise `None`.
    pub(crate) fn resolve(&self, raw_key: &str) -> Option<&str> {
        self.0
            .get(&normalize_alias_key(raw_key))
            .map(String::as_str)
    }
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

    /// DNG whose IFD0 is a 256x171 reduced-resolution preview
    /// (`NewSubfileType = 1`) and whose SubIFD holds the full image.
    fn tiff_le_with_preview_ifd0(full: (u32, u32), preview: (u32, u32)) -> Vec<u8> {
        fn entry(tag: u16, typ: u16, value: u32) -> Vec<u8> {
            let mut e = tag.to_le_bytes().to_vec();
            e.extend_from_slice(&typ.to_le_bytes());
            e.extend_from_slice(&1u32.to_le_bytes());
            e.extend_from_slice(&value.to_le_bytes());
            e
        }
        // IFD0 at 8: count + 4 entries + next = 54 bytes, so SubIFD at 62.
        let mut v = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        v.extend_from_slice(&4u16.to_le_bytes());
        v.extend(entry(254, 4, 1));
        v.extend(entry(256, 4, preview.0));
        v.extend(entry(257, 4, preview.1));
        v.extend(entry(0x14A, 4, 62));
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&3u16.to_le_bytes());
        v.extend(entry(254, 4, 0));
        v.extend(entry(256, 4, full.0));
        v.extend(entry(257, 4, full.1));
        v.extend_from_slice(&0u32.to_le_bytes());
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
    fn tiff_dimensions_prefers_full_resolution_subifd_over_preview_ifd0() {
        let dng = tiff_le_with_preview_ifd0((5280, 3956), (256, 171));
        let dims = tiff_dimensions(&dng).expect("dimensions");
        assert_eq!((dims.width, dims.height), (5280, 3956));
    }

    #[test]
    fn tiff_dimensions_keeps_ifd0_when_no_subifd() {
        let dims = tiff_dimensions(&tiff_le_with_dimensions(4000, 3000)).expect("dimensions");
        assert_eq!((dims.width, dims.height), (4000, 3000));
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
    fn stale_gps_pointer_keeps_the_rest_of_the_exif() {
        let mut tiff = tiff_le_with_make("TestCam");
        // IFD0 holds one entry (Make) at offset 8; add a GPS pointer entry
        // far outside the file by rewriting the count and appending it.
        let count_off = 8;
        tiff[count_off] = 2;
        let entry_end = count_off + 2 + 12;
        let mut gps = Vec::new();
        gps.extend_from_slice(&0x8825u16.to_le_bytes());
        gps.extend_from_slice(&4u16.to_le_bytes());
        gps.extend_from_slice(&1u32.to_le_bytes());
        gps.extend_from_slice(&0x00FF_FFFFu32.to_le_bytes());
        tiff.splice(entry_end..entry_end, gps);
        // The inserted entry pushes the out-of-line Make string back 12 bytes.
        tiff[18..22].copy_from_slice(&38u32.to_le_bytes());
        let tags = parse_tiff_tags(&tiff).expect("a stale GPS pointer is not fatal");
        assert_eq!(tags.make.as_deref(), Some("TestCam"));
        assert_eq!(tags.gps_latitude, None);
    }

    #[test]
    fn progressive_jpeg_reports_dimensions() {
        let mut sof = vec![0x08];
        sof.extend_from_slice(&300u16.to_be_bytes());
        sof.extend_from_slice(&400u16.to_be_bytes());
        sof.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        let file = jpeg_with_segments(&[(0xC2, sof)]);
        let dims = jpeg_dimensions(&file).expect("SOF2 dimensions");
        assert_eq!((dims.width, dims.height), (400, 300));
        assert!(!is_sof_marker(0xC4) && !is_sof_marker(0xC8) && !is_sof_marker(0xCC));
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

    fn required_metadata() -> ImageMetadata {
        ImageMetadata {
            gps_latitude: Some(-7.5),
            gps_longitude: Some(110.0),
            gps_altitude_m: Some(120.0),
            date_time_original: "2026-09-18T07:12:00".parse().ok(),
            width: Some(4000),
            height: Some(3000),
            ..ImageMetadata::default()
        }
    }

    #[test]
    fn completeness_empty_is_all_missing() {
        let report = describe_completeness(&ImageMetadata::default());
        assert!(report.present.is_empty());
        assert_eq!(report.missing.len(), 32);
        assert!(report.missing.contains(&"gps_latitude".to_string()));
        // Every required field shows up as missing.
        for name in REQUIRED_METADATA_FIELDS {
            assert!(report.missing.contains(&(*name).to_string()));
        }
    }

    #[test]
    fn completeness_buckets_partial() {
        let meta = ImageMetadata {
            gps_latitude: Some(-7.5),
            width: Some(4000),
            ..ImageMetadata::default()
        };
        let report = describe_completeness(&meta);
        assert_eq!(
            report.present,
            vec!["gps_latitude".to_string(), "width".to_string()]
        );
        assert_eq!(report.present.len() + report.missing.len(), 32);
        assert!(report.missing.contains(&"date_time_original".to_string()));
    }

    #[test]
    fn missing_required_empty_when_required_present() {
        // Extra optional fields absent: still counts as complete for import.
        assert!(missing_required_fields(&required_metadata()).is_empty());
    }

    #[test]
    fn every_required_field_is_in_the_presence_table() {
        let table = field_presence(&ImageMetadata::default());
        for name in REQUIRED_METADATA_FIELDS {
            assert!(
                table.iter().any(|(n, _)| n == name),
                "{name} missing from field_presence"
            );
        }
    }

    #[test]
    fn missing_required_lists_only_required_absent() {
        let mut meta = required_metadata();
        meta.date_time_original = None;
        meta.gps_altitude_m = None;
        // A present-but-optional field must not appear in the report.
        meta.make = Some("DJI".to_string());
        assert_eq!(
            missing_required_fields(&meta),
            vec![
                "gps_altitude_m".to_string(),
                "date_time_original".to_string()
            ]
        );
    }

    // Restored (deleted without justification in 5f09e0c): serde and
    // default coverage for the module's plain data types.
    #[test]
    fn gps_coordinate_round_trips_through_serde() {
        let coord = GpsCoordinate {
            degrees: 1.0,
            minutes: 16.0,
            seconds: 12.5,
            reference: 'S',
        };
        let json = serde_json::to_string(&coord).expect("serialize");
        let decoded: GpsCoordinate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(coord, decoded);
    }

    #[test]
    fn pixel_dimensions_round_trips_through_serde() {
        let dims = PixelDimensions {
            width: 4000,
            height: 3000,
        };
        let json = serde_json::to_string(&dims).expect("serialize");
        let decoded: PixelDimensions = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dims, decoded);
    }

    #[test]
    fn metadata_completeness_defaults_to_empty() {
        let report = MetadataCompleteness::default();
        assert!(report.present.is_empty());
        assert!(report.missing.is_empty());
    }

    #[test]
    fn raw_exif_tags_default_has_all_none() {
        let tags = RawExifTags::default();
        assert_eq!(tags, RawExifTags::default());
        assert!(tags.gps_latitude.is_none());
        assert!(tags.camera_model_name.is_none());
    }

    #[test]
    fn raw_xmp_tags_default_has_all_none() {
        let tags = RawXmpTags::default();
        assert!(tags.absolute_altitude_m.is_none());
        assert!(tags.gimbal_yaw_degree.is_none());
    }

    #[test]
    fn detect_format_accepts_jpeg() {
        let header = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(
            detect_format(&header, "jpg").expect("jpg"),
            ImageFormat::Jpeg
        );
        assert_eq!(
            detect_format(&header, "jpeg").expect("jpeg"),
            ImageFormat::Jpeg
        );
        // Extension matching is case-insensitive with an optional dot.
        assert_eq!(
            detect_format(&header, ".JPG").expect("dot JPG"),
            ImageFormat::Jpeg
        );
    }

    #[test]
    fn detect_format_accepts_tiff_magic_both_endians() {
        let little = [0x49, 0x49, 0x2A, 0x00, 0x08, 0x00];
        let big = [0x4D, 0x4D, 0x00, 0x2A, 0x00, 0x08];
        assert_eq!(
            detect_format(&little, "dng").expect("little-endian"),
            ImageFormat::Dng
        );
        assert_eq!(
            detect_format(&big, "DNG").expect("big-endian"),
            ImageFormat::Dng
        );
    }

    #[test]
    fn detect_format_rejects_mismatch() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0];
        let tiff = [0x49, 0x49, 0x2A, 0x00];
        assert!(matches!(
            detect_format(&tiff, "jpg"),
            Err(MetadataError::MagicMismatch { .. })
        ));
        assert!(matches!(
            detect_format(&jpeg, "dng"),
            Err(MetadataError::MagicMismatch { .. })
        ));
    }

    #[test]
    fn detect_format_rejects_unsupported_extension() {
        let header = [0xFF, 0xD8, 0xFF, 0xE0];
        for extension in ["png", "tif", "mp4", ""] {
            match detect_format(&header, extension) {
                Err(MetadataError::UnsupportedFormat { extension: got, .. }) => {
                    assert_eq!(got, extension)
                }
                other => panic!("{extension:?} should be unsupported, got {other:?}"),
            }
        }
    }

    const DJI_XMP: &str = concat!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">",
        "<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">",
        "<rdf:Description rdf:about=\"\" ",
        "xmlns:drone-dji=\"http://dji.com/drone-dji/1.0/\" ",
        "drone-dji:AbsoluteAltitude=\"+120.5\" ",
        "drone-dji:RelativeAltitude=\"+45.25\" ",
        "drone-dji:GimbalRollDegree=\"+0.00\" ",
        "drone-dji:GimbalYawDegree=\"+12.50\" ",
        "drone-dji:GimbalPitchDegree=\"-30.00\" ",
        "drone-dji:FlightRollDegree=\"+1.00\" ",
        "drone-dji:FlightYawDegree=\"+12.00\" ",
        "drone-dji:FlightPitchDegree=\"-2.00\" ",
        "drone-dji:FlightXSpeed=\"+0.50\" ",
        "drone-dji:FlightYSpeed=\"-1.25\" ",
        "drone-dji:FlightZSpeed=\"+0.00\"/>",
        "</rdf:RDF></x:xmpmeta>",
    );

    #[test]
    fn parse_xmp_dji_absent_packet_is_default() {
        let tags = parse_xmp_dji(b"\x00\xff not an xmp packet");
        assert_eq!(tags, RawXmpTags::default());
        let adobe = b"<x:xmpmeta><rdf:RDF><rdf:Description/></rdf:RDF></x:xmpmeta>";
        let tags = parse_xmp_dji(adobe);
        assert_eq!(tags, RawXmpTags::default());
    }

    #[test]
    fn parse_xmp_dji_reads_dji_telemetry() {
        let tags = parse_xmp_dji(DJI_XMP.as_bytes());
        assert_eq!(tags.absolute_altitude_m, Some(120.5));
        assert_eq!(tags.relative_altitude_m, Some(45.25));
        assert_eq!(tags.gimbal_roll_degree, Some(0.0));
        assert_eq!(tags.gimbal_yaw_degree, Some(12.5));
        assert_eq!(tags.gimbal_pitch_degree, Some(-30.0));
        assert_eq!(tags.flight_roll_degree, Some(1.0));
        assert_eq!(tags.flight_yaw_degree, Some(12.0));
        assert_eq!(tags.flight_pitch_degree, Some(-2.0));
        assert_eq!(tags.flight_x_speed, Some(0.5));
        assert_eq!(tags.flight_y_speed, Some(-1.25));
        assert_eq!(tags.flight_z_speed, Some(0.0));
    }

    #[test]
    fn parse_xmp_dji_reads_element_form() {
        let packet = "<x:xmpmeta><rdf:RDF><rdf:Description \
            xmlns:drone-dji=\"http://dji.com/drone-dji/1.0/\">\
            <drone-dji:GimbalYawDegree>+16.20</drone-dji:GimbalYawDegree>\
            <drone-dji:AbsoluteAltitude>80.50</drone-dji:AbsoluteAltitude>\
            </rdf:Description></rdf:RDF></x:xmpmeta>";
        let tags = parse_xmp_dji(packet.as_bytes());
        assert_eq!(tags.gimbal_yaw_degree, Some(16.2));
        assert_eq!(tags.absolute_altitude_m, Some(80.5));
    }

    #[test]
    fn parse_xmp_dji_ignores_unknown_attributes() {
        let packet = "<x:xmpmeta><rdf:Description \
            drone-dji:FutureTag=\"99\" \
            drone-dji:GimbalYawDegree=\"+12.50\"/></x:xmpmeta>";
        let tags = parse_xmp_dji(packet.as_bytes());
        assert_eq!(tags.gimbal_yaw_degree, Some(12.5));
    }

    #[test]
    fn parse_xmp_dji_skips_stray_text_outside_the_packet() {
        // A JPEG comment segment carrying unrelated drone-dji text must not
        // be read as telemetry, nor fail the real packet that follows.
        let file = b"comment: stray drone-dji: text, not an attribute\
            <x:xmpmeta>\
            <rdf:Description drone-dji:GimbalYawDegree=\"+12.50\"/>\
            </x:xmpmeta>";
        let tags = parse_xmp_dji(file);
        assert_eq!(tags.gimbal_yaw_degree, Some(12.5));
    }

    #[test]
    fn parse_xmp_dji_unparseable_value_leaves_that_field_unset() {
        let packet = DJI_XMP.replace("+12.50", "n/a  ");
        let tags = parse_xmp_dji(packet.as_bytes());
        assert_eq!(tags.gimbal_yaw_degree, None);
        // Siblings still parse.
        assert_eq!(tags.absolute_altitude_m, Some(120.5));
        assert_eq!(tags.gimbal_roll_degree, Some(0.0));
    }

    #[test]
    fn parse_xmp_dji_rejects_non_finite_numbers() {
        let packet = br#"<x:xmpmeta drone-dji:GimbalYawDegree="nan" drone-dji:GimbalPitchDegree="inf" drone-dji:FlightXSpeed="-infinity" drone-dji:FlightYSpeed="+1.5"/>"#;
        let tags = parse_xmp_dji(packet);
        assert_eq!(tags.gimbal_yaw_degree, None);
        assert_eq!(tags.gimbal_pitch_degree, None);
        assert_eq!(tags.flight_x_speed, None);
        assert_eq!(tags.flight_y_speed, Some(1.5));
    }

    #[test]
    fn parse_xmp_dji_truncated_attribute_does_not_error() {
        // Nothing follows the opening quote: the attribute is skipped, and
        // nothing is read, but no error is raised.
        let truncated = "<x:xmpmeta><rdf:Description \
            drone-dji:AbsoluteAltitude=\"+120.5</x:xmpmeta>";
        let tags = parse_xmp_dji(truncated.as_bytes());
        assert_eq!(tags.absolute_altitude_m, None);
    }

    #[test]
    fn parse_xmp_dji_truncated_element_keeps_earlier_tags() {
        let packet = "<x:xmpmeta><rdf:Description>\
            <drone-dji:GimbalYawDegree>+16.20</drone-dji:GimbalYawDegree>\
            <drone-dji:AbsoluteAltitude>80.5";
        let tags = parse_xmp_dji(packet.as_bytes());
        assert_eq!(tags.gimbal_yaw_degree, Some(16.2));
        assert_eq!(tags.absolute_altitude_m, Some(80.5));
    }

    #[test]
    fn to_plugin_metadata_projects_full_metadata() {
        let metadata = ImageMetadata {
            gps_latitude: Some(-7.5),
            gps_longitude: Some(110.0),
            width: Some(4000),
            height: Some(3000),
            format: Some(ImageFormat::Dng),
            ..ImageMetadata::default()
        };
        let projected = to_plugin_metadata(&metadata, Path::new("/photos/dji_0001.dng"));
        assert_eq!(projected.path, "/photos/dji_0001.dng");
        assert!(projected.has_gps);
        assert_eq!(projected.width, 4000);
        assert_eq!(projected.height, 3000);
        assert_eq!(projected.format, "dng");
    }

    #[test]
    fn to_plugin_metadata_missing_fields_fail_closed() {
        let projected = to_plugin_metadata(&ImageMetadata::default(), Path::new("a.jpg"));
        assert!(!projected.has_gps);
        assert_eq!(projected.width, 0);
        assert_eq!(projected.height, 0);
        assert_eq!(projected.format, "");
        let half = ImageMetadata {
            gps_latitude: Some(-7.5),
            ..ImageMetadata::default()
        };
        assert!(!to_plugin_metadata(&half, Path::new("a.jpg")).has_gps);
    }

    #[test]
    fn alias_table_matches_case_insensitively() {
        let aliases: HashMap<String, String> =
            [("Tinggi_meter".to_string(), "height_meter".to_string())]
                .into_iter()
                .collect();
        let table = AliasTable::new(&aliases);
        assert_eq!(table.resolve("tinggi_METER"), Some("height_meter"));
        assert_eq!(table.resolve("unknown"), None);
        assert_eq!(
            AliasTable::new(&HashMap::new()).resolve("Tinggi_meter"),
            None
        );
    }

    #[test]
    fn alias_table_folds_unicode_keys() {
        let aliases: HashMap<String, String> =
            [("Élévation".to_string(), "height_meter".to_string())]
                .into_iter()
                .collect();
        let table = AliasTable::new(&aliases);
        assert_eq!(table.resolve("élévation"), Some("height_meter"));
        assert_eq!(table.resolve("e\u{0301}lévation"), Some("height_meter"));
    }

    #[test]
    fn normalize_alias_key_folds_case_and_accent_composition() {
        assert_eq!(
            normalize_alias_key("Élévation"),
            normalize_alias_key("e\u{0301}lévation")
        );
        assert_eq!(normalize_alias_key("HEIGHT"), normalize_alias_key("height"));
        // Not a filesystem rule: ß is left alone.
        assert_ne!(
            normalize_alias_key("Stra\u{00DF}e"),
            normalize_alias_key("STRASSE")
        );
    }

    #[test]
    fn alias_table_folded_collision_is_deterministic() {
        let aliases: HashMap<String, String> = [
            ("Height".to_string(), "from_upper".to_string()),
            ("height".to_string(), "from_lower".to_string()),
        ]
        .into_iter()
        .collect();
        // "Height" sorts before "height", so it wins on every run.
        assert_eq!(
            AliasTable::new(&aliases).resolve("HEIGHT"),
            Some("from_upper")
        );
    }

    fn jpeg_sof_no_exif() -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        v.extend_from_slice(b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
        v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        v.extend_from_slice(&300u16.to_be_bytes());
        v.extend_from_slice(&400u16.to_be_bytes());
        v.extend_from_slice(&[0x01, 0x01, 0x11, 0x00, 0xFF, 0xD9]);
        v
    }

    #[test]
    fn extract_metadata_jpeg_without_exif_reads_dimensions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, jpeg_sof_no_exif()).expect("write fixture");
        let metadata = extract_metadata(&path).expect("extract");
        assert_eq!(metadata.width, Some(400));
        assert_eq!(metadata.height, Some(300));
        assert_eq!(metadata.format, Some(ImageFormat::Jpeg));
        assert_eq!(metadata.gps_latitude, None);
    }

    #[test]
    fn extract_metadata_reports_real_path_and_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, jpeg_sof_no_exif()).expect("write fixture");
        let err = extract_metadata(&path).unwrap_err();
        match err {
            MetadataError::UnsupportedFormat {
                path: got,
                extension,
            } => {
                assert_eq!(got, path.display().to_string());
                assert_eq!(extension, "txt");
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn extract_metadata_dng_reads_tiff_dimensions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.dng");
        std::fs::write(&path, tiff_le_with_dimensions(400, 300)).expect("write fixture");
        let metadata = extract_metadata(&path).expect("extract");
        assert_eq!(metadata.width, Some(400));
        assert_eq!(metadata.height, Some(300));
        assert_eq!(metadata.format, Some(ImageFormat::Dng));
    }

    fn jpeg_with_segments(segments: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8];
        for (code, payload) in segments {
            v.extend_from_slice(&[0xFF, *code]);
            v.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            v.extend_from_slice(payload);
        }
        v
    }

    fn sof_payload() -> Vec<u8> {
        let mut p = vec![0x08];
        p.extend_from_slice(&300u16.to_be_bytes());
        p.extend_from_slice(&400u16.to_be_bytes());
        p.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        p
    }

    #[test]
    fn read_jpeg_head_stops_after_the_sos_header() {
        let mut file = jpeg_with_segments(&[(0xC0, sof_payload()), (0xDA, vec![0x00, 0x00])]);
        let head_len = file.len();
        file.extend(std::iter::repeat_n(0xAB, 10_000));
        let head = read_jpeg_head(&mut file.as_slice()).expect("read");
        assert_eq!(head.len(), head_len);
        assert_eq!(head, file[..head_len]);
    }

    #[test]
    fn read_jpeg_head_and_jpeg_segments_agree_on_where_metadata_ends() {
        // Padding bytes and standalone markers between segments, then scan
        // data: the head reader must stop exactly where the parser does.
        let mut file = vec![0xFF, 0xD8, 0xFF, 0xFF, 0xD0];
        file.extend_from_slice(&[0xFF, 0xE1, 0x00, 0x04, 0xAA, 0xBB]);
        file.extend_from_slice(&[0xFF, 0x01]);
        file.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B]);
        file.extend_from_slice(&sof_payload());
        file.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        let head_len = file.len();
        file.extend(std::iter::repeat_n(0x5A, 500));

        let head = read_jpeg_head(&mut file.as_slice()).expect("read");
        assert_eq!(head.len(), head_len);
        let from_head: Vec<u8> = jpeg_segments(&head)
            .map(|seg| seg.expect("seg").0)
            .collect();
        let from_file: Vec<u8> = jpeg_segments(&file)
            .map(|seg| seg.expect("seg").0)
            .collect();
        assert_eq!(from_head, vec![0xE1, 0xC0, 0xDA]);
        assert_eq!(from_head, from_file);
    }

    #[test]
    fn read_jpeg_head_keeps_a_truncated_segment_for_the_parser_to_report() {
        let mut file = jpeg_with_segments(&[(0xC0, sof_payload())]);
        file.truncate(file.len() - 3);
        let head = read_jpeg_head(&mut file.as_slice()).expect("read");
        assert_eq!(head, file);
        assert!(jpeg_dimensions(&head).is_err());
    }

    #[test]
    fn extract_metadata_reads_dji_xmp_from_the_xmp_segment() {
        let mut xmp = XMP_APP1_HEADER.to_vec();
        xmp.extend_from_slice(
            b"<x:xmpmeta><rdf:Description drone-dji:GimbalYawDegree=\"-90.5\"/></x:xmpmeta>",
        );
        let file = jpeg_with_segments(&[(0xE1, xmp), (0xC0, sof_payload())]);
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, file).expect("write fixture");
        let metadata = extract_metadata(&path).expect("extract");
        assert_eq!(metadata.gimbal_yaw_degree, Some(-90.5));
        assert_eq!(metadata.width, Some(400));
    }

    #[test]
    fn xmp_packet_reads_the_dng_xmp_tag() {
        let packet = b"<x:xmpmeta drone-dji:FlightYawDegree=\"7.5\"/>";
        let mut dng = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        dng.extend_from_slice(&1u16.to_le_bytes());
        dng.extend_from_slice(&0x02BCu16.to_le_bytes());
        dng.extend_from_slice(&1u16.to_le_bytes()); // BYTE
        dng.extend_from_slice(&(packet.len() as u32).to_le_bytes());
        dng.extend_from_slice(&26u32.to_le_bytes()); // after IFD0 + next pointer
        dng.extend_from_slice(&0u32.to_le_bytes());
        dng.extend_from_slice(packet);
        let found = xmp_packet(&dng, ImageFormat::Dng).expect("packet");
        assert_eq!(found, packet);
        assert_eq!(parse_xmp_dji(found).flight_yaw_degree, Some(7.5));
    }

    #[test]
    fn extract_metadata_ignores_dji_text_outside_the_xmp_segment() {
        let comment = b"<x:xmpmeta>drone-dji:GimbalYawDegree=\"12\"</x:xmpmeta>".to_vec();
        let file = jpeg_with_segments(&[(0xFE, comment), (0xC0, sof_payload())]);
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, file).expect("write fixture");
        let metadata = extract_metadata(&path).expect("extract");
        assert_eq!(metadata.gimbal_yaw_degree, None);
    }

    #[test]
    fn extract_metadata_missing_file_is_io_error() {
        let err = extract_metadata(Path::new("/nonexistent-dir-xyz/photo.jpg")).unwrap_err();
        assert!(matches!(err, MetadataError::Io(_)));
    }
}
