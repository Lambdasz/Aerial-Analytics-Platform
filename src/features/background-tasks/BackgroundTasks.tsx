import { useBackgroundTasks } from "../../hooks/useBackgroundTasks";
import { BackgroundTaskPanel } from "./BackgroundTaskPanel";
import "./backgroundTasks.css";

/**
 * Background task section for the sidebar: lists every task inline.
 * Rendered by the Sidebar, below the module menu.
 */
export function BackgroundTasks() {
  const { tasks, cancel, clearFinished, hasFinished } = useBackgroundTasks();

  return (
    <BackgroundTaskPanel
      tasks={tasks}
      canClear={hasFinished}
      onCancel={(taskId) => void cancel(taskId)}
      onClearFinished={() => void clearFinished()}
    />
  );
}
