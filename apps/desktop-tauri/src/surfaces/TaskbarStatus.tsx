import { useStatusSurface } from "../hooks/useStatusSurface";
import { useTheme } from "../hooks/useTheme";
import { TaskbarStatusContents } from "./TaskbarStatusContents";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";
import "./TaskbarStatus.css";

export default function TaskbarStatus() {
  const surface = useStatusSurface();
  useTheme(surface.bootstrap?.settings.theme ?? "system");
  if (surface.bootstrap && !surface.bootstrap.platform.taskbarStatus) {
    return null;
  }
  const presentation = buildTaskbarStatusPresentation(surface);
  const closeFailed = surface.closeFailedBySurface.taskbarStatus;

  return (
    <div className="taskbar-status-host" data-testid="taskbar-status-content">
      <TaskbarStatusContents
        mode="visible"
        presentation={presentation}
        closeFailed={closeFailed}
        onOpen={() => void surface.openPanel()}
      />
    </div>
  );
}
