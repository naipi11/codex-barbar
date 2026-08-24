import type {
  AppErrorKind,
  AppSettingsDto,
  BootstrapDto,
  ProfileSummaryDto,
  ProfileUsageStateDto,
  UsageWindowDto,
} from "../types/bridge";

export const defaultSettings: AppSettingsDto = {
  autostartEnabled: false,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  theme: "system",
  language: "en-US",
  codexExecutableOverride: null,
  taskbarStatusEnabled: false,
  floatBallEnabled: false,
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

export function currentCliProfile(
  overrides: Partial<ProfileSummaryDto> = {},
): ProfileSummaryDto {
  return {
    id: "personal",
    kind: "currentCli",
    label: "Personal",
    email: null,
    accountDisplayName: null,
    accountEmail: null,
    accountStatus: "unavailable",
    accountUpdatedAt: null,
    planType: "plus",
    presentationName: "账号信息不可用",
    avatarKind: "default",
    avatarAssetUri: null,
    authMode: "chatGpt",
    removable: false,
    lastSuccessAt: "2026-08-06T00:00:00Z",
    ...overrides,
  };
}

export function managedProfile(
  overrides: Partial<ProfileSummaryDto> = {},
): ProfileSummaryDto {
  return {
    id: "work",
    kind: "managed",
    label: "Work",
    email: null,
    accountDisplayName: null,
    accountEmail: null,
    accountStatus: "unavailable",
    accountUpdatedAt: null,
    planType: "team",
    presentationName: "账号信息不可用",
    avatarKind: "default",
    avatarAssetUri: null,
    authMode: "chatGpt",
    removable: true,
    lastSuccessAt: "2026-08-06T00:00:00Z",
    ...overrides,
  };
}

export function usageWindow(
  remainingPercent: number,
  overrides: Partial<UsageWindowDto> = {},
): UsageWindowDto {
  return {
    limitId: "five-hour",
    label: "5-hour quota",
    usedPercent: 100 - remainingPercent,
    remainingPercent,
    windowDurationMinutes: 300,
    resetsAt: "2026-08-06T05:00:00Z",
    reachedType: null,
    ...overrides,
  };
}

export function profileUsageFixture(
  profileIdOrOptions:
    | string
    | {
        profileId?: string;
        primaryRemaining?: number;
        errorKind?: AppErrorKind | null;
      },
  primaryRemaining = 42,
  overrides: Partial<ProfileUsageStateDto> = {},
): ProfileUsageStateDto {
  const options =
    typeof profileIdOrOptions === "string"
      ? {
          profileId: profileIdOrOptions,
          primaryRemaining,
          errorKind: overrides.currentError?.kind ?? null,
        }
      : profileIdOrOptions;
  const errorKind = options.errorKind ?? null;
  return {
    profileId: options.profileId ?? "personal",
    primary: errorKind === "apiKeyNoQuota" ? null : usageWindow(options.primaryRemaining ?? 42),
    secondary: null,
    additionalWindows: [],
    fetchedAt: "2026-08-06T00:00:00Z",
    currentError: errorKind
      ? {
          kind: errorKind,
          userMessageKey: `errors.${errorKind}`,
          action:
            errorKind === "apiKeyNoQuota" ? "explainApiBilling" : "retry",
          retryAfter: null,
        }
      : null,
    freshness: errorKind ? "stale" : "fresh",
    refreshStatus: "idle",
    manualCooldownUntil: null,
    protocolAnomaly: false,
    ...overrides,
  };
}

export function bootstrapWithTwoProfiles(): BootstrapDto {
  return {
    productName: "codex-barbar",
    version: "1.0.0",
    settings: { ...defaultSettings },
    profiles: [currentCliProfile(), managedProfile()],
    selectedProfileId: "personal",
    usageByProfile: {
      personal: profileUsageFixture("personal", 42),
      work: profileUsageFixture("work", 61),
    },
    statusSurfaceFeedback: {
      taskbarStatusCloseFailed: false,
      floatBallCloseFailed: false,
    },
    codex: {
      status: "compatible",
      installation: "nativeExe",
      executablePath: "C:\\Program Files\\Codex\\codex.exe",
      version: "0.1.0",
      capabilities: {
        accountRead: true,
        rateLimitsRead: true,
        managedLogin: true,
      },
    },
  };
}

export function personalLateState(): ProfileUsageStateDto {
  return profileUsageFixture("personal", 99);
}

export function readyTwoWindowFixture(): BootstrapDto {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.usageByProfile.personal = {
    ...bootstrap.usageByProfile.personal,
    secondary: usageWindow(61, {
      limitId: "weekly",
      label: "Weekly quota",
      windowDurationMinutes: 10_080,
      resetsAt: "2026-08-14T00:00:00Z",
    }),
  };
  return bootstrap;
}

export function weeklyOnlyUsage(
  overrides: Partial<UsageWindowDto> = {},
): ProfileUsageStateDto {
  return profileUsageFixture("personal", 98, {
    primary: usageWindow(98, {
      limitId: "weekly",
      label: "Weekly quota",
      windowDurationMinutes: 10_080,
      resetsAt: "2026-08-20T00:00:00Z",
      ...overrides,
    }),
    secondary: null,
    additionalWindows: [],
  });
}

export function staleOfflineFixture(): BootstrapDto {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.usageByProfile.personal = profileUsageFixture({
    profileId: "personal",
    primaryRemaining: 42,
    errorKind: "offlineOrTimeout",
  });
  return bootstrap;
}
