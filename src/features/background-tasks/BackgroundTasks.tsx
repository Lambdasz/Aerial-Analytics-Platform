import { useBackgroundTasks } from "../../hooks/useBackgroundTasks";
import { BackgroundTaskPanel } from "./BackgroundTaskPanel";
import "./backgroundTasks.css";

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
