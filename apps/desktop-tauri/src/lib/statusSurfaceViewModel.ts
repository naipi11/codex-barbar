import type {
  AppSettingsDto,
  ProfileSummaryDto,
  ProfileUsageStateDto,
  UsageWindowDto,
} from "../types/bridge";

const FIVE_HOUR_MINUTES = 300;
const WEEKLY_MINUTES = 10_080;
const GENERIC_QUOTA_LABEL = "Quota";
const STATUS_LABEL_LENGTH = 6;

export type StatusSurfaceStatus =
  | "ready"
  | "warning"
  | "critical"
  | "refreshing"
  | "stale"
  | "missing";

export type TrustState = "trusted" | "refreshing" | "cached" | "missing";
export type QuotaBand = "high" | "medium" | "low" | "unknown";

export interface StatusQuotaMetric {
  kind: "fiveHour" | "weekly" | "other";
  limitId: string;
  label: string;
  shortLabel: string;
  usedPercent: number;
  remainingPercent: number;
  displayedPercent: number;
  displayMode: "remaining" | "used";
  resetText: string;
  resetsAt: string | null;
  band: QuotaBand;
}

export interface StatusSurfaceViewModel {
  displayName: string;
  compactIdentity: string;
  accountStatus: "signedIn" | "signedOut" | "unavailable";
  metrics: StatusQuotaMetric[];
  primaryMetric: StatusQuotaMetric | null;
  secondaryMetric: StatusQuotaMetric | null;
  urgentMetric: StatusQuotaMetric | null;
  universalMetric: StatusQuotaMetric | null;
  trustState: TrustState;
  freshness: "fresh" | "stale" | "missing";
  refreshStatus: string;
  updatedText: string | null;
  status: StatusSurfaceStatus;
}

export interface StatusSurfaceViewModelInput {
  profile: ProfileSummaryDto | null;
  state: ProfileUsageStateDto;
  displayMode: AppSettingsDto["displayMode"];
  language?: AppSettingsDto["language"];
  nowMs: number;
}

function cleanIdentity(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  if (/^current[\s_-]*cli$/i.test(trimmed)) return null;
  return trimmed;
}

function accountStatusFallback(
  status: ProfileSummaryDto["accountStatus"] | undefined,
): string {
  switch (status) {
    case "signedIn":
      return "已登录（名称不可用）";
    case "signedOut":
      return "未登录";
    default:
      return "账号信息不可用";
  }
}

export function profileDisplayName(profile: ProfileSummaryDto | null): string {
  if (!profile) return "未登录";
  return (
    cleanIdentity(profile.accountDisplayName) ??
    cleanIdentity(profile.accountEmail) ??
    cleanIdentity(profile.email) ??
    (profile.kind === "managed" ? cleanIdentity(profile.label) : null) ??
    accountStatusFallback(profile.accountStatus)
  );
}

export function compactIdentity(value: string): string {
  const trimmed = value.trim();
  const localPart = emailLocalPart(trimmed) ?? trimmed;
  return graphemeSlice(localPart, STATUS_LABEL_LENGTH);
}

function emailLocalPart(value: string): string | null {
  const at = value.indexOf("@");
  if (at <= 0) return null;
  const before = value.slice(0, at);
  if (before.includes("@") || before.includes(" ")) return null;
  return before;
}

function graphemeSlice(value: string, max: number): string {
  const intlWithSegmenter = Intl as typeof Intl & {
    Segmenter?: new (
      locales?: string | string[],
      options?: { granularity?: string },
    ) => {
      segment(input: string): Iterable<{ segment: string }>;
    };
  };
  if (intlWithSegmenter.Segmenter) {
    const segmenter = new intlWithSegmenter.Segmenter(undefined, {
      granularity: "grapheme",
    });
    const segments = Array.from(segmenter.segment(value), (segment) => segment.segment);
    return segments.slice(0, max).join("");
  }
  return Array.from(value).slice(0, max).join("");
}

export function quotaBandFor(
  remainingPercent: number,
  trust: TrustState,
): QuotaBand {
  if (trust === "missing") return "unknown";
  const remaining = clampPercent(remainingPercent);
  if (remaining >= 67) return "high";
  if (remaining >= 34) return "medium";
  return "low";
}

function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, Math.round(value)));
}

function formatResetCountdown(value: string | null, nowMs: number): string {
  if (!value) return "—";
  const resetsAtMs = Date.parse(value);
  if (!Number.isFinite(resetsAtMs)) return "—";

  const remainingMinutes = Math.ceil((resetsAtMs - nowMs) / 60_000);
  if (remainingMinutes <= 0) return "即将重置";
  if (remainingMinutes < 60) return `${remainingMinutes}m`;

  const hours = Math.floor(remainingMinutes / 60);
  const minutes = remainingMinutes % 60;
  if (hours < 48) return `${hours}h${String(minutes).padStart(2, "0")}m`;
  return `${Math.floor(hours / 24)}天`;
}

function formatUpdatedText(value: string | null, nowMs: number): string | null {
  if (!value) return null;
  const updatedAtMs = Date.parse(value);
  if (!Number.isFinite(updatedAtMs)) return null;

  const elapsedMinutes = Math.max(0, Math.floor((nowMs - updatedAtMs) / 60_000));
  if (elapsedMinutes < 60) return `${elapsedMinutes}分钟前`;
  const hours = Math.floor(elapsedMinutes / 60);
  const minutes = elapsedMinutes % 60;
  if (hours < 48) return `${hours}h${String(minutes).padStart(2, "0")}m前`;
  return `${Math.floor(hours / 24)}天前`;
}

function metricKind(window: UsageWindowDto): StatusQuotaMetric["kind"] {
  if (window.windowDurationMinutes === FIVE_HOUR_MINUTES) return "fiveHour";
  if (window.windowDurationMinutes === WEEKLY_MINUTES) return "weekly";
  return "other";
}

function isChinese(language?: AppSettingsDto["language"]): boolean {
  if (language === "en-US") return false;
  return true;
}


function shortWindowLabel(window: UsageWindowDto, language?: AppSettingsDto["language"]): string {
  if (window.windowDurationMinutes === FIVE_HOUR_MINUTES) return isChinese(language) ? "5H" : "5H";
  if (window.windowDurationMinutes === WEEKLY_MINUTES) return isChinese(language) ? "周" : "Wk";
  const label = window.label?.trim();
  return label
    ? Array.from(label).slice(0, STATUS_LABEL_LENGTH).join("")
    : GENERIC_QUOTA_LABEL;
}

function metricWindowKey(window: UsageWindowDto): string {
  const limitId = window.limitId.trim();
  if (limitId) return `id:${limitId}`;
  return `window:${window.windowDurationMinutes ?? ""}\u0000${window.label ?? ""}\u0000${window.resetsAt ?? ""}`;
}

function metricWindows(state: ProfileUsageStateDto): UsageWindowDto[] {
  const seen = new Set<string>();
  const unique = [state.primary, state.secondary, ...state.additionalWindows].filter(
    (window): window is UsageWindowDto => {
      if (!window) return false;
      const key = metricWindowKey(window);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    },
  );
  // A provider may report the same quota window twice under different limit
  // ids (e.g. one exhausted and one full weekly bucket). Show only the most
  // urgent window per duration so surfaces never render contradictory
  // duplicates like "周 0%" next to "周 100%".
  const urgentByDuration = new Map<number, UsageWindowDto>();
  for (const window of unique) {
    const duration = window.windowDurationMinutes;
    if (duration == null) continue;
    const existing = urgentByDuration.get(duration);
    if (!existing || window.remainingPercent < existing.remainingPercent) {
      urgentByDuration.set(duration, window);
    }
  }
  const kept = new Set(urgentByDuration.values());
  return unique.filter(
    (window) => window.windowDurationMinutes == null || kept.has(window),
  );
}

function toMetric(
  window: UsageWindowDto,
  displayMode: AppSettingsDto["displayMode"],
  nowMs: number,
  trustState: TrustState,
  language?: AppSettingsDto["language"],
): StatusQuotaMetric {
  const remainingPercent = clampPercent(window.remainingPercent);
  const usedPercent = clampPercent(window.usedPercent);
  const shortLabel = shortWindowLabel(window, language);
  return {
    kind: metricKind(window),
    limitId: window.limitId,
    label: shortLabel,
    shortLabel,
    usedPercent,
    remainingPercent,
    displayedPercent: displayMode === "used" ? usedPercent : remainingPercent,
    displayMode,
    resetText: formatResetCountdown(window.resetsAt, nowMs),
    resetsAt: window.resetsAt,
    band: quotaBandFor(remainingPercent, trustState),
  };
}

function trustStateFor(
  state: ProfileUsageStateDto,
  windows: readonly UsageWindowDto[],
): TrustState {
  if (state.freshness === "missing" || windows.length === 0) return "missing";
  if (state.refreshStatus === "refreshing") return "refreshing";
  if (state.currentError || state.freshness === "stale") return "cached";
  return "trusted";
}

function urgentMetric(metrics: readonly StatusQuotaMetric[]): StatusQuotaMetric | null {
  return metrics.reduce<StatusQuotaMetric | null>(
    (urgent, metric) =>
      !urgent || metric.remainingPercent < urgent.remainingPercent ? metric : urgent,
    null,
  );
}

function universalWeeklyWindow(
  state: ProfileUsageStateDto,
): UsageWindowDto | null {
  return (
    [state.primary, state.secondary].find(
      (window): window is UsageWindowDto =>
        window?.windowDurationMinutes === WEEKLY_MINUTES,
    ) ?? null
  );
}

function deriveStatus(
  state: ProfileUsageStateDto,
  urgent: StatusQuotaMetric | null,
  trustState: TrustState,
  language?: AppSettingsDto["language"],
): StatusSurfaceStatus {
  switch (trustState) {
    case "missing":
      return "missing";
    case "refreshing":
      return "refreshing";
    case "cached":
      return state.currentError ? "critical" : "stale";
    case "trusted": {
      const usedPercent = urgent?.usedPercent ?? 0;
      if (usedPercent >= 90) return "critical";
      if (usedPercent >= 75) return "warning";
      return "ready";
    }
  }
}

function refreshStatusText(state: ProfileUsageStateDto): string {
  if (state.freshness === "missing") return "等待数据";
  if (state.refreshStatus === "refreshing") return "正在刷新";
  if (state.refreshStatus === "cooldown" || state.refreshStatus === "backoff") {
    return "等待重试";
  }
  if (state.currentError) return "刷新失败";
  if (state.freshness === "stale") return "缓存数据";
  return "已更新";
}

export function buildStatusSurfaceViewModel({
  profile,
  state,
  displayMode,
  language,
  nowMs,
}: StatusSurfaceViewModelInput): StatusSurfaceViewModel {
  const windows = metricWindows(state);
  const universalWindow = universalWeeklyWindow(state);
  const trustState = trustStateFor(
    state,
    universalWindow ? [universalWindow] : [],
  );
  const metrics = windows.map((window) =>
    toMetric(window, displayMode, nowMs, trustState, language),
  );
  const urgent = urgentMetric(metrics);
  const universal = universalWindow
    ? toMetric(universalWindow, displayMode, nowMs, trustState, language)
    : null;
  const displayName = profileDisplayName(profile);

  return {
    displayName,
    compactIdentity: compactIdentity(displayName),
    accountStatus: profile?.accountStatus ?? "signedOut",
    metrics,
    primaryMetric: metrics.find((metric) => metric.kind === "fiveHour") ?? null,
    secondaryMetric: metrics.find((metric) => metric.kind === "weekly") ?? null,
    urgentMetric: urgent,
    universalMetric: universal,
    trustState,
    freshness: state.freshness,
    refreshStatus: refreshStatusText(state),
    updatedText: formatUpdatedText(state.fetchedAt, nowMs),
    status: deriveStatus(state, universal, trustState),
  };
}
