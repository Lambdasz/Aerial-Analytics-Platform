import type { Project } from "./projectCommands";

export type M1Intent =
  | { kind: "select-project"; project: Project }
  | { kind: "open-session"; project: Project; sessionId: string }
  | { kind: "create-session"; project: Project }
  | { kind: "create-project" };

export const M1_INTENT_EVENT = "m1-intent";

const M1_SLUG = "image-project-manager";

let pending: M1Intent | null = null;

export function sendM1Intent(intent: M1Intent): void {
  pending = intent;
  window.dispatchEvent(new CustomEvent(M1_INTENT_EVENT));
  window.dispatchEvent(new CustomEvent("open-module", { detail: { slug: M1_SLUG } }));
}

export function takeM1Intent(): M1Intent | null {
  const intent = pending;
  pending = null;
  return intent;
}
