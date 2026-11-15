import { invoke } from "@tauri-apps/api/core";

/**
 * Event emitted whenever a background task changes. The payload is a
 * {@link TaskSnapshot}.
 *
 * Kept in sync with `models::background_task::BACKGROUND_TASK_EVENT`.
 */
export const BACKGROUND_TASK_EVENT = "background-task-updated";

/** Mirrors the Rust `TaskKind` enum (snake_case). */
export type TaskKind = "blur_scoring";

/** Mirrors the Rust `TaskState` enum (lowercase). */
export type TaskState = "queued" | "running" | "done" | "warn" | "cancelled" | "failed";

/** A point-in-time view of one background task (Rust `TaskSnapshot`, camelCase). */
export type TaskSnapshot = {
  id: string;
  kind: TaskKind;
  title: string;
  sessionId: string;
  sessionName: string;
  state: TaskState;
  /** Items finished so far. */
  done: number;
  total: number;
  /** Finished items that produced no result (too small or undecodable). */
  unscorable: number;
  /** Name of the item that most recently finished. */
  current: string | null;
  /** Failure message when `state` is `failed`. */
  error: string | null;
  /** RFC 3339, UTC. */
  startedAt: string;
  /** RFC 3339, UTC. `null` while the task is live. */
  finishedAt: string | null;
};

/** Whether the task is still queued or running. */
export function isLive(state: TaskState): boolean {
  return state === "queued" || state === "running";
}

/**
 * Picks the more recent of two snapshots of the same task.
 *
 * Events and the initial list can arrive in either order, and workers finish
 * out of order, so a snapshot only wins if it is further along.
 */
export function newerSnapshot(a: TaskSnapshot, b: TaskSnapshot): TaskSnapshot {
  if (a.finishedAt !== null && b.finishedAt === null) return a;
  if (b.finishedAt !== null && a.finishedAt === null) return b;
  return b.done >= a.done ? b : a;
}

/** Lists every background task: live ones first, newest first within each group. */
export function listBackgroundTasks(): Promise<TaskSnapshot[]> {
  return invoke<TaskSnapshot[]>("list_background_tasks");
}

/**
 * Asks a task to stop after the images it is working on. Unfinished images
 * stay queued and are scored on the next launch.
 */
export function cancelBackgroundTask(taskId: string): Promise<void> {
  return invoke<void>("cancel_background_task", { taskId });
}

/** Forgets every finished task and returns the removed ids. */
export function clearFinishedBackgroundTasks(): Promise<string[]> {
  return invoke<string[]>("clear_finished_background_tasks");
}
