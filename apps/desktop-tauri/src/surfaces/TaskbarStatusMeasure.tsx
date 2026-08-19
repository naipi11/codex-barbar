import { useRef } from "react";
import { useStatusSurface } from "../hooks/useStatusSurface";
import { useTheme } from "../hooks/useTheme";
import { useTaskbarStatusWidth } from "../hooks/useTaskbarStatusWidth";
import { TaskbarStatusContents } from "./TaskbarStatusContents";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";
import "./TaskbarStatus.css";

export default function TaskbarStatusMeasure() {
  const surface = useStatusSurface();
  useTheme(surface.bootstrap?.settings.theme ?? "system");
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
