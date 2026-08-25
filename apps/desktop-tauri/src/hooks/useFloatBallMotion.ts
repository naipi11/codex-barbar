import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { events, getFloatBallMotion } from "../lib/tauri";
import type { FloatBallMotionDto, MotionState } from "../types/bridge";

const RECOVERY_MS = 2000;
const IDLE_SECONDS = 2.222;
const DEGREES_PER_MS = 360 / (IDLE_SECONDS * 1000);

export const MOTION_SPEED: Record<MotionState, number> = {
  idle: 1,
  thinking: 2,
  fast: 3,
};

function parseMotion(value: unknown): MotionState {
  if (typeof value === "string" && value in MOTION_SPEED) {
    return value as MotionState;
  }
  if (value && typeof value === "object") {
    const record = value as { state?: unknown; thinking?: unknown; fast?: unknown };
    if (typeof record.state === "string" && record.state in MOTION_SPEED) {
      return record.state as MotionState;
    }
    if (record.fast === true) return "fast";
    if (record.thinking === true) return "thinking";
  }
  return "idle";
}

export function useFloatBallMotion() {
  const [motion, setMotion] = useState<MotionState>("idle");
  const nodeRef = useRef<HTMLElement | null>(null);
  const phase = useRef(0);
  const lastFrame = useRef<number | null>(null);
  const frame = useRef<number | null>(null);
  const speedRef = useRef(MOTION_SPEED.idle);
  const reducedMotion = useRef(false);

  useEffect(() => {
    let active = true;
    let recovery: ReturnType<typeof setInterval> | undefined;
    let unlisten: (() => void | Promise<void>) | undefined;

    const apply = (next: MotionState) => {
      if (!active) return;
      speedRef.current = MOTION_SPEED[next];
      setMotion(next);
    };

    const startRecovery = () => {
      if (recovery) return;
      recovery = setInterval(() => {
        void getFloatBallMotion()
          .then((snapshot) => apply(parseMotion(snapshot)))
          .catch(() => undefined);
      }, RECOVERY_MS);
    };

    void getFloatBallMotion()
      .then((snapshot) => apply(parseMotion(snapshot)))
      .catch(() => apply("idle"));

    void listen<FloatBallMotionDto>(events.floatBallMotionChanged, (event) => {
      apply(parseMotion(event.payload));
    })
      .then((fn) => {
        if (active) unlisten = fn;
        else void fn();
      })
      .catch(() => {
        if (active) startRecovery();
      });

    return () => {
      active = false;
      if (recovery) clearInterval(recovery);
      if (unlisten) void unlisten();
    };
  }, []);

  useEffect(() => {
    const media = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    const syncReduced = () => {
      reducedMotion.current = Boolean(media?.matches);
    };
    syncReduced();
    media?.addEventListener?.("change", syncReduced);

    const advance = (now: number) => {
      const node = nodeRef.current;
      if (!node) {
        frame.current = requestAnimationFrame(advance);
        return;
      }
      if (reducedMotion.current) {
        node.style.setProperty("--float-rotation-deg", "0");
        lastFrame.current = now;
        frame.current = requestAnimationFrame(advance);
        return;
      }
      const previous = lastFrame.current ?? now;
      const elapsed = Math.max(0, now - previous);
      phase.current = (phase.current + elapsed * speedRef.current * DEGREES_PER_MS) % 360;
      node.style.setProperty("--float-rotation-deg", String(phase.current));
      lastFrame.current = now;
      frame.current = requestAnimationFrame(advance);
    };

    frame.current = requestAnimationFrame(advance);
    return () => {
      media?.removeEventListener?.("change", syncReduced);
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    };
  }, []);

  return {
    motion,
    speed: MOTION_SPEED[motion],
    bindRotation(node: HTMLElement | null) {
      nodeRef.current = node;
    },
  };
}

