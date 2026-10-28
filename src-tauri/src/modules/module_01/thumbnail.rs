//! Cached list thumbnails for stored aerial images (Module 1).
//!
//! The session image list must not decode full frames: [`thumbnail_jpeg`]
//! reuses a JPEG's own EXIF thumbnail when it is already small enough and
//! re-encodes anything else (including a DNG's embedded preview) to a
//! [`THUMB_LONG_EDGE`] long edge. Results are cached under
//! `<library_root>/.thumbs`, keyed by the source path, mtime, and size.
//!
//! The import pipeline warms the cache for every copy it writes
//! ([`warm_thumbnail`]), so opening a session's list is a few-kilobyte read
//! instead of a decode pass.

use std::io::{Cursor, Write};
use std::path::Path;

use exif::{In, Tag};
use sha2::{Digest, Sha256};

use super::dng_preview::read_dng_preview;
use super::error::MetadataError;

/// The long edge a thumbnail is bounded to, in pixels.
pub(crate) const THUMB_LONG_EDGE: u32 = 160;

/// Returns the cached thumbnail of the image at `path`, building it on a miss.
///
/// A JPEG's own EXIF thumbnail is reused when it is already small enough, and
/// everything else is decoded and re-encoded to a [`THUMB_LONG_EDGE`] long
/// edge, so a list render never decodes a full frame. The result is cached in
/// `cache_dir` as `<key>.jpg`, where the key hashes the absolute path, mtime,
/// and size — an unchanged file is a few-kilobyte read, while a replaced file
/// misses the cache.
///
/// # Purity
///
/// Impure — reads the source file and writes the cache.
///
/// # Errors
///
/// [`MetadataError::UnsupportedFormat`] for an extension this module does not
/// handle, [`MetadataError::PreviewUnavailable`] for a DNG that stores no
/// JPEG, and [`MetadataError::MagicMismatch`]/[`MetadataError::Io`] for a
/// file that cannot be read.
pub(crate) fn thumbnail_jpeg(path: &Path, cache_dir: &Path) -> Result<Vec<u8>, MetadataError> {
    let cache_file = cache_dir.join(format!("{}.jpg", thumbnail_cache_key(path)?));
    if let Ok(bytes) = std::fs::read(&cache_file) {
        return Ok(bytes);
    }
    let bytes = build_thumbnail(path)?;
    publish_thumbnail(&cache_file, &bytes);
    Ok(bytes)
}

/// Builds and caches the thumbnail for `path`, swallowing every error.
///
/// Import calls this for each copy it writes, so the first render of a
/// session list is a cache read instead of a decode pass. A failed warm-up is
/// not an import failure — the list builds the thumbnail lazily instead.
pub(crate) fn warm_thumbnail(path: &Path, cache_dir: &Path) {
    let _ = thumbnail_jpeg(path, cache_dir);
}

/// Cache key for one source file: SHA-256 over its absolute path, mtime, and size.
///
/// The file's content is never hashed: mtime and size already make a replaced
/// file miss the cache, and hashing megabytes would defeat its purpose.
fn thumbnail_cache_key(path: &Path) -> Result<String, MetadataError> {
    let metadata = std::fs::metadata(path)?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|age| age.as_nanos())
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(modified_ns.to_le_bytes());
    hasher.update(metadata.len().to_le_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

/// Stores `bytes` at `cache_file`, best-effort: a read-only library still serves thumbs.
///
/// Uses a unique named temp file per writer so concurrent writes (e.g. background
/// import warming racing an on-demand UI request) never overwrite each other's partial
/// file, and persists atomically into place.
fn publish_thumbnail(cache_file: &Path, bytes: &[u8]) {
    let Some(dir) = cache_file.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    if let Ok(mut tmp) = tempfile::NamedTempFile::new_in(dir) {
        if tmp.write_all(bytes).is_ok() {
            let _ = tmp.persist(cache_file);
        }
    }
}

/// Removes the cached thumbnail for `path` from `cache_dir`, if present.
///
/// Called when an image is deleted or moved, preventing stale cache files
/// from accumulating indefinitely in `<library_root>/.thumbs`.
pub(crate) fn discard_thumbnail(path: &Path, cache_dir: &Path) {
    if let Ok(key) = thumbnail_cache_key(path) {
        let cache_file = cache_dir.join(format!("{key}.jpg"));
        let _ = std::fs::remove_file(cache_file);
    }
}

/// Builds the thumbnail bytes for one image without consulting the cache.
fn build_thumbnail(path: &Path) -> Result<Vec<u8>, MetadataError> {
    let extension = path
        .extension()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .trim_start_matches('.')
        .to_ascii_lowercase();
    match extension.as_str() {
        "dng" => shrink_to_thumbnail(&read_dng_preview(path)?),
        "jpg" | "jpeg" => jpeg_thumbnail(path),
        _ => Err(MetadataError::UnsupportedFormat {
            path: path.display().to_string(),
            extension,
        }),
    }
}

/// A JPEG's thumbnail: the EXIF one when it fits, otherwise the decoded frame.
fn jpeg_thumbnail(path: &Path) -> Result<Vec<u8>, MetadataError> {
    let bytes = std::fs::read(path)?;
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Err(MetadataError::MagicMismatch {
            path: path.display().to_string(),
        });
    }
    if let Some(embedded) = embedded_exif_thumbnail(&bytes) {
        if jpeg_dimensions(&embedded).is_some_and(|(w, h)| w.max(h) <= THUMB_LONG_EDGE) {
            // Already small: reuse byte-for-byte; no frame is ever decoded.
            return Ok(embedded);
        }
        return shrink_to_thumbnail(&embedded);
    }
    shrink_to_thumbnail(&bytes)
}

/// The JPEG stored in the EXIF thumbnail slot, when there is one.
fn embedded_exif_thumbnail(jpeg_bytes: &[u8]) -> Option<Vec<u8>> {
    let exif = exif::Reader::new()
        .continue_on_error(true)
        .read_from_container(&mut Cursor::new(jpeg_bytes))
        .ok()?;
    let offset = exif
        .get_field(Tag::JPEGInterchangeFormat, In::THUMBNAIL)?
        .value
        .get_uint(0)? as usize;
    let length = exif
        .get_field(Tag::JPEGInterchangeFormatLength, In::THUMBNAIL)?
        .value
        .get_uint(0)? as usize;
    let thumbnail = exif.buf().get(offset..offset.checked_add(length)?)?;
    thumbnail
        .starts_with(&[0xFF, 0xD8])
        .then(|| thumbnail.to_vec())
}

/// `(width, height)` of a JPEG from its first SOF marker, without decoding pixels.
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            return None;
        }
        let code = bytes[pos + 1];
        if code == 0xFF {
            // Fill byte; advance until the real marker code.
            pos += 1;
            continue;
        }
        if is_standalone_jpeg_marker(code) {
            pos += 2;
            continue;
        }
        let length = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        if length < 2 || pos + 2 + length > bytes.len() {
            return None;
        }
        if is_sof_marker(code) {
            if length < 7 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[pos + 5], bytes[pos + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[pos + 7], bytes[pos + 8]]) as u32;
            return (width > 0 && height > 0).then_some((width, height));
        }
        if code == 0xDA {
            // Scan data starts; no SOF was found before it.
            return None;
        }
        pos += 2 + length;
    }
    None
}

/// Markers that carry no length field.
fn is_standalone_jpeg_marker(code: u8) -> bool {
    code == 0x01 || (0xD0..=0xD7).contains(&code)
}

/// SOF markers of any frame type (excluding the non-SOF entries in `C0..=CF`).
fn is_sof_marker(code: u8) -> bool {
    matches!(code, 0xC0..=0xCF) && !matches!(code, 0xC4 | 0xC8 | 0xCC)
}

/// Decodes `bytes` (a JPEG) and re-encodes it with a [`THUMB_LONG_EDGE`] long edge.
fn shrink_to_thumbnail(bytes: &[u8]) -> Result<Vec<u8>, MetadataError> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes));
    reader.set_format(image::ImageFormat::Jpeg);
    let decoded = reader
        .decode()
        .map_err(|err| MetadataError::Io(std::io::Error::other(err)))?;
    let resized = decoded.resize(
        THUMB_LONG_EDGE,
        THUMB_LONG_EDGE,
        image::imageops::FilterType::Triangle,
    );
    let mut out = Vec::new();
    resized
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .map_err(|err| MetadataError::Io(std::io::Error::other(err)))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A JPEG of the given size, flat enough to compress cheaply.
    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let source = image::RgbImage::from_pixel(width, height, image::Rgb([120, 90, 60]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
            .expect("encode jpeg");
        bytes
    }

    /// A JPEG whose EXIF block carries `thumbnail` in the thumbnail slot.
    ///
    /// The container holds no image data of its own: a thumbnail that comes
    /// back from it can only have come from the EXIF segment.
    fn jpeg_with_exif_thumbnail(thumbnail: &[u8]) -> Vec<u8> {
        let mut tiff = Vec::new();
        let mut writer = exif::experimental::Writer::new();
        // The writer refuses an empty IFD0, so it carries one real field.
        let description = exif::Field {
            tag: Tag::ImageDescription,
            ifd_num: In::PRIMARY,
            value: exif::Value::Ascii(vec![b"fixture".to_vec()]),
        };
        writer.push_field(&description);
        writer.set_jpeg(thumbnail, exif::In::THUMBNAIL);
        writer
            .write(&mut Cursor::new(&mut tiff), false)
            .expect("write exif");

        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let payload = 2 + 6 + tiff.len();
        jpeg.extend_from_slice(&(payload as u16).to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    /// A little-endian TIFF (DNG) with the JPEG-interchange tags on IFD0
    /// pointing at `blob`, which is where `read_dng_preview` looks third.
    fn dng_with_preview(blob: &[u8]) -> Vec<u8> {
        let blob_offset = 38u32; // header (8) + IFD0 (2 + 2*12 + 4)
        let mut tiff = vec![0x49, 0x49, 0x2A, 0x00];
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        // JPEGInterchangeFormat (513), LONG, count 1
        tiff.extend_from_slice(&513u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&blob_offset.to_le_bytes());
        // JPEGInterchangeFormatLength (514), LONG, count 1
        tiff.extend_from_slice(&514u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        tiff.extend_from_slice(blob);
        tiff
    }

    /// Decodes a JPEG so a test can assert its dimensions.
    fn dimensions(bytes: &[u8]) -> (u32, u32) {
        let decoded = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .expect("guess format")
            .decode()
            .expect("decode");
        (decoded.width(), decoded.height())
    }

    #[test]
    fn thumbnail_reuses_an_embedded_exif_thumbnail_without_decoding() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = tempfile::tempdir().expect("cache");
        let embedded = jpeg(160, 120);
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, jpeg_with_exif_thumbnail(&embedded)).expect("write");

        let thumb = thumbnail_jpeg(&path, cache.path()).expect("thumbnail");
        // Byte-identical to the embedded JPEG: nothing was decoded or re-encoded.
        assert_eq!(thumb, embedded);
    }

    #[test]
    fn thumbnail_shrinks_a_jpeg_without_an_exif_thumbnail() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = tempfile::tempdir().expect("cache");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, jpeg(320, 240)).expect("write");

        let thumb = thumbnail_jpeg(&path, cache.path()).expect("thumbnail");
        assert_eq!(dimensions(&thumb), (THUMB_LONG_EDGE, 120));
    }

    #[test]
    fn thumbnail_second_call_reads_the_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = tempfile::tempdir().expect("cache");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, jpeg(320, 240)).expect("write");

        thumbnail_jpeg(&path, cache.path()).expect("first");
        let cached: Vec<_> = std::fs::read_dir(cache.path())
            .expect("cache dir")
            .flatten()
            .map(|entry| entry.path())
            .collect();
        assert_eq!(cached.len(), 1, "one cache file per source file");
        std::fs::write(&cached[0], b"served-from-cache").expect("overwrite cache");

        let second = thumbnail_jpeg(&path, cache.path()).expect("second");
        assert_eq!(second, b"served-from-cache");
    }

    #[test]
    fn thumbnail_shrinks_a_dng_embedded_preview() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = tempfile::tempdir().expect("cache");
        let path = dir.path().join("a.dng");
        let preview = jpeg(320, 240);
        std::fs::write(&path, dng_with_preview(&preview)).expect("write");

        let thumb = thumbnail_jpeg(&path, cache.path()).expect("thumbnail");
        assert_ne!(thumb, preview, "the raw embedded preview is not returned");
        assert_eq!(dimensions(&thumb), (THUMB_LONG_EDGE, 120));
    }

    #[test]
    fn warm_thumbnail_writes_the_cache_without_failing_on_a_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = tempfile::tempdir().expect("cache");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, jpeg(320, 240)).expect("write");

        warm_thumbnail(&path, cache.path());
        assert_eq!(
            std::fs::read_dir(cache.path()).expect("cache dir").count(),
            1,
            "the warm-up wrote the cache entry"
        );

        // A missing source is swallowed, not propagated.
        warm_thumbnail(&dir.path().join("gone.jpg"), cache.path());
    }
}
