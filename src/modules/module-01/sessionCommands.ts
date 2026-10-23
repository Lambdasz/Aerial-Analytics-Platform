import { invoke } from "@tauri-apps/api/core";

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

export function recalculateSessionDateRange(sessionId: string) {
  return invoke<void>("recalculate_session_date_range", { sessionId });
}

export function deleteSession(sessionId: string) {
  return invoke<void>("delete_session", { sessionId });
}
