import { useCallback, useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  BACKGROUND_TASK_EVENT,
  cancelBackgroundTask,
  clearFinishedBackgroundTasks,
  isLive,
  listBackgroundTasks,
  newerSnapshot,
  type TaskSnapshot,
} from "../lib/backgroundTasks";

/** Inserts or replaces `incoming` by id, keeping the further-along snapshot. */
function upsert(tasks: TaskSnapshot[], incoming: TaskSnapshot): TaskSnapshot[] {
  const index = tasks.findIndex((task) => task.id === incoming.id);
  if (index === -1) return [incoming, ...tasks];
  const next = tasks.slice();
  next[index] = newerSnapshot(tasks[index], incoming);
  return next;
}

/**
 * Live view of the app's background tasks.
 *
 * Subscribes to `background-task-updated` first and only then lists the
 * current tasks, so nothing that changes in between is missed. Outside the
 * desktop app (plain `npm run dev` in a browser) the Tauri bridge is absent;
 * the hook then reports no tasks instead of throwing.
 */
export function useBackgroundTasks() {
  const [tasks, setTasks] = useState<TaskSnapshot[]>([]);

  useEffect(() => {
    let alive = true;
    let unlisten: UnlistenFn | undefined;

    void (async () => {
      const stop = await listen<TaskSnapshot>(BACKGROUND_TASK_EVENT, (event) => {
        if (alive) setTasks((current) => upsert(current, event.payload));
      });
      if (alive) unlisten = stop;
      else stop();

      const initial = await listBackgroundTasks();
      if (alive) setTasks((current) => initial.reduce(upsert, current));
    })().catch((error: unknown) => {
      console.warn("background tasks unavailable:", error);
    });

    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const cancel = useCallback(async (taskId: string) => {
    try {
      await cancelBackgroundTask(taskId);
    } catch (error) {
      console.warn("could not cancel the task:", error);
    }
  }, []);

  const clearFinished = useCallback(async () => {
    try {
      const removed = new Set(await clearFinishedBackgroundTasks());
      setTasks((current) => current.filter((task) => !removed.has(task.id)));
    } catch (error) {
      console.warn("could not clear finished tasks:", error);
    }
  }, []);

  const hasFinished = tasks.some((task) => !isLive(task.state));

  return { tasks, cancel, clearFinished, hasFinished };
}
