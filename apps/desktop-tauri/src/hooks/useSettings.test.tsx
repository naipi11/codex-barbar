import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { events } from "../lib/tauri";
import { invokeMock } from "../test/setup";
import type { AppSettingsDto } from "../types/bridge";
import { defaultAppSettings, useSettings } from "./useSettings";

type SettingsEventCallback = (event: { payload: AppSettingsDto }) => void;

const eventHarness = vi.hoisted(() => {
  let callback: SettingsEventCallback | null = null;
  return {
    listen(eventName: string, next: SettingsEventCallback) {
      if (eventName === "settings-changed") callback = next;
      return Promise.resolve(() => {
        callback = null;
      });
    },
    emit(payload: AppSettingsDto) {
      callback?.({ payload });
    },
    reset() {
      callback = null;
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, callback: SettingsEventCallback) =>
    eventHarness.listen(eventName, callback),
}));

describe("useSettings", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    eventHarness.reset();
  });

  it("defaults taskbar disabled, float ball enabled, and notifications disabled when loading fails", async () => {
    invokeMock.mockRejectedValue(new Error("settings unavailable"));

    const { result } = renderHook(() => useSettings());

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_settings_snapshot");
    });
    expect(result.current.settings.taskbarStatusEnabled).toBe(false);
    expect(result.current.settings.floatBallEnabled).toBe(true);
    expect(result.current.settings.autostartEnabled).toBe(true);
    expect(result.current.settings.taskbarTransparencyPercent).toBe(20);
    expect(result.current.settings.floatBallTransparencyPercent).toBe(20);
    expect(result.current.settings.notifications).toEqual({
      enabled: false,
      playSound: true,
      warningEnabled: true,
      dangerEnabled: true,
      weeklyResetEnabled: true,
      resetCreditIncreaseEnabled: true,
      refreshFailureEnabled: true,
      updateAvailableEnabled: true,
      warningRemainingPercent: 66,
      dangerRemainingPercent: 33,
    });
  });

  it("refreshes both flags and opacities from the settings-changed event", async () => {
    invokeMock.mockResolvedValue(defaultAppSettings);
    const { result } = renderHook(() => useSettings());

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_settings_snapshot");
    });

    act(() => {
      eventHarness.emit({
        ...defaultAppSettings,
        taskbarStatusEnabled: true,
        floatBallEnabled: true,
        taskbarTransparencyPercent: 35,
        floatBallTransparencyPercent: 60,
      });
    });

    expect(events.settingsChanged).toBe("settings-changed");
    expect(result.current.settings.taskbarStatusEnabled).toBe(true);
    expect(result.current.settings.floatBallEnabled).toBe(true);
    expect(result.current.settings.taskbarTransparencyPercent).toBe(35);
    expect(result.current.settings.floatBallTransparencyPercent).toBe(60);
  });

  it("updates hook state from a typed surface toggle response", async () => {
    const next = {
      ...defaultAppSettings,
      taskbarStatusEnabled: true,
    };
    invokeMock
      .mockResolvedValueOnce(defaultAppSettings)
      .mockResolvedValueOnce(next);

    const { result } = renderHook(() => useSettings());

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_settings_snapshot");
    });

    let returned: AppSettingsDto | undefined;
    await act(async () => {
      returned = await result.current.setSurfaceEnabled("taskbarStatus", true);
    });

    expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", {
      surface: "taskbarStatus",
      enabled: true,
    });
    expect(returned).toEqual(next);
    expect(result.current.settings).toEqual(next);
  });
});
