import type React from "react";
import type { CSSProperties } from "react";
import ChatGptMark from "../theme/ChatGptMark";
import {
  compactTaskbarMetric,
  type TaskbarStatusPresentation,
} from "./taskbarStatusPresentation";

export type TaskbarStatusContentsMode = "visible" | "measurement";

export interface TaskbarStatusContentsProps {
  mode: TaskbarStatusContentsMode;
  presentation: TaskbarStatusPresentation;
  closeFailed?: boolean;
  onOpen?(): void;
  measurementRef?: React.Ref<HTMLDivElement>;
}

function resetDate(metric: TaskbarStatusPresentation["reset"]): string {
  if (!metric?.resetsAt) return "—";
  const date = new Date(metric.resetsAt);
  return Number.isNaN(date.valueOf()) ? "—" : `${date.getMonth() + 1}/${date.getDate()}`;
}

export function TaskbarStatusContents({
  mode,
  presentation,
  closeFailed = false,
  onOpen,
  measurementRef,
}: TaskbarStatusContentsProps): JSX.Element {
  const visible = mode === "visible";
  const inertProps = visible ? {} : ({ inert: "" } as Record<string, string>);
  const {
    ariaLabel,
    compactIdentity,
    displayName,
    metrics,
    reset,
    trustState,
  } = presentation;

  return (
    <div
      {...inertProps}
      ref={visible ? undefined : measurementRef}
      className={`taskbar-status taskbar-status--${mode}`}
      data-testid={`taskbar-status-${mode}`}
      data-trust={visible ? trustState : undefined}
      aria-hidden={visible ? undefined : "true"}
      style={{
        "--surface-bg-alpha": presentation.surfaceAlpha,
      } as CSSProperties}
    >
      {visible ? (
        <span className="taskbar-status__live" role="status" aria-live="polite">
          {closeFailed ? "关闭失败，点击重试" : ""}
        </span>
      ) : null}
      <button
        type="button"
        className="taskbar-status__main"
        aria-label={visible ? ariaLabel : undefined}
        title={displayName}
        tabIndex={visible ? undefined : -1}
        onClick={visible && onOpen ? () => onOpen() : undefined}
      >
        <span className="taskbar-status__avatar" aria-hidden="true">
          <ChatGptMark className="taskbar-status__avatar-mark" />
        </span>
        <span className="taskbar-status__identity">{compactIdentity}</span>
        <span
          className="taskbar-status__quota-track"
          data-testid={visible ? "taskbar-status-quota-track" : undefined}
        >
          {metrics.map((metric, index) => (
            <span
              key={`${index}:${metric.limitId}:${metric.shortLabel}:${metric.resetsAt ?? ""}`}
              className="taskbar-status__metric"
              data-testid={visible ? "taskbar-status-metric" : undefined}
              data-band={metric.band}
              title={`${compactTaskbarMetric(metric)}；${metric.resetText}`}
            >
              {compactTaskbarMetric(metric)}
            </span>
          ))}
        </span>
        <span
          className="taskbar-status__reset"
          data-testid={visible ? "taskbar-status-reset" : undefined}
          title={reset?.resetText ?? "无重置时间"}
        >
          {resetDate(reset)}
        </span>
      </button>
    </div>
  );
}
