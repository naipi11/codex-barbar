import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup, configure } from "@testing-library/react";

// Parallel workers share CPU, so bootstrap promises and React updates can be
// delayed past Testing Library default 1s wait. Keep assertions semantic;
// only widen the async wait window so load spikes do not make tests flaky.
configure({ asyncUtilTimeout: 5000 });

if (typeof window !== "undefined" && !window.PointerEvent) {
  class TestPointerEvent extends MouseEvent {
    readonly pointerId: number;
    readonly isPrimary: boolean;

    constructor(type: string, init: PointerEventInit = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 0;
      this.isPrimary = init.isPrimary ?? true;
    }
  }

  Object.defineProperty(window, "PointerEvent", {
    configurable: true,
    value: TestPointerEvent,
  });
}

export const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

afterEach(() => {
  cleanup();
  invokeMock.mockReset();
});
