import { useCallback, useMemo, useState, type ReactNode } from "react";

import type { Project } from "../modules/module-01/projectCommands";
import {
  CurrentProjectContext,
  type CurrentProjectContextValue,
  readStoredProject,
  writeStoredProject,
} from "./currentProject";

export function CurrentProjectProvider({ children }: { children: ReactNode }) {
  const [currentProject, setCurrentProjectState] = useState<Project | null>(() =>
    readStoredProject(),
  );

  const setCurrentProject = useCallback((project: Project | null) => {
    writeStoredProject(project);
    setCurrentProjectState(project);
  }, []);

  const clearCurrentProject = useCallback(() => {
    writeStoredProject(null);
    setCurrentProjectState(null);
  }, []);

  const value = useMemo<CurrentProjectContextValue>(
    () => ({
      currentProject,
      currentProjectId: currentProject?.id ?? null,
      setCurrentProject,
      clearCurrentProject,
    }),
    [currentProject, setCurrentProject, clearCurrentProject],
  );

  return <CurrentProjectContext.Provider value={value}>{children}</CurrentProjectContext.Provider>;
}
