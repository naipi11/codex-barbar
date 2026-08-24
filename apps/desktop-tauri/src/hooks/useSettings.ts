import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  events,
  getSettingsSnapshot,
  setStatusSurfaceEnabled as invokeSetStatusSurfaceEnabled,
  updateSettings as invokeUpdateSettings,
} from "../lib/tauri";
import type {
  AppSettingsDto,
  SettingsPatchDto,
  StatusSurfaceKind,
} from "../types/bridge";

export const defaultAppSettings: AppSettingsDto = {
  autostartEnabled: true,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  theme: "system",
  language: "system",
  codexExecutableOverride: null,
  taskbarStatusEnabled: false,
  floatBallEnabled: true,
  taskbarTransparencyPercent: 20,
  floatBallTransparencyPercent: 20,
  floatBallGlowPercent: 20,
  taskbarPresentation: {
    showTaskbarIcon: true,
    showTaskbarAccount: true,
    showWeeklyLabel: true,
    showWeeklyPercent: true,
    showResetDate: true,
    density: "compact",
    hideStatusSurfacesInFullscreen: true,
  },
  menu: {
    nativeTray: {
      order: [
        "open_panel",
        "refresh",
        "accounts",
        "open_usage",
        "settings",
        "about",
        "quit",
      ],
      hidden: [],
    },
    trayPanel: {
      order: ["refresh", "open_usage", "settings", "dismiss", "quit"],
      hidden: [],
    },
  },
  panel: {
    density: "compact",
    showResetTime: true,
    showFreshness: true,
    showAccountStatus: true,
    actions: {
      order: ["refresh", "open_usage", "settings", "dismiss", "quit"],
      hidden: [],
    },
  },
  notifications: {
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
  },
};

export interface UseSettingsResult {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<AppSettingsDto>;
  setSurfaceEnabled(
    surface: StatusSurfaceKind,
    enabled: boolean,
  ): Promise<AppSettingsDto>;
}

export function useSettings(): UseSettingsResult {
  const [settings, setSettings] = useState<AppSettingsDto>(defaultAppSettings);

  useEffect(() => {
    let active = true;
    void getSettingsSnapshot()
      .then((snapshot) => {
        if (active) setSettings(snapshot);
      })
      .catch(() => {
        if (active) setSettings(defaultAppSettings);
      });
    let unlisten: (() => void | Promise<void>) | undefined;
    void listen<AppSettingsDto>(events.settingsChanged, (event) => {
      if (active) setSettings(event.payload);
    }).then((fn) => {
      if (active) unlisten = fn;
      else void fn();
    });
    return () => {
      active = false;
      if (unlisten) void unlisten();
    };
  }, []);

  const update = useCallback(async (patch: SettingsPatchDto) => {
    const next = await invokeUpdateSettings(patch);
    setSettings(next);
    return next;
  }, []);

  const setSurfaceEnabled = useCallback(
    async (surface: StatusSurfaceKind, enabled: boolean) => {
      const next = await invokeSetStatusSurfaceEnabled(surface, enabled);
      setSettings(next);
      return next;
    },
    [],
  );

  return { settings, update, setSurfaceEnabled };
}
