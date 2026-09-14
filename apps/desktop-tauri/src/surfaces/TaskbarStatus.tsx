import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useRef, type PointerEvent as ReactPointerEvent } from "react";
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
  useNative: boolean;
  origin: { x: number; y: number } | null;
  scaleFactor: number;
  latest: { x: number; y: number } | null;
  failed: boolean;
  ready: Promise<void>;
  movePromise: Promise<void> | null;
  draggingReady: Promise<void> | null;
  nativeDragReady: Promise<void> | null;
};

export default function TaskbarStatus() {
  const surface = useStatusSurface();
  const pointerRef = useRef<DragState | null>(null);
  const skipNextClickRef = useRef(false);
  useTheme(surface.bootstrap?.settings.theme ?? "system");
  const presentation = buildTaskbarStatusPresentation(surface);
  const closeFailed = surface.closeFailedBySurface.taskbarStatus;

  const queueMove = (pointer: DragState) => {
    if (pointer.failed || pointer.movePromise || !pointer.latest) return;
    pointer.movePromise = (async () => {
      try {
        await pointer.ready;
        await pointer.draggingReady?.catch(() => undefined);
        while (pointer.latest && pointer.origin) {
          const latest = pointer.latest;
          pointer.latest = null;
          const window = getCurrentWindow();
          const x =
            pointer.origin.x +
            Math.round((latest.x - pointer.x) * pointer.scaleFactor);
          const y =
            pointer.origin.y +
            Math.round((latest.y - pointer.y) * pointer.scaleFactor);
          await window.setPosition(new PhysicalPosition(x, y));
        }
      } catch {
        pointer.failed = true;
        pointer.latest = null;
      }
    })().finally(() => {
      pointer.movePromise = null;
      if (pointer.latest && !pointer.failed) queueMove(pointer);
    });
  };

  const finishBackendDrag = async (pointer: DragState) => {
    while (pointer.movePromise || pointer.latest) {
      if (!pointer.movePromise && pointer.latest) queueMove(pointer);
      if (pointer.movePromise) await pointer.movePromise;
    }
    await pointer.ready.catch(() => undefined);
    await pointer.draggingReady?.catch(() => undefined);
    await pointer.nativeDragReady?.catch(() => undefined);
    await setTaskbarStatusDragging(false).catch(() => undefined);
  };

  const startDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const window = getCurrentWindow();
    const useNative = window.label === "taskbar-status";
    void (window.setIgnoreCursorEvents?.(false) ?? Promise.resolve()).catch(
      () => undefined,
    );
    const pointer = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      dragged: false,
      useNative,
      origin: null,
      scaleFactor: 1,
      latest: null,
      failed: false,
      ready: Promise.resolve(),
      movePromise: null,
      draggingReady: null,
      nativeDragReady: null,
    } as DragState;
    pointer.ready = Promise.all([window.outerPosition(), window.scaleFactor()]).then(
      ([position, scaleFactor]) => {
        pointer.origin = position;
        pointer.scaleFactor = scaleFactor;
      },
    );
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
    pointer.draggingReady = setTaskbarStatusDragging(true).catch(
      () => undefined,
    );
    surface.setIsDragging(true);
    if (pointer.useNative) {
      pointer.nativeDragReady = pointer.draggingReady
        .then(() => getCurrentWindow().startDragging())
        .catch(() => undefined);
    } else {
      pointer.nativeDragReady = pointer.draggingReady
        .then(() => startTaskbarStatusDragging())
        .catch(() => undefined);
    }
    pointer.latest = { x: event.clientX, y: event.clientY };
    queueMove(pointer);
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
    if (dragged) void finishBackendDrag(pointer);
  };

  const cancelDrag = () => {
    const pointer = pointerRef.current;
    pointerRef.current = null;
    surface.setIsDragging(false);
    skipNextClickRef.current = true;
    if (pointer?.dragged) void finishBackendDrag(pointer);
  };

  const openPanel = () => {
    if (skipNextClickRef.current) {
      skipNextClickRef.current = false;
      return;
    }
    void surface.openPanel();
  };

  return (
    <div className="taskbar-status-host" data-testid="taskbar-status-content">
      <TaskbarStatusContents
        mode="visible"
        presentation={presentation}
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
