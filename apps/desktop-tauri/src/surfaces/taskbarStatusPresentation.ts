import type { UseStatusSurfaceResult } from "../hooks/useStatusSurface";
import type {
  StatusQuotaMetric,
  TrustState,
} from "../lib/statusSurfaceViewModel";

export interface TaskbarStatusPresentation {
  displayName: string;
  compactIdentity: string;
  metrics: readonly StatusQuotaMetric[];
  reset: StatusQuotaMetric | null;
  trustState: TrustState;
  ariaLabel: string;
  surfaceAlpha: string;
}

export function compactTaskbarMetric(metric: StatusQuotaMetric): string {
  return `${metric.shortLabel} ${metric.displayedPercent}%`;
}

function nearestReset(
  metrics: readonly StatusQuotaMetric[],
): StatusQuotaMetric | null {
  return metrics.reduce<StatusQuotaMetric | null>((nearest, metric) => {
    const time = metric.resetsAt ? Date.parse(metric.resetsAt) : Number.NaN;
    const nearestTime = nearest?.resetsAt
      ? Date.parse(nearest.resetsAt)
      : Number.NaN;
    if (!Number.isFinite(time)) return nearest;
    return !Number.isFinite(nearestTime) || time < nearestTime
      ? metric
      : nearest;
  }, null);
}

function surfaceAlpha(opacity: number | undefined): string {
  return String(Math.max(0, Math.min(100, opacity ?? 20)) / 100);
}

export function buildTaskbarStatusPresentation(
  surface: UseStatusSurfaceResult,
): TaskbarStatusPresentation {
  const reset = nearestReset(surface.metrics);
  const metricsText =
    surface.metrics.map(compactTaskbarMetric).join("，") || "无可用额度";
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
    compactIdentity: surface.compactIdentity,
    metrics: surface.metrics,
    reset,
    trustState: surface.trustState,
    ariaLabel,
    surfaceAlpha: surfaceAlpha(
      surface.bootstrap?.settings.taskbarStatusOpacity,
    ),
  };
}
