import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { resolveTheme, useTheme } from "./useTheme";

type ChangeListener = (event: { matches: boolean }) => void;

function installMedia(initialMatches: boolean) {
  const listeners = new Set<ChangeListener>();
  const media = {
    matches: initialMatches,
    media: "(prefers-color-scheme: dark)",
    onchange: null,
    addEventListener: (_: string, listener: ChangeListener) => {
      listeners.add(listener);
    },
    removeEventListener: (_: string, listener: ChangeListener) => {
      listeners.delete(listener);
    },
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  };
  const matchMedia = vi.fn(() => media);
  vi.stubGlobal("matchMedia", matchMedia);
  return {
    emit(matches: boolean) {
      media.matches = matches;
      for (const listener of listeners) listener({ matches });
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.theme;
});

describe("resolveTheme", () => {
  it("honors explicit light and dark preferences", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("follows the system signal for the system preference", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });
});

describe("useTheme", () => {
  it("sets data-theme and follows system changes", () => {
    const media = installMedia(true);
    const { result } = renderHook(() => useTheme("system"));
    expect(result.current).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");

    act(() => media.emit(false));
    expect(result.current).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("ignores system changes for an explicit preference", () => {
    const media = installMedia(true);
    const { result } = renderHook(() => useTheme("light"));
    expect(result.current).toBe("light");

    act(() => media.emit(false));
    expect(result.current).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
