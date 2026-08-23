import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  events,
  getBootstrapState,
  openTrayPanel,
  setFloatBallExpanded,
  setStatusSurfaceEnabled,
} from "../lib/tauri";
import {
  buildStatusSurfaceViewModel,
  type StatusSurfaceViewModel,
} from "../lib/statusSurfaceViewModel";
import type {
  AppSettingsDto,
  BootstrapDto,
  ProfileSummaryDto,
  ProfileUsageStateDto,
  StatusSurfaceFeedbackChangedDto,
  StatusSurfaceKind,
} from "../types/bridge";
import { useProfileUsage } from "./useProfileUsage";

export { profileDisplayName } from "../lib/statusSurfaceViewModel";
export type { StatusSurfaceStatus } from "../lib/statusSurfaceViewModel";

export interface UseStatusSurfaceResult extends StatusSurfaceViewModel {
  bootstrap: BootstrapDto | null;
  profile: ProfileSummaryDto | null;
  state: ProfileUsageStateDto;
  isDragging: boolean;
  closeFailedBySurface: Record<StatusSurfaceKind, boolean>;
  setIsDragging(value: boolean): void;
  openPanel(): Promise<void>;
  disableSurface(surface: StatusSurfaceKind): Promise<unknown>;
  setFloatBallExpanded(expanded: boolean): Promise<void>;
}

const EMPTY_BOOTSTRAP: BootstrapDto = {
  productName: "codex-barbar",
  version: "0.0.0",
  settings: {
    autostartEnabled: true,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    theme: "dark",
    language: "system",
    codexExecutableOverride: null,
    taskbarStatusEnabled: false,
    floatBallEnabled: true,
    taskbarStatusOpacity: 20,
    floatBallOpacity: 20,
    floatBallGlow: 20,
    taskbarTray: {
      showTaskbarIcon: true,
      showTaskbarAccount: true,
      showWeeklyLabel: true,
      showWeeklyPercent: true,
      showResetDate: true,
      density: "compact",
      trayIconMode: "dynamic",
      tooltipAccount: true,
      tooltipWeekly: true,
      tooltipResetDate: true,
      tooltipUpdatedAt: true,
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
  },
  profiles: [],
  selectedProfileId: "",
  usageByProfile: {},
  statusSurfaceFeedback: {
    taskbarStatusCloseFailed: false,
    floatBallCloseFailed: false,
  },
  codex: {
    status: "notChecked",
    installation: null,
    executablePath: null,
    version: null,
    capabilities: {
      accountRead: false,
      rateLimitsRead: false,
      managedLogin: false,
    },
  },
};

const EMPTY_CLOSE_FEEDBACK: Record<StatusSurfaceKind, boolean> = {
  taskbarStatus: false,
  floatBall: false,
};

export function profileStatusLabel(profile: ProfileSummaryDto | null): string {
  if (!profile) return "未登录";
  switch (profile.accountStatus) {
    case "signedIn":
      return "已登录";
    case "signedOut":
      return "未登录";
    default:
      return "账号信息不可用";
  }
}

export function useStatusSurface(): UseStatusSurfaceResult {
  const [bootstrap, setBootstrap] = useState<BootstrapDto | null>(null);
  const latestSettings = useRef<AppSettingsDto | null>(null);
  const latestCloseFeedback = useRef<
    Partial<Record<StatusSurfaceKind, boolean>>
  >({});
  const [isDragging, setIsDragging] = useState(false);
  const [closeFailedBySurface, setCloseFailedBySurface] = useState(
    EMPTY_CLOSE_FEEDBACK,
  );
  const usage = useProfileUsage(bootstrap ?? EMPTY_BOOTSTRAP);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void | Promise<void>) | undefined;
    void listen<AppSettingsDto>(events.settingsChanged, (event) => {
      if (!active) return;
      latestSettings.current = event.payload;
      setBootstrap((current) =>
        current ? { ...current, settings: event.payload } : current,
      );
    })
      .then((nextUnlisten) => {
        if (active) unlisten = nextUnlisten;
        else void nextUnlisten();
      })
      .catch(() => {});
    return () => {
      active = false;
      void unlisten?.();
    };
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void | Promise<void>) | undefined;
    void listen<StatusSurfaceFeedbackChangedDto>(
      events.statusSurfaceFeedbackChanged,
      (event) => {
        if (!active) return;
        const { surface, closeFailed } = event.payload;
        latestCloseFeedback.current = {
          ...latestCloseFeedback.current,
          [surface]: closeFailed,
        };
        setCloseFailedBySurface((current) => ({
          ...current,
          [surface]: closeFailed,
        }));
      },
    )
      .then((nextUnlisten) => {
        if (active) unlisten = nextUnlisten;
        else void nextUnlisten();
      })
      .catch(() => {});
    return () => {
      active = false;
      void unlisten?.();
    };
  }, []);

  useEffect(() => {
    let active = true;
    getBootstrapState()
      .then((next) => {
        if (active) {
          const bootstrapCloseFeedback: Record<StatusSurfaceKind, boolean> = {
            taskbarStatus: next.statusSurfaceFeedback.taskbarStatusCloseFailed,
            floatBall: next.statusSurfaceFeedback.floatBallCloseFailed,
          };
          setBootstrap({
            ...next,
            settings: latestSettings.current ?? next.settings,
          });
          setCloseFailedBySurface({
            ...bootstrapCloseFeedback,
            ...latestCloseFeedback.current,
          });
        }
      })
      .catch(() => {
        if (active) setBootstrap(null);
      });
    return () => {
      active = false;
    };
  }, []);

  const profile = useMemo(
    () =>
      usage.profiles.find((candidate) => candidate.id === usage.selectedProfileId) ??
      null,
    [usage.profiles, usage.selectedProfileId],
  );
  const model = useMemo(
    () =>
      buildStatusSurfaceViewModel({
        profile,
        state: usage.state,
        displayMode:
          bootstrap?.settings.displayMode ?? EMPTY_BOOTSTRAP.settings.displayMode,
        language:
          bootstrap?.settings.language ?? EMPTY_BOOTSTRAP.settings.language,
        nowMs: Date.now(),
      }),
    [bootstrap?.settings.displayMode, bootstrap?.settings.language, profile, usage.state],
  );
  const openPanel = useCallback(() => openTrayPanel(), []);
  const disableSurface = useCallback(async (surface: StatusSurfaceKind) => {
    setCloseFailedBySurface((current) => ({ ...current, [surface]: false }));
    try {
      return await setStatusSurfaceEnabled(surface, false);
    } catch (error) {
      setCloseFailedBySurface((current) => ({ ...current, [surface]: true }));
      throw error;
    }
  }, []);
  const setExpanded = useCallback(
    (expanded: boolean) => setFloatBallExpanded(expanded),
    [],
  );

  return {
    ...model,
    bootstrap,
    profile,
    state: usage.state,
    isDragging,
    closeFailedBySurface,
    setIsDragging,
    openPanel,
    disableSurface,
    setFloatBallExpanded: setExpanded,
  };
}
