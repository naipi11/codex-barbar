import { useRef } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStatusSurface } from "../hooks/useStatusSurface";
import "./FloatBall.css";

const RADIUS = 27;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
const DRAG_THRESHOLD = 4;

function surfaceAlpha(opacity: number | undefined): string {
  return String(Math.max(0, Math.min(100, opacity ?? 20)) / 100);
}

function trustText(trustState: ReturnType<typeof useStatusSurface>["trustState"], refreshStatus: string): string {
  if (trustState === "cached") return "缓存数据";
  if (trustState === "missing") return "额度不可用";
  return refreshStatus;
}

export default function FloatBall() {
  const surface = useStatusSurface();
  const closeError = surface.closeFailedBySurface.floatBall
    ? "关闭失败，请重试"
    : null;
  const pointerRef = useRef<{
    id: number;
    x: number;
    y: number;
    dragged: boolean;
  } | null>(null);
  const skipNextClickRef = useRef(false);
  const metric = surface.urgentMetric;
  const displayedPercent = metric?.displayedPercent ?? null;
  const percent = displayedPercent ?? 0;
  const dashOffset = CIRCUMFERENCE * (1 - percent / 100);
  const footer = metric ? `${metric.shortLabel} 剩余` : "额度剩余";
  const confidence = trustText(surface.trustState, surface.refreshStatus);
  const updated = surface.updatedText ? `，${surface.updatedText}` : "";
  const bodyLabel = `打开完整面板，${surface.displayName}，${
    displayedPercent === null
      ? "额度不可用"
      : `${displayedPercent}% ${metric?.displayMode ?? "remaining"}`
  }，${confidence}${updated}，${surface.status}${
    surface.status === "missing" ? " unavailable" : ""
  }`;

  const startDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    pointerRef.current = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      dragged: false,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  const moveDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId || pointer.dragged) return;
    const distance = Math.hypot(
      event.clientX - pointer.x,
      event.clientY - pointer.y,
    );
    if (distance <= DRAG_THRESHOLD) return;

    pointer.dragged = true;
    void (async () => {
      surface.setIsDragging(true);
      await getCurrentWindow().startDragging().catch(() => undefined);
    })();
  };

  const finishPointer = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId) return;
    pointerRef.current = null;
    const movedDistance = Math.hypot(
      event.clientX - pointer.x,
      event.clientY - pointer.y,
    );
    const dragged = pointer.dragged || movedDistance > DRAG_THRESHOLD;
    surface.setIsDragging(false);
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    skipNextClickRef.current = true;
    if (!dragged) void surface.openPanel();
  };


  return (
    <div
      className={`float-ball-shell float-ball--${surface.status} float-ball--collapsed${
        surface.isDragging ? " float-ball--dragging" : ""
      }`}
      data-testid="float-ball-shell"
      data-status={surface.status}
      data-dragging={surface.isDragging}
      data-trust={surface.trustState}
      style={{
        "--surface-bg-alpha": surfaceAlpha(surface.bootstrap?.settings.floatBallOpacity),
      } as CSSProperties}
    >
      <button
        type="button"
        className="float-ball__body"
        data-band={metric?.band ?? "unknown"}
        data-status={surface.status}
        data-dragging={surface.isDragging}
        aria-label={bodyLabel}
        title={bodyLabel}
        onPointerDown={startDrag}
        onPointerMove={moveDrag}
        onPointerUp={finishPointer}
        onPointerCancel={(event) => {
          const pointer = pointerRef.current;
          if (!pointer || pointer.id !== event.pointerId) return;
          pointerRef.current = null;
          surface.setIsDragging(false);
        }}
        onClick={() => {
          if (skipNextClickRef.current) {
            skipNextClickRef.current = false;
            return;
          }
          void surface.openPanel();
        }}
      >
        <div className="float-ball__orbit" data-band={metric?.band ?? "unknown"}>

          <svg className="float-ball__ring" viewBox="0 0 64 64" aria-hidden="true">
            <circle className="float-ball__track" cx="32" cy="32" r={RADIUS} />
            <circle
              className="float-ball__progress"
              data-testid="float-ball-ring-progress"
              data-band={metric?.band ?? "unknown"}
              cx="32"
              cy="32"
              r={RADIUS}
              strokeDasharray={CIRCUMFERENCE}
              strokeDashoffset={dashOffset}
              opacity={percent > 0 ? 1 : 0}
            />

          </svg>
          <span className="float-ball__value" aria-hidden="true">
            {displayedPercent === null ? "—" : displayedPercent}
          </span>
          <span className="float-ball__footer" aria-hidden="true">{footer}</span>
        </div>
      </button>

      {closeError && (
        <span className="float-ball__error" role="status">
          {closeError}
        </span>
      )}
    </div>
  );
}