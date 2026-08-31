import { invoke } from "@tauri-apps/api/core";
import type {
  AccountsSnapshotDto,
  AppSettingsDto,
  BootstrapDto,
  CodexCompatibilityDto,
  CurrentSurfaceState,
  DiagnosticsExportDto,
  DiagnosticsSummaryDto,
  ManagedLoginStateDto,
  ManualUpdateResult,
  MenuPreferencesPatchDto,
  PanelPreferencesPatchDto,
  NotificationCapabilityDto,
  ProfileUsageStateDto,
  StatusSurfaceKind,
  SettingsPatchDto,
  UsageSpendDto,
  UsageSpendRange,
  FloatBallMotionDto,
  PlatformCapabilitiesDto,
} from "../types/bridge";

/** Frozen invoke names. Keep this list in sync with the Rust command registry. */
export const commands = {
  getBootstrapState: "get_bootstrap_state",
  getPlatformCapabilities: "get_platform_capabilities",
  getSettingsSnapshot: "get_settings_snapshot",
  getNotificationCapability: "get_notification_capability",
  updateSettings: "update_settings",
  applyMenuPreferences: "apply_menu_preferences",
  getUsageSpend: "get_usage_spend",
  sendTestNotification: "send_test_notification",
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
  saveProfileAvatar: "save_profile_avatar",
  clearProfileAvatar: "clear_profile_avatar",
  getDiagnosticsSummary: "get_diagnostics_summary",
  exportDiagnostics: "export_diagnostics",
  validateCodexExecutable: "validate_codex_executable",
  checkForUpdates: "check_for_updates",
  openReleasePage: "open_release_page",
  openCodexUsagePage: "open_codex_usage_page",
  openWindowsNotificationSettings: "open_windows_notification_settings",
  openSettingsWindow: "open_settings_window",
  closeSettingsWindow: "close_settings_window",
  dismissTrayPanel: "dismiss_tray_panel",
  openTrayPanel: "open_tray_panel",
  setFlyoutSize: "set_flyout_size",
  setFlyoutInteracting: "set_flyout_interacting",
  getCurrentSurfaceState: "get_current_surface_state",
  quitApp: "quit_app",
  getFloatBallMotion: "get_float_ball_motion",
} as const;

/** Frozen event names emitted by the Rust account/settings services. */
export const events = {
  profileUsageStateChanged: "profile-usage-state-changed",
  refreshStateChanged: "refresh-state-changed",
  accountsUpdated: "accounts-updated",
  accountLoginUpdated: "account-login-updated",
  selectedProfileChanged: "selected-profile-changed",
  settingsChanged: "settings-changed",
  statusSurfaceFeedbackChanged: "status-surface-feedback-changed",
  floatBallMotionChanged: "codexbar:float-ball-motion-changed",
  localeChanged: "locale-changed",
  updateStateChanged: "update-state-changed",
} as const;

export type ManagedLoginMethod = "browser" | "deviceCode";

export interface StartManagedLoginArgs {
  label: string;
  method: ManagedLoginMethod;
  replaceProfileId?: string | null;
}

export const getBootstrapState = () =>
  invoke<BootstrapDto>(commands.getBootstrapState);

export const getPlatformCapabilities = () =>
  invoke<PlatformCapabilitiesDto>(commands.getPlatformCapabilities);

export const getSettingsSnapshot = () =>
  invoke<AppSettingsDto>(commands.getSettingsSnapshot);

export const getNotificationCapability = () =>
  invoke<NotificationCapabilityDto>(commands.getNotificationCapability);

export const updateSettings = (patch: SettingsPatchDto) =>
  invoke<AppSettingsDto>(commands.updateSettings, {
    patch: patch as Record<string, unknown>,
  });

export const applyPanelPreferences = (preferences: PanelPreferencesPatchDto) =>
  updateSettings({ panel: preferences });

export const applyMenuPreferences = (preferences: MenuPreferencesPatchDto) =>
  invoke<AppSettingsDto>(commands.applyMenuPreferences, {
    preferences: preferences as Record<string, unknown>,
  });

export const getUsageSpend = (range: UsageSpendRange) =>
  invoke<UsageSpendDto>(commands.getUsageSpend, { range });

export const sendTestNotification = () =>
  invoke<void>(commands.sendTestNotification);

export const setStatusSurfaceEnabled = (
  surface: StatusSurfaceKind,
  enabled: boolean,
) =>
  invoke<AppSettingsDto>(commands.setStatusSurfaceEnabled, {
    surface,
    enabled,
  });

export const setFloatBallExpanded = (expanded: boolean) =>
  invoke<void>(commands.setFloatBallExpanded, { expanded });

export const setTaskbarStatusWidth = (width: number) =>
  invoke<void>(commands.setTaskbarStatusWidth, { width });

export const getLocaleStrings = (language?: AppSettingsDto["language"]) =>
  language === undefined
    ? invoke<Record<string, string>>(commands.getLocaleStrings)
    : invoke<Record<string, string>>(commands.getLocaleStrings, { language });

export const selectProfile = (profileId: string) =>
  invoke<AccountsSnapshotDto>(commands.selectProfile, { profileId });

export const refreshSelectedProfile = () =>
  invoke<void>(commands.refreshSelectedProfile);

export const startManagedLogin = (args: StartManagedLoginArgs) =>
  invoke<ManagedLoginStateDto>(
    commands.startManagedLogin,
    args as unknown as Record<string, unknown>,
  );

export const cancelManagedLogin = (operationId: string) =>
  invoke<void>(commands.cancelManagedLogin, { operationId });

export const renameManagedProfile = (profileId: string, label: string) =>
  invoke<AccountsSnapshotDto>(commands.renameManagedProfile, {
    profileId,
    label,
  });

export const removeManagedProfile = (profileId: string) =>
  invoke<AccountsSnapshotDto>(commands.removeManagedProfile, { profileId });

export const saveProfileAvatar = (profileId: string, pngDataUrl: string) =>
  invoke<AccountsSnapshotDto>(commands.saveProfileAvatar, {
    profileId,
    pngDataUrl,
  });

export const clearProfileAvatar = (profileId: string) =>
  invoke<AccountsSnapshotDto>(commands.clearProfileAvatar, { profileId });

export const getDiagnosticsSummary = () =>
  invoke<DiagnosticsSummaryDto>(commands.getDiagnosticsSummary);

export const exportDiagnostics = () =>
  invoke<DiagnosticsExportDto>(commands.exportDiagnostics);

export const validateCodexExecutable = (path: string) =>
  invoke<CodexCompatibilityDto>(commands.validateCodexExecutable, { path });

export const checkForUpdates = () =>
  invoke<ManualUpdateResult>(commands.checkForUpdates);

export const openReleasePage = () => invoke<void>(commands.openReleasePage);

export const openCodexUsagePage = () =>
  invoke<void>(commands.openCodexUsagePage);

export const openWindowsNotificationSettings = () =>
  invoke<void>(commands.openWindowsNotificationSettings);

export const openSettingsWindow = () =>
  invoke<void>(commands.openSettingsWindow);

export const closeSettingsWindow = () =>
  invoke<void>(commands.closeSettingsWindow);

export const dismissTrayPanel = () =>
  invoke<void>(commands.dismissTrayPanel);

export const openTrayPanel = () => invoke<void>(commands.openTrayPanel);

export const setFlyoutSize = (width: number, height: number) =>
  invoke<void>(commands.setFlyoutSize, { width, height });

export const setFlyoutInteracting = (active: boolean) =>
  invoke<void>(commands.setFlyoutInteracting, { active });

export const getCurrentSurfaceState = () =>
  invoke<CurrentSurfaceState>(commands.getCurrentSurfaceState);

export const quitApp = () => invoke<void>(commands.quitApp);

export const getFloatBallMotion = () =>
  invoke<FloatBallMotionDto>(commands.getFloatBallMotion);
