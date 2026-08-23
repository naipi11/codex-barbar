import type { UseStatusSurfaceResult } from "../hooks/useStatusSurface";
import type {
  StatusQuotaMetric,
  TrustState,
} from "../lib/statusSurfaceViewModel";
import { surfaceAlphaFromTransparency } from "../lib/surfaceTransparency";

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

export function buildTaskbarStatusPresentation(
  surface: UseStatusSurfaceResult,
): TaskbarStatusPresentation {
  const metrics = surface.universalMetric ? [surface.universalMetric] : [];
  const reset = surface.universalMetric;
  const metricsText =
    metrics.map(compactTaskbarMetric).join("，") || "无可用额度";
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
    metrics,
    reset,
    trustState: surface.trustState,
    ariaLabel,
    surfaceAlpha: String(
      surfaceAlphaFromTransparency(
        surface.bootstrap?.settings.taskbarStatusOpacity ?? 20,
      ),
    ),
  };
}
