import type { UseStatusSurfaceResult } from "../hooks/useStatusSurface";
import type {
  StatusQuotaMetric,
  TrustState,
} from "../lib/statusSurfaceViewModel";
import { surfaceAlphaFromTransparency } from "../lib/surfaceTransparency";

export interface TaskbarStatusPresentation {
  displayName: string;
  compactIdentity: string | null;
  avatarKind: "default" | "official" | "manual";
  avatarAssetUri: string | null;
  showIcon: boolean;
  showAccount: boolean;
  showWeeklyLabel: boolean;
  showWeeklyPercent: boolean;
  showResetDate: boolean;
  density: "compact" | "standard";
  weeklyText: string | null;
  resetDateText: string | null;
  metrics: readonly StatusQuotaMetric[];
  reset: StatusQuotaMetric | null;
  trustState: TrustState;
  ariaLabel: string;
  surfaceAlpha: string;
}

export function compactTaskbarMetric(metric: StatusQuotaMetric): string {
  return `${metric.shortLabel} ${metric.displayedPercent}%`;
}

function weeklyText(
  metric: StatusQuotaMetric | null,
  showLabel: boolean,
  showPercent: boolean,
): string | null {
  if (!metric) return null;
  const parts: string[] = [];
  if (showLabel) parts.push(metric.shortLabel);
  if (showPercent) parts.push(`${metric.displayedPercent}%`);
  return parts.length > 0 ? parts.join(" ") : null;
}

function resetDateText(metric: StatusQuotaMetric | null): string | null {
  if (!metric?.resetsAt) return null;
  const date = new Date(metric.resetsAt);
  return Number.isNaN(date.valueOf())
    ? null
    : `${date.getMonth() + 1}/${date.getDate()}`;
}

export function buildTaskbarStatusPresentation(
  surface: UseStatusSurfaceResult,
): TaskbarStatusPresentation {
  const metrics = surface.universalMetric ? [surface.universalMetric] : [];
  const reset = surface.universalMetric;
  const prefs = surface.bootstrap?.settings.taskbarPresentation;
  const showIcon = prefs?.showTaskbarIcon ?? true;
  const showAccount = prefs?.showTaskbarAccount ?? true;
  const showWeeklyLabel = prefs?.showWeeklyLabel ?? true;
  const showWeeklyPercent = prefs?.showWeeklyPercent ?? true;
  const showResetDate = prefs?.showResetDate ?? true;
  const density = prefs?.density ?? "compact";
  const weekly = weeklyText(reset, showWeeklyLabel, showWeeklyPercent);
  const resetDate = showResetDate ? resetDateText(reset) : null;
  const metricsText =
    weekly ?? (metrics.map(compactTaskbarMetric).join("，") || "无可用额度");
  const trustText =
    surface.trustState === "cached" ? "缓存数据" : surface.refreshStatus;
  const ariaLabel = [
    "打开完整面板",
    surface.displayName,
    metricsText,
    reset?.resetText,
    trustText,
    surface.updatedText,
  ]
    .filter(Boolean)
    .join("，");

  return {
    displayName: surface.displayName,
    compactIdentity: showAccount ? surface.compactIdentity : null,
    avatarKind: surface.avatarKind,
    avatarAssetUri: surface.avatarAssetUri,
    showIcon,
    showAccount,
    showWeeklyLabel,
    showWeeklyPercent,
    showResetDate,
    density,
    weeklyText: weekly,
    resetDateText: resetDate,
    metrics,
    reset,
    trustState: surface.trustState,
    ariaLabel,
    surfaceAlpha: String(
      surfaceAlphaFromTransparency(
        surface.bootstrap?.settings.taskbarTransparencyPercent ?? 20,
      ),
    ),
  };
}
