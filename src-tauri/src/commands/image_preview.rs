//! Preview image command.
//!
//! Returns a displayable JPEG for an aerial image on disk. A browser cannot
//! put a raw `.dng` in an `<img>` tag, so a DNG is served from the JPEG it
//! already stores. A `.jpg` is served as it is, shrunk when it is large.
//!
//! `thumbnail_image` is the small, cached variant list views use. Its work
//! runs on a blocking thread (the webview must not wait on file reads while
//! building a list), and the import pipeline warms the same cache, so opening
//! a session's image list is a few-kilobyte read.

use std::io::Cursor;
use std::path::Path;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde::Serialize;

use crate::modules::module_01::dng_preview::read_dng_preview;
use crate::modules::module_01::error::MetadataError;
use crate::modules::module_01::thumbnail;
use crate::plugin_manager::error::CommandError;
use crate::AppState;

/// The MIME type of every preview this command returns.
const PREVIEW_MIME: &str = "image/jpeg";

/// A JPEG preview, ready for the frontend to display.
#[derive(Debug, Clone, Serialize)]
pub struct ImagePreview {
    /// The MIME type of `base64`. Always `"image/jpeg"`.
    pub mime: String,
    /// The JPEG bytes, base64-encoded.
    ///
    /// The bytes travel as base64 rather than as a byte array because
    /// Tauri would otherwise send every byte as a JSON number.
    pub base64: String,
}

/// The largest JPEG returned as it is, in bytes.
const MAX_INLINE_JPEG_BYTES: u64 = 4 * 1024 * 1024;

/// The largest JPEG this command will read from disk, in bytes.
const MAX_JPEG_INPUT_BYTES: u64 = 80 * 1024 * 1024;

/// The long edge a resized JPEG is bounded to, in pixels.
const MAX_LONG_EDGE: u32 = 1280;

/// Reads a displayable JPEG preview of the image at `path`.
///
/// The extension must be `jpg`, `jpeg` or `dng`. A `.dng` is served from the
/// JPEG it stores; a `.jpg` is served as it is when it is small enough, and
/// resized down to a 1280px long edge when it is not.
///
/// # Errors
///
/// Returns a [`CommandError`] with code `UNSUPPORTED_FORMAT` for an
/// extension this command does not handle, `PREVIEW_UNAVAILABLE` for a DNG
/// that stores no JPEG, and `IO_ERROR` for a file that cannot be read.
#[tauri::command]
pub fn preview_image(path: String) -> Result<ImagePreview, CommandError> {
    let extension = Path::new(&path)
        .extension()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .trim_start_matches('.')
        .to_ascii_lowercase();
    let bytes = match extension.as_str() {
        "dng" => read_dng_preview(Path::new(&path))?,
        "jpg" | "jpeg" => read_jpeg_preview(Path::new(&path))?,
        _ => {
            return Err(MetadataError::UnsupportedFormat {
                path: path.clone(),
                extension,
            }
            .into())
        }
    };
    Ok(ImagePreview {
        mime: PREVIEW_MIME.to_string(),
        base64: BASE64.encode(bytes),
    })
}

/// Reads a JPEG original, shrinking it when it is larger than the inline cap.
fn read_jpeg_preview(path: &Path) -> Result<Vec<u8>, MetadataError> {
    let length = std::fs::metadata(path)?.len();
    if length > MAX_JPEG_INPUT_BYTES {
        return Err(MetadataError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "file is {} MiB, over the {} MiB preview limit",
                length / (1024 * 1024),
                MAX_JPEG_INPUT_BYTES / (1024 * 1024)
            ),
        )));
    }
    let bytes = std::fs::read(path)?;
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Err(MetadataError::MagicMismatch {
            path: path.display().to_string(),
        });
    }
    if length <= MAX_INLINE_JPEG_BYTES {
        return Ok(bytes);
    }
    shrink_jpeg(&bytes)
}

/// Re-encodes a JPEG so its long edge is at most [`MAX_LONG_EDGE`] pixels.
fn shrink_jpeg(bytes: &[u8]) -> Result<Vec<u8>, MetadataError> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes));
    reader.set_format(image::ImageFormat::Jpeg);
    let decoded = reader
        .decode()
        .map_err(|err| MetadataError::Io(std::io::Error::other(err)))?;
    let resized = decoded.resize(
        MAX_LONG_EDGE,
        MAX_LONG_EDGE,
        image::imageops::FilterType::Triangle,
    );
    let mut out = Vec::new();
    resized
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .map_err(|err| MetadataError::Io(std::io::Error::other(err)))?;
    Ok(out)
}

/// Reads a small thumbnail of the image at `path`, caching it under the library.
///
/// This is the list-view counterpart to [`preview_image`]. The file work
/// (EXIF thumbnail extraction or a 160 px resize) runs on a blocking thread,
/// and the result is served from `<library_root>/.thumbs` on every later
/// call.
///
/// # Errors
///
/// The same codes as [`preview_image`]: `UNSUPPORTED_FORMAT` for an extension
/// this command does not handle, `PREVIEW_UNAVAILABLE` for a DNG that stores
/// no JPEG, and `IO_ERROR` for a file that cannot be read.
#[tauri::command]
pub async fn thumbnail_image(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<ImagePreview, CommandError> {
    let cache_dir = state.library_root.join(".thumbs");
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = thumbnail::thumbnail_jpeg(Path::new(&path), &cache_dir)?;
        Ok::<ImagePreview, CommandError>(ImagePreview {
            mime: PREVIEW_MIME.to_string(),
            base64: BASE64.encode(bytes),
        })
    })
    .await
    .map_err(|err| CommandError {
        code: "THUMBNAIL_JOIN_ERROR".to_string(),
        message: format!("the thumbnail task stopped unexpectedly: {err}"),
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

    /// A JPEG of the given size, flat enough to compress cheaply.
    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let source = RgbImage::from_pixel(width, height, Rgb([120, 90, 60]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(source)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode jpeg");
        bytes
    }

    #[test]
    fn preview_image_rejects_an_unsupported_extension() {
        let error = preview_image("a.tif".to_string()).expect_err("expected an error");
        assert_eq!(error.code, "UNSUPPORTED_FORMAT");
    }

    #[test]
    fn preview_image_returns_a_small_jpeg_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.jpg");
        let original = jpeg(64, 48);
        std::fs::write(&path, &original).expect("write");

        let preview = preview_image(path.display().to_string()).expect("preview");
        assert_eq!(preview.mime, "image/jpeg");
        assert_eq!(BASE64.decode(&preview.base64).expect("decode"), original);
    }

    #[test]
    fn preview_image_rejects_a_jpeg_without_jpeg_magic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.jpg");
        std::fs::write(&path, b"not a jpeg").expect("write");

        let error = preview_image(path.display().to_string()).expect_err("expected an error");
        assert_eq!(error.code, "MAGIC_MISMATCH");
    }

    #[test]
    fn shrink_jpeg_bounds_the_long_edge() {
        let original = jpeg(2000, 1500);
        let shrunk = shrink_jpeg(&original).expect("shrink");
        assert!(shrunk.starts_with(&[0xFF, 0xD8]));

        let decoded = image::ImageReader::new(Cursor::new(&shrunk))
            .with_guessed_format()
            .expect("guess format")
            .decode()
            .expect("decode");
        assert!(decoded.width() <= MAX_LONG_EDGE);
        assert!(decoded.height() <= MAX_LONG_EDGE);
    }
}
