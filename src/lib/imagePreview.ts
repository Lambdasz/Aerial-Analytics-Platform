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
