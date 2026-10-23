import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Event name emitted after every imported file.
 *
 * Kept in sync with `commands::import::IMPORT_PROGRESS_EVENT`.
 */
export const IMPORT_PROGRESS_EVENT = "import-progress";

/** Progress for one imported file. */
export type ImportProgress = {
  /** Session being imported into; lets the UI ignore another run's events. */
  session_id: string;
  done: number;
  total: number;
  /** Destination path of the copy that was just recorded. */
  path: string;
};

/**
 * Why one file was not imported.
 *
 * Mirrors the Rust `ImportError` enum. Serde encodes unit variants as a plain
 * string and data variants as a single-key object, so both shapes appear.
 */
export type ImportError =
  | "NoImagesFound"
  | { SessionNotFound: { session_id: string } }
  | { NotImplemented: { path: string } }
  | { InvalidFileName: { path: string } }
  | { MissingContentHash: { path: string } }
  | { UnsupportedFormat: { path: string; extension: string } }
  | { MagicMismatch: { path: string } }
  | { CopyFailed: { path: string; reason: string } }
  | { RecordFailed: { path: string; reason: string } }
  | { DestinationExists: { path: string } }
  | { NotAnalysisReady: { image_id: string } };

/** A candidate that could not be imported, and why. */
export type RejectedFile = {
  source_path: string;
  reason: ImportError;
};

/** A candidate whose content matches an image that is already stored. */
export type DuplicateFlag = {
  source_path: string;
  existing_image_id: string;
  content_hash: string;
};

/** Outcome of one import run. */
export type ImportReport = {
  session_id: string;
  /** Persisted image ids, usable with {@link getAnalysisInput}. */
  imported_image_ids: string[];
  duplicates: DuplicateFlag[];
  rejected: RejectedFile[];
  /** Imported DNGs with no JPEG sibling: not analysable yet. */
  not_analysis_ready: string[];
};

/**
 * The image a plugin may analyse.
 *
 * Kept in sync with the Rust `plugin_manager::payload::ImageMetadata`.
 */
export type AnalysisInput = {
  path: string;
  has_gps: boolean;
  width: number;
  height: number;
  format: string;
};

/** Options for {@link importImages}. */
export type ImportOptions = {
  /** Scan each source folder (non-recursive) instead of treating it as a file. */
  fromFolder?: boolean;
  /** Called after every imported file. */
  onProgress?: (progress: ImportProgress) => void;
};

/**
 * Copies picked files into the app's library and records them in the session.
 *
 * The originals are only read. Once the promise resolves, every id in
 * `imported_image_ids` refers to a copy, so the user may move or delete the
 * folder they imported from.
 *
 * @param sessionId - Destination session UUID.
 * @param sources - Absolute paths of files, or of folders when `fromFolder`.
 * @param options - Folder mode and a progress callback.
 * @returns The import report, including per-file rejections.
 * @throws The backend error when the run itself fails (unknown session, no
 * importable files, storage problem).
 */
export async function importImages(
  sessionId: string,
  sources: string[],
  options: ImportOptions = {},
): Promise<ImportReport> {
  const { fromFolder = false, onProgress } = options;
  let unlisten: UnlistenFn | undefined;
  if (onProgress) {
    unlisten = await listen<ImportProgress>(IMPORT_PROGRESS_EVENT, (event) =>
      onProgress(event.payload),
    );
  }
  try {
    return await invoke<ImportReport>("import_images", {
      sessionId,
      sources,
      fromFolder,
    });
  } finally {
    unlisten?.();
  }
}

/**
 * Returns the image a plugin may analyse for `imageId`.
 *
 * For a JPEG that is the image itself. For a DNG it is the same-stem JPEG
 * imported with it; a DNG without one is rejected with `IMAGE_NOT_READY`.
 *
 * @param imageId - Persisted image id from {@link ImportReport}.
 * @throws The backend error; check `code` for `IMAGE_NOT_READY`.
 */
export async function getAnalysisInput(imageId: string): Promise<AnalysisInput> {
  return invoke<AnalysisInput>("get_analysis_input", { imageId });
}

/**
 * Returns the backend error code for a rejection reason.
 *
 * Useful for branching on `SOURCE_CHANGED` or `DESTINATION_EXISTS` without
 * parsing the display message.
 */
const IMPORT_ERROR_CODES: Record<string, string> = {
  NoImagesFound: "NO_IMAGES_FOUND",
  SessionNotFound: "SESSION_NOT_FOUND",
  NotImplemented: "NOT_IMPLEMENTED",
  InvalidFileName: "INVALID_FILE_NAME",
  MissingContentHash: "MISSING_CONTENT_HASH",
  UnsupportedFormat: "UNSUPPORTED_FORMAT",
  MagicMismatch: "MAGIC_MISMATCH",
  CopyFailed: "COPY_FAILED",
  RecordFailed: "RECORD_FAILED",
  DestinationExists: "DESTINATION_EXISTS",
  NotAnalysisReady: "IMAGE_NOT_READY",
};

export function importErrorCode(reason: ImportError): string {
  const variant = typeof reason === "string" ? reason : (Object.keys(reason)[0] ?? "");
  return IMPORT_ERROR_CODES[variant] ?? variant;
}
