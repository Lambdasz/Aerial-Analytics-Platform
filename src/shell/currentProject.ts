import { createContext, useContext } from "react";

import type { Project } from "../modules/module-01/projectCommands";
import { isValidUuid } from "../modules/module-01/sessionStats";

export const ACTIVE_PROJECT_STORAGE_KEY = "m1_last_project_id";

export interface CurrentProjectContextValue {
  /** The currently selected project across the app, or `null` when none is selected. */
  readonly currentProject: Project | null;
  /** Convenience accessor for `currentProject?.id ?? null`. */
  readonly currentProjectId: string | null;
  /** Update the globally active project without navigating away from the current page. */
  readonly setCurrentProject: (project: Project | null) => void;
  /** Clear the globally active project. */
  readonly clearCurrentProject: () => void;
}

export const CurrentProjectContext = createContext<CurrentProjectContextValue | null>(null);

export function readStoredProject(): Project | null {
  try {
    const raw = window.localStorage.getItem(ACTIVE_PROJECT_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<Project>;
    if (
      parsed &&
      typeof parsed.id === "string" &&
      isValidUuid(parsed.id) &&
      typeof parsed.name === "string"
    ) {
      return parsed as Project;
    }
  } catch {
    // Ignore storage read/parse errors
  }
  return null;
}

export function writeStoredProject(project: Project | null): void {
  try {
    if (project) {
      window.localStorage.setItem(ACTIVE_PROJECT_STORAGE_KEY, JSON.stringify(project));
    } else {
      window.localStorage.removeItem(ACTIVE_PROJECT_STORAGE_KEY);
    }
  } catch {
    // Ignore storage write errors (private mode / quota)
  }
}

/**
 * Access the globally selected project state.
 *
 * Any module rendered inside `AppShell` can call this hook to read
 * `currentProject` / `currentProjectId` or update the active project.
 */
export function useCurrentProject(): CurrentProjectContextValue {
  const ctx = useContext(CurrentProjectContext);
  if (!ctx) {
    throw new Error("useCurrentProject must be used within a CurrentProjectProvider");
  }
  return ctx;
}
