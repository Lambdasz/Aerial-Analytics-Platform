import { invoke } from "@tauri-apps/api/core";

/**
 * A JPEG preview returned by the backend.
 */
export type ImagePreview = {
  mime: string;
  base64: string;
};

/**
 * The error code the backend uses when a file holds no preview image.
 *
 * A DNG that stores only sensor data has no embedded JPEG. That is not a
 * broken file, so the caller should say the preview is unavailable rather
 * than report a failure.
 */
export const PREVIEW_UNAVAILABLE = "PREVIEW_UNAVAILABLE";

/**
 * Reads a displayable preview of an aerial image.
 *
 * A browser cannot put a raw `.dng` in an `<img>` tag, so the backend serves
 * the JPEG the DNG already stores. A `.jpg` is served as it is.
 *
 * This is the viewer: it may return the whole frame. List views and small
 * boxes should use {@link loadImageThumbnail} instead.
 *
 * @param path - Absolute path of the image on disk.
 * @returns The preview as an object URL, ready for an `<img src>`.
 * @throws The backend error. Check `code` for `PREVIEW_UNAVAILABLE`.
 */
export async function loadImagePreview(path: string): Promise<string> {
  const preview = await invoke<ImagePreview>("preview_image", { path });
  const bytes = decodeBase64(preview.base64);
  return URL.createObjectURL(new Blob([bytes.buffer], { type: preview.mime }));
}

/**
 * Object URLs already produced this session, keyed by source path.
 *
 * A thumbnail is a few kilobytes and immutable for a given file, so
 * re-rendering a list must not re-fetch it. The URLs live for the session:
 * components share them and must not revoke them on unmount.
 */
const thumbnailUrls = new Map<string, Promise<string>>();

/**
 * Reads a small thumbnail of an aerial image, cached per session.
 *
 * Unlike {@link loadImagePreview}, this never yields more than a 160 px long
 * edge: the backend reuses a JPEG's own EXIF thumbnail and caches the result
 * on disk, so rendering a list of images stays cheap. Concurrent callers for
 * the same path share one request.
 *
 * @param path - Absolute path of the image on disk.
 * @returns The thumbnail as an object URL, ready for an `<img src>`.
 * @throws The backend error. Check `code` for `PREVIEW_UNAVAILABLE`.
 */
export function loadImageThumbnail(path: string): Promise<string> {
  const cached = thumbnailUrls.get(path);
  if (cached) return cached;
  const pending = invoke<ImagePreview>("thumbnail_image", { path })
    .then((preview) => {
      const bytes = decodeBase64(preview.base64);
      return URL.createObjectURL(new Blob([bytes.buffer], { type: preview.mime }));
    })
    .catch((error) => {
      // A transient failure must not poison the cache for later attempts.
      thumbnailUrls.delete(path);
      throw error;
    });
  thumbnailUrls.set(path, pending);
  return pending;
}

/**
 * Decodes a base64 string into bytes.
 *
 * `atob` returns a binary string, so each character is re-expanded into its
 * byte. The buffer is allocated explicitly so the result is a plain
 * `ArrayBuffer` and not a view onto a shared one.
 */
function decodeBase64(encoded: string): Uint8Array<ArrayBuffer> {
  const binary = atob(encoded);
  const buffer = new ArrayBuffer(binary.length);
  const bytes = new Uint8Array(buffer);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}
