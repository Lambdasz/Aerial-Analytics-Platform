import { invoke } from "@tauri-apps/api/core";
import type { ImageMetadata } from "./imageMetadata";

export type SessionStatus = "active" | "archived";

export interface Session {
  id: string;
  projectId: string;
  name: string;
  dateStart: string;
  dateEnd: string;
  status: SessionStatus;
  createdAt: string;
  updatedAt: string;
}

/** Shape of the error Tauri rejects with (see `plugin_manager::error::CommandError`). */
export interface CommandError {
  code: string;
  message: string;
}

/** True when `err` looks like a `CommandError` from the Rust side. */
export function isCommandError(err: unknown): err is CommandError {
  return typeof err === "object" && err !== null && "code" in err && "message" in err;
}

/** Extracts a human-readable message from whatever `invoke()` rejected with. */
export function commandErrorMessage(err: unknown): string {
  if (isCommandError(err)) return `${err.message} (${err.code})`;
  if (err instanceof Error) return err.message;
  return String(err);
}

export function createSession(projectId: string, name: string | null) {
  return invoke<Session>("create_session", { projectId, name });
}

export function getSession(sessionId: string) {
  return invoke<Session>("get_session", { sessionId });
}

export function getSessionsByProject(projectId: string) {
  return invoke<Session[]>("get_sessions_by_project", { projectId });
}

export function updateSessionName(sessionId: string, newName: string) {
  return invoke<Session>("update_session_name", { sessionId, newName });
}

export function updateSessionStatus(sessionId: string, status: SessionStatus) {
  return invoke<Session>("update_session_status", { sessionId, status });
}

export function assignImageToSession(sessionId: string, imageId: string) {
  return invoke<void>("assign_image_to_session", { sessionId, imageId });
}

/** A stored image and the metadata recorded at import. */
export interface StoredImage {
  id: string;
  sessionId: string;
  filePath: string;
  locationLat: number | null;
  locationLon: number | null;
  capturedAt: string | null;
  gpsAltitudeM: number | null;
  width: number | null;
  height: number | null;
  format: string | null;
  make: string | null;
  cameraModel: string | null;
  fileSize: number | null;
  rawSiblingId: string | null;
  analysisReady: boolean | null;
}

export function getImagesBySession(sessionId: string) {
  return invoke<StoredImage[]>("get_images_by_session", { sessionId });
}

/** EXIF and XMP read from the stored file, not just the columns on the row. */
export function getImageMetadata(imageId: string) {
  return invoke<ImageMetadata>("get_image_metadata", { imageId });
}

export function recalculateSessionDateRange(sessionId: string) {
  return invoke<void>("recalculate_session_date_range", { sessionId });
}

/**
 * The result of a session deletion.
 *
 * `filesLeft` is `true` when some copied files could not be removed or
 * rescued into `.trash/` and remain on disk under the session's folder.
 */
export interface DeleteOutcome {
  filesLeft: boolean;
}

/**
 * Deletes a session, its image rows, and the copies it imported.
 *
 * The backend refuses without `confirm: true` (`CONFIRMATION_REQUIRED`), so
 * callers must ask the user first — the dialog lives at the call site.
 */
export function deleteSession(sessionId: string) {
  return invoke<DeleteOutcome>("delete_session", { sessionId, confirm: true });
}

/**
 * The result of deleting one image.
 *
 * `filesLeft` is `true` when the copy could not be removed or rescued into
 * `.trash/` and remains on disk.
 */
export interface DeleteImageOutcome {
  filesLeft: boolean;
}

/**
 * Deletes one image: its row, its pair link, and its copied file.
 *
 * The backend refuses without `confirm: true` (`CONFIRMATION_REQUIRED`); the
 * caller asks the user first.
 */
export function deleteImage(imageId: string) {
  return invoke<DeleteImageOutcome>("delete_image", { imageId, confirm: true });
}

/**
 * Moves one image into another session of the same project.
 *
 * The row and the file move together; `CROSS_PROJECT_IMAGE` and
 * `SAME_SESSION` are refused by the backend.
 */
export function moveImageToSession(imageId: string, targetSessionId: string) {
  return invoke<void>("move_image_to_session", { imageId, targetSessionId });
}

/**
 * Copies one image into another session of the same project.
 *
 * Byte-for-byte copy with a fresh row; nothing is replaced at the
 * destination (a taken name gets a ` (n)` suffix).
 */
export function copyImageToSession(imageId: string, targetSessionId: string) {
  return invoke<void>("copy_image_to_session", { imageId, targetSessionId });
}
