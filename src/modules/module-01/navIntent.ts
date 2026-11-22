import type { Project } from "./projectCommands";

/**
 * Bridge between the app-shell navbar and Module 01.
 *
 * Module 01 owns the selected project and its sessions, so the navbar never touches that state
 * directly. It sends an intent instead, and Module 01 applies it. The intent is parked in
 * `pending` first, which means it still works when Module 01 is not mounted yet: the navbar
 * switches module, Module 01 mounts and takes the parked intent.
 */
export type M1Intent =
  | { kind: "select-project"; project: Project }
  | { kind: "open-session"; project: Project; sessionId: string }
  | { kind: "create-session"; project: Project }
  | { kind: "create-project" };

export const M1_INTENT_EVENT = "m1-intent";

/** Slug of Module 01 in `modules.ts`. */
const M1_SLUG = "image-project-manager";

let pending: M1Intent | null = null;

export function sendM1Intent(intent: M1Intent): void {
  pending = intent;
  // Already mounted: the page listens for this and takes the intent right away.
  window.dispatchEvent(new CustomEvent(M1_INTENT_EVENT));
  // Somewhere else: AppShell switches module, and the page takes the intent when it mounts.
  window.dispatchEvent(new CustomEvent("open-module", { detail: { slug: M1_SLUG } }));
}

export function takeM1Intent(): M1Intent | null {
  const intent = pending;
  pending = null;
  return intent;
}
