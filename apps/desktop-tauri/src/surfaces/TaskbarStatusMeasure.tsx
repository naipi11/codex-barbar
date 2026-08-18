import { useRef } from "react";
import { useStatusSurface } from "../hooks/useStatusSurface";
import { useTaskbarStatusWidth } from "../hooks/useTaskbarStatusWidth";
import { TaskbarStatusContents } from "./TaskbarStatusContents";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";
import "./TaskbarStatus.css";

export default function TaskbarStatusMeasure() {
  const surface = useStatusSurface();
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
