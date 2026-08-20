import { useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getFloatBallMotion } from "../lib/tauri";
import { useStatusSurface } from "../hooks/useStatusSurface";
import { useTheme } from "../hooks/useTheme";
import ChatGptMark from "../theme/ChatGptMark";
import "./FloatBall.css";

const DRAG_THRESHOLD = 4;
const IDLE_SECONDS = 2.222;

function clampPercent(value: number | undefined): number {
  return Math.max(0, Math.min(100, value ?? 20));
}

function motionFromSnapshot(snapshot: { thinking: boolean; fast: boolean }): "idle" | "thinking" | "fast" {
  if (snapshot.fast && snapshot.thinking) return "fast";
  if (snapshot.fast) return "fast";
  if (snapshot.thinking) return "thinking";
  return "idle";
}

function detectMotion(): { thinking: boolean; fast: boolean } {
  // Best-effort local probe: any live Codex/ChatGPT process counts as thinking.
  // Fast is inferred from the current Codex config service tier / model name.
  return { thinking: false, fast: false };
}

export default function FloatBall() {
  const surface = useStatusSurface();
  useTheme(surface.bootstrap?.settings.theme ?? "system");
  const closeError = surface.closeFailedBySurface.floatBall ? "关闭失败，请重试" : null;
  const pointerRef = useRef<{ id: number; x: number; y: number; dragged: boolean } | null>(null);
  const skipNextClickRef = useRef(false);
  const [motion, setMotion] = useState<"idle" | "thinking" | "fast">("idle");
  const metric = surface.universalMetric;
  const displayedPercent = metric?.displayedPercent ?? null;
  const language = surface.bootstrap?.settings.language;
  const chinese =
    language === "zh-CN" ||
    (language !== "en-US" && (navigator.language || "").toLowerCase().startsWith("zh"));
  const glow = clampPercent(surface.bootstrap?.settings.floatBallGlow);
  const opacity = clampPercent(surface.bootstrap?.settings.floatBallOpacity);
  const speedSeconds =
    motion === "fast" ? IDLE_SECONDS / 3 : motion === "thinking" ? IDLE_SECONDS / 2 : IDLE_SECONDS;
  const bodyLabel = `${chinese ? "打开完整面板" : "Open panel"}，${surface.displayName}，${
    displayedPercent === null ? (chinese ? "额度不可用" : "quota unavailable") : `${displayedPercent}%`
  }，${motion}`;

  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      try {
        const snapshot = await getFloatBallMotion();
        if (!cancelled) setMotion(motionFromSnapshot(snapshot));
      } catch {
        if (!cancelled) setMotion(motionFromSnapshot(detectMotion()));
      }
    };
    void tick();
    const id = window.setInterval(() => void tick(), 4000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  const startDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    pointerRef.current = { id: event.pointerId, x: event.clientX, y: event.clientY, dragged: false };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  const moveDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId || pointer.dragged) return;
    if (Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) <= DRAG_THRESHOLD) return;
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
    const dragged =
      pointer.dragged || Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) > DRAG_THRESHOLD;
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
      data-motion={motion}
      style={
        {
          "--surface-bg-alpha": String(opacity / 100),
          "--float-glow": String(glow / 80),
          "--float-spin-duration": `${speedSeconds}s`,
        } as CSSProperties
      }
    >
      <button
        type="button"
        className="float-ball__body"
        data-band={metric?.band ?? "unknown"}
        data-status={surface.status}
        data-dragging={surface.isDragging}
        data-motion={motion}
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
        <span className="float-ball__halo" aria-hidden="true" />
        <span className="float-ball__spin" aria-hidden="true">
          <ChatGptMark className="float-ball__blossom" variant="blossom" />
        </span>
      </button>
      {closeError ? (
        <span className="float-ball__error" role="status">
          {closeError}
        </span>
      ) : null}
    </div>
  );
}
