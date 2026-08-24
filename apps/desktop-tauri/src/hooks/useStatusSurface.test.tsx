import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import {
  bootstrapWithTwoProfiles,
  readyTwoWindowFixture,
  weeklyOnlyUsage,
} from "../test/profileUsageFixtures";
import { events } from "../lib/tauri";

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

import { profileDisplayName } from "../lib/statusSurfaceViewModel";
import { useStatusSurface } from "./useStatusSurface";

describe("useStatusSurface", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    eventHarness.listeners.clear();
  });

  it("derives the selected account identity and remaining percentage", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "Ming Zhao";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage({
      remainingPercent: 42,
      usedPercent: 58,
    });
    invokeMock.mockResolvedValue(bootstrap);

    const { result } = renderHook(() => useStatusSurface());

    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());
    expect(result.current.displayName).toBe("Ming Zhao");
    expect(result.current.urgentMetric?.displayedPercent).toBe(42);
    expect(result.current.status).toBe("ready");
  });

  it("falls back from display name to account email", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = null;
    bootstrap.profiles[0]!.accountEmail = "ming@example.com";
    invokeMock.mockResolvedValue(bootstrap);

    const { result } = renderHook(() => useStatusSurface());

    await waitFor(() => expect(result.current.displayName).toBe("ming@example.com"));
  });

  it("distinguishes a signed-in account with no displayable identity", () => {
    const bootstrap = bootstrapWithTwoProfiles();
    const profile = bootstrap.profiles[0]!;
    profile.accountDisplayName = null;
    profile.accountEmail = null;
    profile.accountStatus = "signedIn";

    expect(profileDisplayName(profile)).toBe("已登录（名称不可用）");
  });

  it("uses explicit signed-out and unavailable identity states", () => {
    const bootstrap = bootstrapWithTwoProfiles();
    const profile = bootstrap.profiles[0]!;

    profile.accountStatus = "signedOut";
    expect(profileDisplayName(profile)).toBe("未登录");

    profile.accountStatus = "unavailable";
    expect(profileDisplayName(profile)).toBe("账号信息不可用");
  });

  it.each([
    [74, "ready"],
    [75, "warning"],
    [90, "critical"],
  ])("uses exact used-percent threshold %s", async (usedPercent, expected) => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = readyTwoWindowFixture().usageByProfile.personal;
    bootstrap.usageByProfile.personal!.secondary!.usedPercent = usedPercent;
    bootstrap.usageByProfile.personal!.secondary!.remainingPercent = 100 - usedPercent;
    invokeMock.mockResolvedValue(bootstrap);

    const { result } = renderHook(() => useStatusSurface());

    await waitFor(() => expect(result.current.status).toBe(expected));
  });

  it("prioritizes refreshing, stale, error, and missing states", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = {
      ...weeklyOnlyUsage({ remainingPercent: 42, usedPercent: 58 }),
      refreshStatus: "refreshing",
    };
    invokeMock.mockResolvedValue(bootstrap);
    const { result, rerender } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(result.current.status).toBe("refreshing"));

    act(() => {
      eventHarness.emit(events.profileUsageStateChanged, {
        ...bootstrap.usageByProfile.personal,
        refreshStatus: "idle",
        freshness: "stale",
      });
    });
    expect(result.current.status).toBe("stale");

    act(() => {
      eventHarness.emit(events.profileUsageStateChanged, {
        ...bootstrap.usageByProfile.personal,
        refreshStatus: "idle",
        freshness: "fresh",
        currentError: {
          kind: "offlineOrTimeout",
          userMessageKey: "errors.offlineOrTimeout",
          action: "retry",
          retryAfter: null,
        },
      });
    });
    expect(result.current.status).toBe("critical");

    rerender();
    act(() => {
      eventHarness.emit(events.profileUsageStateChanged, {
        ...bootstrap.usageByProfile.personal,
        primary: null,
        secondary: null,
        currentError: null,
        freshness: "missing",
        refreshStatus: "idle",
      });
    });
    expect(result.current.status).toBe("missing");
  });

  it("adopts authoritative settings events without reloading bootstrap", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockResolvedValue(bootstrap);
    const { result } = renderHook(() => useStatusSurface());

    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());
    const settings = {
      ...bootstrap.settings,
      displayMode: "used" as const,
      taskbarTransparencyPercent: 35,
      floatBallTransparencyPercent: 60,
    };
    act(() => eventHarness.emit(events.settingsChanged, settings));

    await waitFor(() => {
      expect(result.current.bootstrap?.settings).toEqual(settings);
      expect(result.current.urgentMetric?.displayMode).toBe("used");
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("keeps settings emitted before deferred bootstrap resolution", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    let resolveBootstrap: (value: typeof bootstrap) => void;
    invokeMock.mockReturnValue(
      new Promise<typeof bootstrap>((resolve) => {
        resolveBootstrap = resolve;
      }),
    );
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(eventHarness.listeners.get(events.settingsChanged)?.size).toBeGreaterThan(1));

    const settings = {
      ...bootstrap.settings,
      displayMode: "used" as const,
      taskbarTransparencyPercent: 35,
      floatBallTransparencyPercent: 60,
    };
    act(() => eventHarness.emit(events.settingsChanged, settings));
    await act(async () => resolveBootstrap!(bootstrap));

    await waitFor(() => {
      expect(result.current.bootstrap?.settings).toEqual(settings);
      expect(result.current.urgentMetric?.displayMode).toBe("used");
    });
  });

  it("restores both close feedback values from bootstrap", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.statusSurfaceFeedback = {
      taskbarStatusCloseFailed: true,
      floatBallCloseFailed: false,
    };
    invokeMock.mockResolvedValue(bootstrap);
    const { result } = renderHook(() => useStatusSurface());

    await waitFor(() =>
      expect(result.current.closeFailedBySurface).toEqual({
        taskbarStatus: true,
        floatBall: false,
      }),
    );
  });

  it("applies feedback events only to their target surface", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockResolvedValue(bootstrap);
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

    act(() =>
      eventHarness.emit(events.statusSurfaceFeedbackChanged, {
        surface: "floatBall",
        closeFailed: true,
      }),
    );

    expect(result.current.closeFailedBySurface).toEqual({
      taskbarStatus: false,
      floatBall: true,
    });
  });

  it("keeps early feedback for one surface while bootstrap supplies its peer", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.statusSurfaceFeedback.floatBallCloseFailed = true;
    let resolveBootstrap: (value: typeof bootstrap) => void;
    invokeMock.mockReturnValue(
      new Promise<typeof bootstrap>((resolve) => {
        resolveBootstrap = resolve;
      }),
    );
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() =>
      expect(
        eventHarness.listeners.get(events.statusSurfaceFeedbackChanged)?.size,
      ).toBeGreaterThan(0),
    );

    act(() =>
      eventHarness.emit(events.statusSurfaceFeedbackChanged, {
        surface: "taskbarStatus",
        closeFailed: true,
      }),
    );
    await act(async () => resolveBootstrap!(bootstrap));

    await waitFor(() =>
      expect(result.current.closeFailedBySurface).toEqual({
        taskbarStatus: true,
        floatBall: true,
      }),
    );
  });

  it("clears only the retry target and restores it when the command rejects", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.statusSurfaceFeedback = {
      taskbarStatusCloseFailed: true,
      floatBallCloseFailed: true,
    };
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_bootstrap_state") return bootstrap;
      if (command === "set_status_surface_enabled") {
        throw new Error("STATUS_SURFACE_SETTINGS_SAVE_FAILED");
      }
      return undefined;
    });
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

    await act(async () => {
      await expect(
        result.current.disableSurface("taskbarStatus"),
      ).rejects.toThrow("STATUS_SURFACE_SETTINGS_SAVE_FAILED");
    });

    expect(result.current.closeFailedBySurface).toEqual({
      taskbarStatus: true,
      floatBall: true,
    });
  });

  it("clears only the pending retry target", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.statusSurfaceFeedback = {
      taskbarStatusCloseFailed: true,
      floatBallCloseFailed: true,
    };
    let resolveDisable: () => void;
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_bootstrap_state") return Promise.resolve(bootstrap);
      if (command === "set_status_surface_enabled") {
        return new Promise<void>((resolve) => {
          resolveDisable = resolve;
        });
      }
      return Promise.resolve(undefined);
    });
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

    let disable: Promise<unknown>;
    act(() => {
      disable = result.current.disableSurface("taskbarStatus");
    });

    expect(result.current.closeFailedBySurface).toEqual({
      taskbarStatus: false,
      floatBall: true,
    });

    await act(async () => resolveDisable!());
    await disable!;
  });

  it("opens the tray panel through the fixed command", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockResolvedValue(bootstrap);
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

    await act(async () => {
      await result.current.openPanel();
    });
    expect(invokeMock).toHaveBeenCalledWith("open_tray_panel");
  });

  it("disables a status surface through the typed command", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockResolvedValue(bootstrap);
    const { result } = renderHook(() => useStatusSurface());
    await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

    await act(async () => {
      await result.current.disableSurface("floatBall");
    });
    expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", {
      surface: "floatBall",
      enabled: false,
    });
  });
});
