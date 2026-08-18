import { describe, expect, it } from "vitest";
import {
  commands,
  events,
  setFloatBallExpanded,
  setTaskbarStatusWidth,
  setStatusSurfaceEnabled,
} from "../lib/tauri";
import { invokeMock } from "../test/setup";
import { bootstrapWithTwoProfiles } from "../test/profileUsageFixtures";
import {
  parseProfileUsageState,
  type AppSettingsDto,
  type ProfileSummaryDto,
  type ProfileUsageStateDto,
} from "./bridge";

function profileUsageFixture(
  overrides: Partial<ProfileUsageStateDto> = {},
): ProfileUsageStateDto {
  return {
    profileId: "profile-1",
    primary: {
      limitId: "five-hour",
      label: "usage.fiveHours",
      usedPercent: 58,
      remainingPercent: 42,
      windowDurationMinutes: 300,
      resetsAt: "2026-08-07T12:00:00Z",
      reachedType: null,
    },
    secondary: null,
    additionalWindows: [],
    fetchedAt: "2026-08-07T11:00:00Z",
    currentError: {
      kind: "offlineOrTimeout",
      userMessageKey: "error.offlineOrTimeout",
      action: "retry",
      retryAfter: null,
    },
    freshness: "stale",
    refreshStatus: "idle",
    manualCooldownUntil: null,
    protocolAnomaly: false,
    ...overrides,
  };
}

describe("V1 bridge contract", () => {
  it("exports the frozen command names", () => {
    expect(commands).toEqual({
      getBootstrapState: "get_bootstrap_state",
      getSettingsSnapshot: "get_settings_snapshot",
      updateSettings: "update_settings",
      setStatusSurfaceEnabled: "set_status_surface_enabled",
      setFloatBallExpanded: "set_float_ball_expanded",
      setTaskbarStatusWidth: "set_taskbar_status_width",
      getLocaleStrings: "get_locale_strings",
      selectProfile: "select_profile",
      refreshSelectedProfile: "refresh_selected_profile",
      startManagedLogin: "start_managed_login",
      cancelManagedLogin: "cancel_managed_login",
      renameManagedProfile: "rename_managed_profile",
      removeManagedProfile: "remove_managed_profile",
      getDiagnosticsSummary: "get_diagnostics_summary",
      exportDiagnostics: "export_diagnostics",
      validateCodexExecutable: "validate_codex_executable",
      checkForUpdates: "check_for_updates",
      openReleasePage: "open_release_page",
      openCodexUsagePage: "open_codex_usage_page",
      openSettingsWindow: "open_settings_window",
      openTrayPanel: "open_tray_panel",
      closeSettingsWindow: "close_settings_window",
      dismissTrayPanel: "dismiss_tray_panel",
      setFlyoutSize: "set_flyout_size",
      getCurrentSurfaceState: "get_current_surface_state",
      quitApp: "quit_app",
    });
  });

  it("exports the frozen event names", () => {
    expect(events).toEqual({
      profileUsageStateChanged: "profile-usage-state-changed",
      refreshStateChanged: "refresh-state-changed",
      accountsUpdated: "accounts-updated",
      accountLoginUpdated: "account-login-updated",
      selectedProfileChanged: "selected-profile-changed",
      settingsChanged: "settings-changed",
      statusSurfaceFeedbackChanged: "status-surface-feedback-changed",
      localeChanged: "locale-changed",
      updateStateChanged: "update-state-changed",
    });
  });

  it("keeps the last successful snapshot and current error separate", () => {
    const state = parseProfileUsageState(
      profileUsageFixture({
        primary: {
          limitId: "five-hour",
          label: "usage.fiveHours",
          usedPercent: 58,
          remainingPercent: 42,
          windowDurationMinutes: 300,
          resetsAt: "2026-08-07T12:00:00Z",
          reachedType: null,
        },
        currentError: {
          kind: "offlineOrTimeout",
          userMessageKey: "error.offlineOrTimeout",
          action: "retry",
          retryAfter: null,
        },
      }),
    );

    expect(state.primary?.remainingPercent).toBe(42);
    expect(state.currentError?.kind).toBe("offlineOrTimeout");
  });

  it("exposes the typed status-surface command", async () => {
    expect(commands.setStatusSurfaceEnabled).toBe("set_status_surface_enabled");

    invokeMock.mockResolvedValue({});
    await setStatusSurfaceEnabled("taskbarStatus", false);

    expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", {
      surface: "taskbarStatus",
      enabled: false,
    });
  });

  it("persists the expanded float-ball state through the typed command", async () => {
    invokeMock.mockResolvedValue(undefined);

    await setFloatBallExpanded(true);

    expect(invokeMock).toHaveBeenCalledWith("set_float_ball_expanded", {
      expanded: true,
    });
  });

  it("keeps the status-surface feedback bridge contract frozen", () => {
    const bootstrap = bootstrapWithTwoProfiles();

    expect(events.statusSurfaceFeedbackChanged).toBe(
      "status-surface-feedback-changed",
    );
    expect(bootstrap.statusSurfaceFeedback).toEqual({
      taskbarStatusCloseFailed: false,
      floatBallCloseFailed: false,
    });
  });

  it("sizes taskbar status through the typed command", async () => {
    invokeMock.mockResolvedValue(undefined);

    await setTaskbarStatusWidth(168);

    expect(invokeMock).toHaveBeenCalledWith("set_taskbar_status_width", {
      width: 168,
    });
  });

  it("accepts identity and opt-in surface fields in the bridge shape", () => {
    const profile = {
      id: "profile-1",
      kind: "currentCli",
      label: "Ming Zhao",
      email: null,
      accountDisplayName: "Ming Zhao",
      accountEmail: "ming.zhao@example.com",
      accountStatus: "signedIn",
      accountUpdatedAt: "2026-08-08T00:00:00Z",
      planType: "plus",
      authMode: "chatGpt",
      removable: false,
      lastSuccessAt: null,
    } satisfies ProfileSummaryDto;
    const settings = {
      autostartEnabled: false,
      refreshIntervalSeconds: 300,
      displayMode: "remaining",
      theme: "system",
      language: "system",
      codexExecutableOverride: null,
      taskbarStatusEnabled: true,
      floatBallEnabled: false,
      taskbarStatusOpacity: 0,
      floatBallOpacity: 80,
    } satisfies AppSettingsDto;

    expect(profile.accountDisplayName).toBe("Ming Zhao");
    expect(profile.accountEmail).toContain("@");
    expect(settings.taskbarStatusEnabled).toBe(true);
    expect(settings.floatBallEnabled).toBe(false);
    expect(settings.taskbarStatusOpacity).toBe(0);
    expect(settings.floatBallOpacity).toBe(80);
  });
});
