import type React from "react";
import type { CSSProperties } from "react";
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
  onClose?(event: React.MouseEvent<HTMLButtonElement>): void;
  measurementRef?: React.Ref<HTMLDivElement>;
}

function initials(name: string): string {
  return Array.from(name.trim())[0]?.toUpperCase() ?? "C";
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
  onClose,
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
          {initials(displayName)}
          <span className="taskbar-status__state-dot" />
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
      <button
        type="button"
        className="taskbar-status__close"
        data-error={visible && closeFailed ? "true" : undefined}
        aria-label={visible ? "关闭任务栏状态" : undefined}
        title={visible && closeFailed ? "关闭失败，点击重试" : "关闭任务栏状态"}
        tabIndex={visible ? undefined : -1}
        onClick={visible ? onClose : undefined}
      >
        <span aria-hidden="true">×</span>
      </button>
    </div>
  );
}
