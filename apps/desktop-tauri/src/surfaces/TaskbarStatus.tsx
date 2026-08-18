import { useStatusSurface } from "../hooks/useStatusSurface";
import { TaskbarStatusContents } from "./TaskbarStatusContents";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";
import "./TaskbarStatus.css";

export default function TaskbarStatus() {
  const surface = useStatusSurface();
  const presentation = buildTaskbarStatusPresentation(surface);
  const closeFailed = surface.closeFailedBySurface.taskbarStatus;

  const closeSurface = async (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    await surface.disableSurface("taskbarStatus").catch(() => undefined);
  };

  return (
    <div className="taskbar-status-host" data-testid="taskbar-status-content">
      <TaskbarStatusContents
        mode="visible"
        presentation={presentation}
        closeFailed={closeFailed}
        onOpen={() => void surface.openPanel()}
        onClose={closeSurface}
      />
    </div>
  );
}
