import { useRef } from "react";
import { useStatusSurface } from "../hooks/useStatusSurface";
import { useTheme } from "../hooks/useTheme";
import { useTaskbarStatusWidth } from "../hooks/useTaskbarStatusWidth";
import { TaskbarStatusContents } from "./TaskbarStatusContents";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";
import "./TaskbarStatus.css";

function SupportedTaskbarStatusMeasure({
  surface,
}: {
  surface: ReturnType<typeof useStatusSurface>;
}) {
  const presentation = buildTaskbarStatusPresentation(surface);
  const measurementRef = useRef<HTMLDivElement>(null);
  useTaskbarStatusWidth(measurementRef);

  return (
    <TaskbarStatusContents
      mode="measurement"
      presentation={presentation}
      measurementRef={measurementRef}
    />
  );
}

export default function TaskbarStatusMeasure() {
  const surface = useStatusSurface();
  useTheme(surface.bootstrap?.settings.theme ?? "system");
  if (!surface.bootstrap?.platform.taskbarStatus) {
    return null;
  }
  return <SupportedTaskbarStatusMeasure surface={surface} />;
}
