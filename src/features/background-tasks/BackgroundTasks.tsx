import { useState } from "react";

import { useBackgroundTasks } from "../../hooks/useBackgroundTasks";
import { BackgroundTaskPanel } from "./BackgroundTaskPanel";
import { BackgroundTaskStatusBar } from "./BackgroundTaskStatusBar";
import { summarize } from "./backgroundTaskFormat";
import "./backgroundTasks.css";

/**
 * The status bar plus its panel, wired to the app's background tasks.
 * Mounted once by the application shell.
 */
export function BackgroundTasks() {
  const { tasks, cancel, clearFinished, hasFinished } = useBackgroundTasks();
  const [open, setOpen] = useState(false);

  return (
    <>
      {open && (
        <BackgroundTaskPanel
          tasks={tasks}
          canClear={hasFinished}
          onClose={() => setOpen(false)}
          onCancel={(taskId) => void cancel(taskId)}
          onClearFinished={() => void clearFinished()}
        />
      )}
      <BackgroundTaskStatusBar
        summary={summarize(tasks)}
        open={open}
        onToggle={() => setOpen((value) => !value)}
        onOpen={() => setOpen(true)}
      />
    </>
  );
}
