/**
 * The only data contract allowed to cross between the Rust shell and the
 * React WebView.  These DTOs intentionally contain no credentials, paths,
 * raw protocol text, or provider-specific identity outside the selected
 * Codex profile.
 */

export type AppErrorKind =
  | "codexNotFound"
  | "unsupportedCodexVersion"
  | "notSignedIn"
  | "apiKeyNoQuota"
  | "authExpired"
  | "offlineOrTimeout"
  | "rateLimited"
  | "protocolMismatch"
  | "vaultFailure"
  | "storageFailure";

export type RecoveryAction =
  | "selectCodexExecutable"
  | "installTestedCodex"
  | "signIn"
  | "reloginManagedProfile"
  | "retry"
  | "waitAndRetry"
  | "explainApiBilling"
  | "exportDiagnostics";

export interface AppErrorDto {
  kind: AppErrorKind;
  userMessageKey: string;
  action: RecoveryAction;
  retryAfter: string | null;
}

export interface NotificationPreferencesDto {
  enabled: boolean;
  playSound: boolean;
  warningEnabled: boolean;
  dangerEnabled: boolean;
  weeklyResetEnabled: boolean;
  resetCreditIncreaseEnabled: boolean;
  refreshFailureEnabled: boolean;
  updateAvailableEnabled: boolean;
  pricingChangedEnabled: boolean;
  pricingRefreshFailureEnabled: boolean;
  warningRemainingPercent: number;
  dangerRemainingPercent: number;
}

export interface NotificationCapabilityDto {
  status: "available" | "appDisabled" | "globalDisabled" | "unsupported";
  canOpenSettings: boolean;
}

export interface TaskbarPresentationPreferencesDto {
  showTaskbarIcon: boolean;
  showTaskbarAccount: boolean;
  showWeeklyLabel: boolean;
  showWeeklyPercent: boolean;
  showResetDate: boolean;
  density: "compact" | "standard";
  hideStatusSurfacesInFullscreen: boolean;
}

export interface MenuLayoutDto {
  order: string[];
  hidden: string[];
}

export interface MenuPreferencesDto {
  nativeTray: MenuLayoutDto;
  trayPanel: MenuLayoutDto;
}

export interface PanelPreferencesDto {
  density: "compact" | "standard";
  showResetTime: boolean;
  showFreshness: boolean;
  showAccountStatus: boolean;
  actions: MenuLayoutDto;
}

export interface PanelPreferencesPatchDto {
  density?: "compact" | "standard";
  showResetTime?: boolean;
  showFreshness?: boolean;
  showAccountStatus?: boolean;
  actions?: MenuLayoutPatchDto;
}

export interface MenuLayoutPatchDto {
  order?: string[];
  hidden?: string[];
}

export interface MenuPreferencesPatchDto {
  nativeTray?: MenuLayoutPatchDto;
  trayPanel?: MenuLayoutPatchDto;
}

export interface AppSettingsDto {
  autostartEnabled: boolean;
  refreshIntervalSeconds: 0 | 60 | 300 | 900 | 1800;
  displayMode: "remaining" | "used";
  theme: "system" | "light" | "dark";
  language: "system" | "zh-CN" | "en-US";
  codexExecutableOverride: string | null;
  taskbarStatusEnabled: boolean;
  floatBallEnabled: boolean;
  taskbarTransparencyPercent: number;
  floatBallTransparencyPercent: number;
  floatBallGlowPercent: number;
  notifications: NotificationPreferencesDto;
  taskbarPresentation: TaskbarPresentationPreferencesDto;
  menu: MenuPreferencesDto;
  panel: PanelPreferencesDto;
  pricingDisplayCurrency: "USD" | "CNY";
}

export type MotionState = "idle" | "thinking" | "fast";

export interface FloatBallMotionDto {
  state: MotionState;
  observedAt?: string;
  thinking?: boolean;
  fast?: boolean;
}

export type StatusSurfaceKind = "taskbarStatus" | "floatBall";

export interface StatusSurfaceFeedbackDto {
  taskbarStatusCloseFailed: boolean;
  floatBallCloseFailed: boolean;
}

export interface StatusSurfaceFeedbackChangedDto {
  surface: StatusSurfaceKind;
  closeFailed: boolean;
}

export interface SettingsPatchDto
  extends Partial<
    Omit<AppSettingsDto, "notifications" | "taskbarPresentation" | "menu" | "panel">
  > {
  notifications?: Partial<NotificationPreferencesDto>;
  taskbarPresentation?: Partial<TaskbarPresentationPreferencesDto>;
  panel?: PanelPreferencesPatchDto;
}

export interface ProfileSummaryDto {
  id: string;
  kind: "currentCli" | "managed";
  label: string;
  email: string | null;
  accountDisplayName: string | null;
  accountEmail: string | null;
  accountStatus: "signedIn" | "signedOut" | "unavailable";
  accountUpdatedAt: string | null;
  planType: string | null;
  presentationName: string;
  avatarKind: "default" | "official" | "manual";
  avatarAssetUri: string | null;
  authMode: "unknown" | "chatGpt" | "apiKey";
  removable: boolean;
  lastSuccessAt: string | null;
}

export interface AccountsSnapshotDto {
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
}

export interface UsageWindowDto {
  limitId: string;
  label: string | null;
  usedPercent: number;
  remainingPercent: number;
  windowDurationMinutes: number | null;
  resetsAt: string | null;
  reachedType: string | null;
}

export type Freshness = "fresh" | "stale" | "missing";
export type RefreshStatus =
  | "idle"
  | "refreshing"
  | "cooldown"
  | "backoff"
  | "blocked";


export type UsageSpendRange =
  | "today"
  | "last7Days"
  | "last30Days"
  | "last365Days"
  | "currentWeekly";
export type ResetCreditsState = "available" | "unsupported" | "stale";
export type LocalUsageState = "ready" | "empty" | "unavailable" | "cancelled";

export interface ResetCreditsStateDto {
  state: ResetCreditsState;
  availableCount: number | null;
  observedAt: string | null;
}

export interface OfficialUsageDto {
  remainingPercent: number | null;
  resetsAt: string | null;
  fetchedAt: string | null;
  freshness: "fresh" | "stale" | "missing";
  resetCredits: ResetCreditsStateDto;
}

export interface CostEstimateDto {
  amount: number | null;
  currency: "USD" | "CNY";
  provenance: "exactObserved" | "officialDirect" | "officialEquivalent" | "unpriced";
  canonicalModel: string | null;
  sourceUpdatedAt: string | null;
}

export interface DailyUsageSpendDto {
  date: string;
  totalTokens: number;
  estimatedCost: CostEstimateDto;
}

export interface ModelUsageSpendDto {
  model: string;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  totalTokens: number;
  estimatedCost: CostEstimateDto;
}

export interface LocalUsageSpendDto {
  attribution: "deviceCombined";
  range: UsageSpendRange;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  totalTokens: number;
  sessionsCount: number;
  estimatedCost: CostEstimateDto;
  displayCurrency: "USD" | "CNY";
  pricingStatus: "complete" | "partial" | "unpriced";
  partialEstimate: boolean;
  unpricedModelCount: number;
  unknownModels: string[];
  daily: DailyUsageSpendDto[];
  activity: DailyUsageSpendDto[];
  models: ModelUsageSpendDto[];
  state: LocalUsageState;
  malformedRecordsSkipped: number;
}

export interface UsageSpendDto {
  official: OfficialUsageDto;
  local: LocalUsageSpendDto;
}

export interface ProfileUsageStateDto {
  profileId: string;
  primary: UsageWindowDto | null;
  secondary: UsageWindowDto | null;
  additionalWindows: UsageWindowDto[];
  fetchedAt: string | null;
  currentError: AppErrorDto | null;
  freshness: Freshness;
  refreshStatus: RefreshStatus;
  manualCooldownUntil: string | null;
  protocolAnomaly: boolean;
}

export interface RefreshStateChangedDto {
  profileId: string;
  status: RefreshStatus;
}

export interface SelectedProfileChangedDto {
  profileId: string;
}

export interface CodexCompatibilityDto {
  status: "notChecked" | "compatible" | "notFound" | "unsupported";
  installation: "nativeExe" | "verifiedNpmLayout" | null;
  executablePath: string | null;
  version: string | null;
  capabilities: {
    accountRead: boolean;
    rateLimitsRead: boolean;
    managedLogin: boolean;
  };
}

export type ManualUpdateResult =
  | {
      status: "current";
      currentVersion: string;
    }
  | {
      status: "available";
      currentVersion: string;
      latestVersion: string;
    }
  | {
      status: "releaseFeedUnavailable";
      currentVersion: string;
    };

export interface ManagedLoginStateDto {
  operationId: string;
  profileId: string;
  stage: "starting" | "awaitingUser" | "succeeded" | "failed" | "cancelled";
  verificationUrl: string | null;
  userCode: string | null;
  errorKind: AppErrorKind | null;
}

export interface BootstrapDto {
  productName: "codex-barbar";
  version: string;
  settings: AppSettingsDto;
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  usageByProfile: Record<string, ProfileUsageStateDto>;
  statusSurfaceFeedback: StatusSurfaceFeedbackDto;
  codex: CodexCompatibilityDto;
}

export interface CurrentSurfaceState {
  mode: "hidden" | "trayPanel" | "settings" | string;
  target: { kind: "summary" } | { kind: "settings"; tab: string };
}

export interface DiagnosticsCapabilitiesDto {
  accountRead: boolean;
  rateLimitsRead: boolean;
  managedLogin: boolean;
}

export interface DiagnosticsSummaryDto {
  productName: "codex-barbar";
  version: string;
  os: string;
  codexVersion: string | null;
  resolvedPathClass: string;
  capabilities: DiagnosticsCapabilitiesDto;
  profileKinds: Record<string, number>;
  profileCount: number;
  refreshTimes: string[];
  errorKinds: string[];
  vaultStatus: string;
  recoveryStatus: string;
  storageStatus: string;
  testedVersions: string[];
  logTail: string;
}

export interface DiagnosticsExportDto {
  path: string;
}

const ERROR_KINDS: readonly AppErrorKind[] = [
  "codexNotFound",
  "unsupportedCodexVersion",
  "notSignedIn",
  "apiKeyNoQuota",
  "authExpired",
  "offlineOrTimeout",
  "rateLimited",
  "protocolMismatch",
  "vaultFailure",
  "storageFailure",
];

const RECOVERY_ACTIONS: readonly RecoveryAction[] = [
  "selectCodexExecutable",
  "installTestedCodex",
  "signIn",
  "reloginManagedProfile",
  "retry",
  "waitAndRetry",
  "explainApiBilling",
  "exportDiagnostics",
];

const FRESHNESS_VALUES: readonly Freshness[] = ["fresh", "stale", "missing"];
const REFRESH_STATUS_VALUES: readonly RefreshStatus[] = [
  "idle",
  "refreshing",
  "cooldown",
  "backoff",
  "blocked",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isWindow(value: unknown): value is UsageWindowDto {
  if (!isRecord(value)) return false;
  return (
    typeof value.limitId === "string" &&
    isNullableString(value.label) &&
    typeof value.usedPercent === "number" &&
    Number.isFinite(value.usedPercent) &&
    typeof value.remainingPercent === "number" &&
    Number.isFinite(value.remainingPercent) &&
    (value.windowDurationMinutes === null ||
      typeof value.windowDurationMinutes === "number") &&
    isNullableString(value.resetsAt) &&
    isNullableString(value.reachedType)
  );
}

function isNullableWindow(value: unknown): value is UsageWindowDto | null {
  return value === null || isWindow(value);
}

function isNullableError(value: unknown): value is AppErrorDto | null {
  if (value === null) return true;
  if (!isRecord(value)) return false;
  return (
    typeof value.kind === "string" &&
    ERROR_KINDS.includes(value.kind as AppErrorKind) &&
    typeof value.userMessageKey === "string" &&
    typeof value.action === "string" &&
    RECOVERY_ACTIONS.includes(value.action as RecoveryAction) &&
    isNullableString(value.retryAfter)
  );
}

/**
 * Validate a profile state without repairing it.
 *
 * Rust owns normalization and remaining-percent calculation.  The frontend
 * parser only checks the shape so a malformed payload is rejected instead of
 * silently replacing the last successful snapshot.
 */
export function parseProfileUsageState(
  value: unknown,
): ProfileUsageStateDto {
  if (!isRecord(value)) {
    throw new TypeError("invalid profile usage state");
  }

  const valid =
    typeof value.profileId === "string" &&
    isNullableWindow(value.primary) &&
    isNullableWindow(value.secondary) &&
    Array.isArray(value.additionalWindows) &&
    value.additionalWindows.every(isWindow) &&
    isNullableString(value.fetchedAt) &&
    isNullableError(value.currentError) &&
    typeof value.freshness === "string" &&
    FRESHNESS_VALUES.includes(value.freshness as Freshness) &&
    typeof value.refreshStatus === "string" &&
    REFRESH_STATUS_VALUES.includes(value.refreshStatus as RefreshStatus) &&
    isNullableString(value.manualCooldownUntil) &&
    typeof value.protocolAnomaly === "boolean";

  if (!valid) {
    throw new TypeError("invalid profile usage state");
  }

  return value as unknown as ProfileUsageStateDto;
}

