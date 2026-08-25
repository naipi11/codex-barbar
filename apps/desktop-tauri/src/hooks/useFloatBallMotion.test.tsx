import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import { useFloatBallMotion } from "./useFloatBallMotion";

type EventCallback = (event: { payload: unknown }) => void;

const eventHarness = vi.hoisted(() => {
  const listeners = new Map<string, Set<EventCallback>>();
  return {
    listeners,
    listen(eventName: string, callback: EventCallback) {
      const callbacks = listeners.get(eventName) ?? new Set<EventCallback>();
      callbacks.add(callback);
      listeners.set(eventName, callbacks);
      return Promise.resolve(() => callbacks.delete(callback));
    },
    emit(eventName: string, payload: unknown) {
      for (const callback of listeners.get(eventName) ?? []) {
        callback({ payload });
      }
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, callback: EventCallback) =>
    eventHarness.listen(eventName, callback),
}));

describe("useFloatBallMotion", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    eventHarness.listeners.clear();
    invokeMock.mockResolvedValue({ state: "idle", thinking: false, fast: false });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("applies a fast event without recreating the rotation phase", async () => {
    const { result } = renderHook(() => useFloatBallMotion());
    await waitFor(() => expect(result.current.motion).toBe("idle"));
    act(() => {
      eventHarness.emit("codexbar:float-ball-motion-changed", { state: "fast" });
    });

    await waitFor(() => expect(result.current.motion).toBe("fast"));
    expect(result.current.speed).toBe(3);
  });
});

