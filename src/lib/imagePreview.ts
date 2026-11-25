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
 * How many image requests may be in flight at once.
 *
 * Every image command decodes or reads a multi-megabyte file in the backend.
 * A list that fires one request per row (or a dialog previewing a few hundred
 * picked files) would otherwise start them all at the same moment, which
 * saturates every core and starves the UI. Requests beyond the limit wait
 * their turn in the order they were made.
 */
const MAX_CONCURRENT_IMAGE_REQUESTS = 4;

let activeRequests = 0;
const waitingRequests: Array<() => void> = [];

function runLimited<T>(task: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const start = () => {
      activeRequests += 1;
      Promise.resolve()
        .then(task)
        .then(resolve, reject)
        .finally(() => {
          activeRequests -= 1;
          waitingRequests.shift()?.();
        });
    };
    if (activeRequests < MAX_CONCURRENT_IMAGE_REQUESTS) start();
    else waitingRequests.push(start);
  });
}

/**
 * Reads a displayable preview of an aerial image.
 *
 * A browser cannot put a raw `.dng` in an `<img>` tag, so the backend serves
 * the JPEG the DNG already stores. A `.jpg` is served as it is.
 *
 * This is the viewer: it may return the whole frame. List views and small
 * boxes should use {@link loadImageThumbnail} instead, and a file that is not
 * in the library yet should use {@link loadSourceThumbnail}.
 *
 * @param path - Absolute path of the image on disk.
 * @returns The preview as an object URL, ready for an `<img src>`.
 * @throws The backend error. Check `code` for `PREVIEW_UNAVAILABLE`.
 */
export async function loadImagePreview(path: string): Promise<string> {
  const preview = await runLimited(() => invoke<ImagePreview>("preview_image", { path }));
  const bytes = decodeBase64(preview.base64);
  return URL.createObjectURL(new Blob([bytes.buffer], { type: preview.mime }));
}

/**
 * Object URLs already produced this session, keyed by path and file size.
 *
 * A thumbnail is a few kilobytes and immutable for a given file, so
 * re-rendering a list must not re-fetch it. The key includes the file size
 * because a new file can reuse a path (replace-then-reimport): the backend
 * decides real freshness by path, mtime, and size, and the row's size is the
 * only part the frontend knows. The URLs live for the session: components
 * share them and must not revoke them on unmount.
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
 * @param fileSize - The row's recorded size, when known; a part of the cache
 *   identity so a new file at a reused path is not served a stale thumbnail.
 * @returns The thumbnail as an object URL, ready for an `<img src>`.
 * @throws The backend error. Check `code` for `PREVIEW_UNAVAILABLE`.
 */
export function loadImageThumbnail(path: string, fileSize?: number | null): Promise<string> {
  const key = `${path}\u0000${fileSize ?? ""}`;
  const cached = thumbnailUrls.get(key);
  if (cached) return cached;
  const pending = runLimited(() => invoke<ImagePreview>("thumbnail_image", { path }))
    .then((preview) => {
      const bytes = decodeBase64(preview.base64);
      return URL.createObjectURL(new Blob([bytes.buffer], { type: preview.mime }));
    })
    .catch((error) => {
      // A transient failure must not poison the cache for later attempts.
      thumbnailUrls.delete(key);
      throw error;
    });
  thumbnailUrls.set(key, pending);
  return pending;
}

/**
 * Reads a small thumbnail of a file that is not in the library yet, such as
 * one picked in the import dialog.
 *
 * Unlike {@link loadImageThumbnail} the backend accepts any path and keeps no
 * cache, so the result is not shared: the caller owns the object URL and must
 * `URL.revokeObjectURL` it when the image leaves the screen.
 *
 * @param path - Absolute path of the picked file.
 * @param signal - Abort to drop the request while it is still waiting its
 *   turn; a request already running is left to finish.
 * @returns The thumbnail as an object URL.
 * @throws The backend error (`PREVIEW_UNAVAILABLE` for a DNG that stores no
 *   JPEG), or an `AbortError` when aborted before it started.
 */
export function loadSourceThumbnail(path: string, signal?: AbortSignal): Promise<string> {
  return runLimited(async () => {
    signal?.throwIfAborted();
    const preview = await invoke<ImagePreview>("thumbnail_source_image", { path });
    const bytes = decodeBase64(preview.base64);
    return URL.createObjectURL(new Blob([bytes.buffer], { type: preview.mime }));
  });
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
