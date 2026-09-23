import { getCurrentWindow } from "@tauri-apps/api/window";
import { useLayoutEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";
import { useStatusSurface } from "../hooks/useStatusSurface";
import {
  setTaskbarStatusDragging,
  startTaskbarStatusDragging,
} from "../lib/tauri";
import { useTheme } from "../hooks/useTheme";
import { TaskbarStatusContents } from "./TaskbarStatusContents";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";
import "./TaskbarStatus.css";

const DRAG_THRESHOLD = 4;

type DragState = {
  id: number;
  x: number;
  y: number;
  dragged: boolean;
};

export default function TaskbarStatus() {
  const surface = useStatusSurface();
  const pointerRef = useRef<DragState | null>(null);
  const skipNextClickRef = useRef(false);
  const hostRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  useTheme(surface.bootstrap?.settings.theme ?? "system");
  const presentation = buildTaskbarStatusPresentation(surface);
  const closeFailed = surface.closeFailedBySurface.taskbarStatus;

  useLayoutEffect(() => {
    const host = hostRef.current;
    const content = contentRef.current;
    if (!host || !content || typeof ResizeObserver === "undefined") return;

    const fit = () => {
      // Both boxes are CSS pixels: WebView DPI scaling is already applied.
      // Transform the whole intrinsic capsule, never shrink individual text tracks.
      const width = content.offsetWidth;
      const height = content.offsetHeight;
      if (!width || !height) return;
      const scale = Math.min(1, host.clientWidth / width, host.clientHeight / height);
      content.style.setProperty("--taskbar-fit-scale", String(scale));
    };
    const observer = new ResizeObserver(fit);
    observer.observe(host);
    observer.observe(content);
    fit();
    return () => observer.disconnect();
  }, []);

  const startDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const window = getCurrentWindow();
    // The native command owns movement and resolves only after the Win32 drag
    // loop ends; JS coordinates must not race it across monitor DPI changes.
    void (window.setIgnoreCursorEvents?.(false) ?? Promise.resolve()).catch(
      () => undefined,
    );
    const pointer = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      dragged: false,
    } as DragState;
    pointerRef.current = pointer;
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  const moveDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId || pointer.dragged) return;
    if (
      Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) <=
      DRAG_THRESHOLD
    ) {
      return;
    }
    pointer.dragged = true;
    surface.setIsDragging(true);
    void setTaskbarStatusDragging(true)
      .then(() => startTaskbarStatusDragging())
      .catch(() => undefined)
      .finally(async () => {
        // Native dragging can consume pointerup/cancel; always release the
        // backend reconciliation guard when its modal move loop finishes.
        await setTaskbarStatusDragging(false).catch(() => undefined);
        if (pointerRef.current === pointer) {
          pointerRef.current = null;
          surface.setIsDragging(false);
          skipNextClickRef.current = true;
        }
      });
  };

  const finishDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const pointer = pointerRef.current;
    if (!pointer || pointer.id !== event.pointerId) return;
    const dragged =
      pointer.dragged ||
      Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) >
        DRAG_THRESHOLD;
    pointerRef.current = null;
    surface.setIsDragging(false);
    skipNextClickRef.current = dragged;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const cancelDrag = () => {
    pointerRef.current = null;
    surface.setIsDragging(false);
    skipNextClickRef.current = true;
  };

  const openPanel = () => {
    if (skipNextClickRef.current) {
      skipNextClickRef.current = false;
      return;
    }
    void surface.openPanel();
  };

  return (
    <div ref={hostRef} className="taskbar-status-host" data-testid="taskbar-status-content">
      <TaskbarStatusContents
        mode="visible"
        presentation={presentation}
        contentRef={contentRef}
        closeFailed={closeFailed}
        onOpen={openPanel}
        onPointerDown={startDrag}
        onPointerMove={moveDrag}
        onPointerUp={finishDrag}
        onPointerCancel={cancelDrag}
      />
    </div>
  );
}
