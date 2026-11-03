#![allow(dead_code)]

//! DNG Preview Image Reader (Module 1.3).
//!
//! Reads the JPEG a DNG already stores, so a browser can display a DNG
//! without any raw-decoding machinery. A browser cannot put a `.dng` in an
//! `<img>` tag; the JPEG embedded in the file is the same preview the file
//! manager shows.
//!
//! This module does **not** demosaic Bayer pixels and does not decode a
//! full-resolution image. It only locates a stored JPEG and hands back its
//! bytes. A DNG that carries only sensor data has no preview, and that is
//! reported as [`MetadataError::PreviewUnavailable`] rather than as a
//! failure to read the file.
//!
//! ## Purity Boundary
//!
//! [`read_dng_preview`] is the only impure function here. Everything else
//! is a pure walk over the TIFF/IFD structures of an in-memory file, so the
//! candidate search can be exercised with fixtures and no file on disk.
//!
//! ## Module Contracts
//!
//! | Function | Direction | Purpose |
//! |----------|-----------|---------|
//! | [`read_dng_preview`] | Module 1 → other modules | The stored JPEG preview of one DNG |

use std::path::Path;

use crate::modules::module_01::error::MetadataError;

use super::metadata_extractor::{
    blob_at, find_entry, parse_ifd, read_u32_val, require_dng_file, tiff_header, u32_at, IfdEntry,
    TiffEndian,
};

/// DNG `PreviewImageStart` — offset of the stored preview image.
const PREVIEW_IMAGE_START: u16 = 50981;

/// DNG `PreviewImageLength` — length of the stored preview image.
const PREVIEW_IMAGE_LENGTH: u16 = 50982;

/// TIFF `JPEGInterchangeFormat` — offset of an embedded JPEG.
const JPEG_INTERCHANGE_FORMAT: u16 = 513;

/// TIFF `JPEGInterchangeFormatLength` — length of an embedded JPEG.
const JPEG_INTERCHANGE_FORMAT_LENGTH: u16 = 514;

/// TIFF `SubIFDs` — IFDs holding reduced-resolution images.
const SUB_IFD_TAG: u16 = 0x014A;

/// The largest embedded JPEG this reader will return, in bytes.
const MAX_PREVIEW_BYTES: usize = 8 * 1024 * 1024;

/// The largest DNG this reader will load into memory, in bytes.
///
/// A DNG is read whole because its IFD offsets can point anywhere in the
/// file. The cap keeps a huge file from turning into a huge allocation.
const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024;

/// The largest SubIFD offset table this reader will follow, in bytes.
const MAX_SUB_IFD_TABLE_BYTES: usize = 64 * 1024;

/// Reads the JPEG preview stored inside a DNG.
///
/// # Purity
///
/// Impure — opens and reads the file.
///
/// # Arguments
///
/// * `path` — filesystem path to a DNG aerial image.
///
/// # Returns
///
/// `Ok(Vec<u8>)` holding the bytes of the embedded JPEG. The bytes always
/// begin with the JPEG start-of-image marker `FF D8`.
///
/// The largest usable preview wins. Candidates are searched in this order:
/// the DNG `PreviewImageStart`/`PreviewImageLength` pair on IFD0, then
/// `JPEGInterchangeFormat` on IFD1, then the same pair on IFD0, then the
/// SubIFDs. Every candidate that is not a standalone JPEG — a raw strip or a
/// tiled payload — is ignored.
///
/// # Errors
///
/// [`MetadataError::UnsupportedFormat`] if the file is not a DNG,
/// [`MetadataError::MagicMismatch`] if a `.dng` does not start with TIFF
/// magic, [`MetadataError::Io`] if the file cannot be read or is larger
/// than 80 MiB, and [`MetadataError::PreviewUnavailable`] if the DNG holds
/// no embedded JPEG.
///
/// # Module Contract
///
/// **Module 1.3 → other modules (DNG preview)**.
pub(crate) fn read_dng_preview(path: &Path) -> Result<Vec<u8>, MetadataError> {
    require_dng_file(path)?;
    let bytes = read_file_capped(path)?;
    let candidates = collect_candidates(&bytes);
    candidates
        .into_iter()
        .max_by_key(|candidate| candidate.len())
        .map(|candidate| candidate.to_vec())
        .ok_or_else(|| MetadataError::PreviewUnavailable {
            path: path.display().to_string(),
        })
}

/// Reads the whole file, refusing anything over [`MAX_FILE_BYTES`].
fn read_file_capped(path: &Path) -> Result<Vec<u8>, MetadataError> {
    let length = std::fs::metadata(path)?.len();
    if length > MAX_FILE_BYTES {
        return Err(MetadataError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "file is {} MiB, over the {} MiB preview limit",
                length / (1024 * 1024),
                MAX_FILE_BYTES / (1024 * 1024)
            ),
        )));
    }
    Ok(std::fs::read(path)?)
}

/// Collects every embedded-JPEG candidate in the container, in search order.
fn collect_candidates(tiff: &[u8]) -> Vec<&[u8]> {
    let mut out: Vec<&[u8]> = Vec::new();
    let Some((endian, ifd0_offset)) = tiff_header(tiff) else {
        return out;
    };
    let Ok(ifd0) = parse_ifd(tiff, endian, ifd0_offset) else {
        return out;
    };

    // 1. The DNG's own preview tags on IFD0.
    push_preview_tags(tiff, &ifd0, endian, &mut out);

    // 2. The thumbnail IFD.
    if let Some(ifd1) = next_ifd(tiff, endian, ifd0_offset, ifd0.len()) {
        if let Ok(entries) = parse_ifd(tiff, endian, ifd1) {
            push_interchange(tiff, &entries, endian, &mut out);
        }
    }

    // 3. The same tags on IFD0.
    push_interchange(tiff, &ifd0, endian, &mut out);

    // 4. The reduced-resolution SubIFDs.
    for sub in sub_ifd_offsets(tiff, &ifd0, endian) {
        let Ok(entries) = parse_ifd(tiff, endian, sub) else {
            continue;
        };
        push_interchange(tiff, &entries, endian, &mut out);
    }

    out
}

/// Pushes the blob named by a `PreviewImageStart`/`PreviewImageLength` pair.
fn push_preview_tags<'a>(
    tiff: &'a [u8],
    entries: &[IfdEntry],
    endian: TiffEndian,
    out: &mut Vec<&'a [u8]>,
) {
    if let Some(blob) = tagged_blob(
        tiff,
        entries,
        endian,
        PREVIEW_IMAGE_START,
        PREVIEW_IMAGE_LENGTH,
    ) {
        out.push(blob);
    }
}

/// Pushes the blob named by a `JPEGInterchangeFormat`/`Length` pair.
fn push_interchange<'a>(
    tiff: &'a [u8],
    entries: &[IfdEntry],
    endian: TiffEndian,
    out: &mut Vec<&'a [u8]>,
) {
    if let Some(blob) = tagged_blob(
        tiff,
        entries,
        endian,
        JPEG_INTERCHANGE_FORMAT,
        JPEG_INTERCHANGE_FORMAT_LENGTH,
    ) {
        out.push(blob);
    }
}

/// Reads the blob an offset/length tag pair points at.
///
/// Only a standalone JPEG is accepted. A strip or a tiled payload starts
/// with sensor data, not with the JPEG marker, and is dropped here.
fn tagged_blob<'a>(
    tiff: &'a [u8],
    entries: &[IfdEntry],
    endian: TiffEndian,
    offset_tag: u16,
    length_tag: u16,
) -> Option<&'a [u8]> {
    let offset = read_u32_val(tiff, entries, endian, offset_tag)? as usize;
    let length = read_u32_val(tiff, entries, endian, length_tag)? as usize;
    let blob = blob_at(tiff, offset, length, MAX_PREVIEW_BYTES)?;
    is_jpeg(blob).then_some(blob)
}

/// Whether `bytes` start with the JPEG start-of-image marker.
fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8])
}

/// The offset of the IFD that follows the one at `ifd_offset`.
fn next_ifd(
    tiff: &[u8],
    endian: TiffEndian,
    ifd_offset: usize,
    entry_count: usize,
) -> Option<usize> {
    let position = ifd_offset + 2 + entry_count * 12;
    let offset = u32_at(tiff, position, endian)? as usize;
    (offset != 0).then_some(offset)
}

/// The offsets of the SubIFDs listed by the `SubIFDs` tag of `entries`.
fn sub_ifd_offsets(tiff: &[u8], entries: &[IfdEntry], endian: TiffEndian) -> Vec<usize> {
    let Some(entry) = find_entry(entries, SUB_IFD_TAG) else {
        return Vec::new();
    };
    if entry.typ != 4 || entry.count == 0 {
        return Vec::new();
    }
    let count = entry.count as usize;
    let total = count * 4;
    let data: &[u8] = if total <= 4 {
        &entry.val_bytes[..total]
    } else {
        match blob_at(tiff, entry.val_u32 as usize, total, MAX_SUB_IFD_TABLE_BYTES) {
            Some(data) => data,
            None => return Vec::new(),
        }
    };
    (0..count)
        .filter_map(|index| u32_at(data, index * 4, endian))
        .map(|offset| offset as usize)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An IFD entry value in a fixture: a literal number, or a reference to
    /// one of the fixture's blobs, patched with its offset when built.
    enum FixtureValue {
        Number(u32),
        Blob(usize),
    }

    /// One IFD in a fixture.
    struct FixtureIfd {
        entries: Vec<(u16, u16, u32, FixtureValue)>,
        next: Option<usize>,
    }

    /// Builds little-endian TIFF fixtures for the preview reader.
    ///
    /// The layout is header, then every IFD in the order they were added,
    /// then every blob. IFD 0 is the one the TIFF header points at, so a
    /// fixture that needs a thumbnail adds its IFD 0 first and links to the
    /// later IFD with [`TiffFixture::link`].
    #[derive(Default)]
    struct TiffFixture {
        ifds: Vec<FixtureIfd>,
        blobs: Vec<Vec<u8>>,
    }

    impl TiffFixture {
        fn new() -> Self {
            Self::default()
        }

        /// Registers a blob and returns its index.
        fn blob(&mut self, bytes: Vec<u8>) -> usize {
            self.blobs.push(bytes);
            self.blobs.len() - 1
        }

        /// Registers an IFD and returns its index.
        fn ifd(&mut self, entries: Vec<(u16, u16, u32, FixtureValue)>) -> usize {
            self.ifds.push(FixtureIfd {
                entries,
                next: None,
            });
            self.ifds.len() - 1
        }

        /// Makes the IFD `from` point at the IFD `to`.
        fn link(&mut self, from: usize, to: usize) {
            self.ifds[from].next = Some(to);
        }

        /// The byte offset of every IFD in the current layout.
        fn ifd_offsets(&self) -> Vec<usize> {
            let mut offsets = Vec::with_capacity(self.ifds.len());
            let mut position = 8usize;
            for ifd in &self.ifds {
                offsets.push(position);
                position += 2 + ifd.entries.len() * 12 + 4;
            }
            offsets
        }

        /// Lays the fixture out and returns the file bytes.
        fn build(&self) -> Vec<u8> {
            let ifd_offsets = self.ifd_offsets();
            let mut blob_offsets = Vec::with_capacity(self.blobs.len());
            let mut position = 8usize;
            for ifd in &self.ifds {
                position += 2 + ifd.entries.len() * 12 + 4;
            }
            for blob in &self.blobs {
                blob_offsets.push(position);
                position += blob.len();
                if position % 2 == 1 {
                    position += 1;
                }
            }

            let mut out = vec![0x49, 0x49, 0x2A, 0x00];
            out.extend_from_slice(&(ifd_offsets[0] as u32).to_le_bytes());
            for ifd in &self.ifds {
                out.extend_from_slice(&(ifd.entries.len() as u16).to_le_bytes());
                for (tag, typ, count, value) in &ifd.entries {
                    out.extend_from_slice(&tag.to_le_bytes());
                    out.extend_from_slice(&typ.to_le_bytes());
                    out.extend_from_slice(&count.to_le_bytes());
                    let raw = match value {
                        FixtureValue::Number(number) => *number,
                        FixtureValue::Blob(index) => blob_offsets[*index] as u32,
                    };
                    out.extend_from_slice(&raw.to_le_bytes());
                }
                let next = ifd.next.map(|index| ifd_offsets[index] as u32).unwrap_or(0);
                out.extend_from_slice(&next.to_le_bytes());
            }
            for blob in &self.blobs {
                out.extend_from_slice(blob);
                if out.len() % 2 == 1 {
                    out.push(0);
                }
            }
            out
        }
    }

    /// A JPEG blob of the requested total length, with the start and end
    /// markers a real file carries.
    fn jpeg_bytes(total: usize) -> Vec<u8> {
        let total = total.max(4);
        let mut bytes = vec![0x11u8; total];
        bytes[0] = 0xFF;
        bytes[1] = 0xD8;
        bytes[total - 2] = 0xFF;
        bytes[total - 1] = 0xD9;
        bytes
    }

    /// Writes a fixture to a `.dng` in a fresh temp directory and returns the
    /// path.
    fn write_dng(dir: &std::path::Path, name: &str, fixture: &TiffFixture) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, fixture.build()).expect("write fixture");
        path
    }

    #[test]
    fn preview_tags_return_the_stored_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        let blob = fixture.blob(jpeg_bytes(64));
        fixture.ifd(vec![
            (PREVIEW_IMAGE_START, 4, 1, FixtureValue::Blob(blob)),
            (PREVIEW_IMAGE_LENGTH, 4, 1, FixtureValue::Number(64)),
        ]);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        let preview = read_dng_preview(&path).expect("preview");
        assert_eq!(preview, jpeg_bytes(64));
    }

    #[test]
    fn ifd1_interchange_format_is_used_without_preview_tags() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        let blob = fixture.blob(jpeg_bytes(80));
        let ifd0 = fixture.ifd(vec![(256, 4, 1, FixtureValue::Number(400))]);
        let ifd1 = fixture.ifd(vec![
            (JPEG_INTERCHANGE_FORMAT, 4, 1, FixtureValue::Blob(blob)),
            (
                JPEG_INTERCHANGE_FORMAT_LENGTH,
                4,
                1,
                FixtureValue::Number(80),
            ),
        ]);
        fixture.link(ifd0, ifd1);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        let preview = read_dng_preview(&path).expect("preview");
        assert_eq!(preview, jpeg_bytes(80));
    }

    #[test]
    fn ifd0_interchange_format_is_used_without_a_thumbnail() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        let blob = fixture.blob(jpeg_bytes(48));
        fixture.ifd(vec![
            (JPEG_INTERCHANGE_FORMAT, 4, 1, FixtureValue::Blob(blob)),
            (
                JPEG_INTERCHANGE_FORMAT_LENGTH,
                4,
                1,
                FixtureValue::Number(48),
            ),
        ]);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        let preview = read_dng_preview(&path).expect("preview");
        assert_eq!(preview, jpeg_bytes(48));
    }

    #[test]
    fn a_sub_ifd_preview_is_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        let blob = fixture.blob(jpeg_bytes(32));
        let ifd0 = fixture.ifd(vec![(256, 4, 1, FixtureValue::Number(400))]);
        let sub = fixture.ifd(vec![
            (JPEG_INTERCHANGE_FORMAT, 4, 1, FixtureValue::Blob(blob)),
            (
                JPEG_INTERCHANGE_FORMAT_LENGTH,
                4,
                1,
                FixtureValue::Number(32),
            ),
        ]);
        // The SubIFDs tag on IFD0 points at the reduced-resolution IFD. That
        // offset depends on the layout, which depends on this entry being
        // present, so the entry is added first and patched once the offsets
        // are known.
        fixture.ifds[ifd0]
            .entries
            .push((SUB_IFD_TAG, 4, 1, FixtureValue::Number(0)));
        let offsets = fixture.ifd_offsets();
        fixture.ifds[ifd0]
            .entries
            .last_mut()
            .expect("subifd entry")
            .3 = FixtureValue::Number(offsets[sub] as u32);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        let preview = read_dng_preview(&path).expect("preview");
        assert_eq!(preview, jpeg_bytes(32));
    }

    #[test]
    fn the_largest_valid_jpeg_wins() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        let small = fixture.blob(jpeg_bytes(40));
        let large = fixture.blob(jpeg_bytes(400));
        let ifd0 = fixture.ifd(vec![
            (PREVIEW_IMAGE_START, 4, 1, FixtureValue::Blob(small)),
            (PREVIEW_IMAGE_LENGTH, 4, 1, FixtureValue::Number(40)),
        ]);
        let ifd1 = fixture.ifd(vec![
            (JPEG_INTERCHANGE_FORMAT, 4, 1, FixtureValue::Blob(large)),
            (
                JPEG_INTERCHANGE_FORMAT_LENGTH,
                4,
                1,
                FixtureValue::Number(400),
            ),
        ]);
        fixture.link(ifd0, ifd1);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        let preview = read_dng_preview(&path).expect("preview");
        assert_eq!(preview, jpeg_bytes(400));
    }

    #[test]
    fn a_strip_payload_is_not_a_preview() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        // Raw sensor data: no JPEG marker at the front.
        let strip = fixture.blob(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05]);
        fixture.ifd(vec![
            (PREVIEW_IMAGE_START, 4, 1, FixtureValue::Blob(strip)),
            (PREVIEW_IMAGE_LENGTH, 4, 1, FixtureValue::Number(6)),
        ]);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        match read_dng_preview(&path) {
            Err(MetadataError::PreviewUnavailable { path: got }) => {
                assert_eq!(got, path.display().to_string());
            }
            other => panic!("expected PreviewUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_dng_without_any_preview_reports_it_as_unavailable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        fixture.ifd(vec![
            (256, 4, 1, FixtureValue::Number(400)),
            (257, 4, 1, FixtureValue::Number(300)),
        ]);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        match read_dng_preview(&path) {
            Err(MetadataError::PreviewUnavailable { path: got }) => {
                assert_eq!(got, path.display().to_string());
            }
            other => panic!("expected PreviewUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_preview_over_the_cap_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fixture = TiffFixture::new();
        let oversized = fixture.blob(jpeg_bytes(MAX_PREVIEW_BYTES + 1));
        fixture.ifd(vec![
            (PREVIEW_IMAGE_START, 4, 1, FixtureValue::Blob(oversized)),
            (
                PREVIEW_IMAGE_LENGTH,
                4,
                1,
                FixtureValue::Number((MAX_PREVIEW_BYTES + 1) as u32),
            ),
        ]);
        let path = write_dng(dir.path(), "a.dng", &fixture);

        match read_dng_preview(&path) {
            Err(MetadataError::PreviewUnavailable { .. }) => {}
            other => panic!("expected PreviewUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_jpeg_is_not_read_as_a_dng_preview() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, jpeg_bytes(32)).expect("write fixture");
        match read_dng_preview(&path) {
            Err(MetadataError::UnsupportedFormat {
                path: got,
                extension,
            }) => {
                assert_eq!(got, path.display().to_string());
                assert_eq!(extension, "jpg");
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_dng_is_an_io_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.dng");
        match read_dng_preview(&path) {
            Err(MetadataError::Io(_)) => {}
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
