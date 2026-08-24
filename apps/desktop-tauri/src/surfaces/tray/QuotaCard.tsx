import { useFormattedResetTime } from "../../hooks/useFormattedResetTime";
import type { AppSettingsDto, UsageWindowDto } from "../../types/bridge";
import { formatDurationMinutes } from "../../lib/relativeTime";
import type { TrayCopy } from "./copy";
import { windowLabel } from "./copy";

interface QuotaCardProps {
  window: UsageWindowDto;
  displayMode: AppSettingsDto["displayMode"];
  copy: TrayCopy;
  locale: string;
  timeZone: string;
  showResetTime?: boolean;
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

function quotaStatus(remainingPercent: number): "ready" | "warning" | "critical" {
  if (remainingPercent < 34) return "critical";
  if (remainingPercent < 67) return "warning";
  return "ready";
}

export default function QuotaCard({
  window,
  displayMode,
  copy,
  locale,
  timeZone,
  showResetTime = true,
}: QuotaCardProps) {
  const percent = clampPercent(
    displayMode === "remaining" ? window.remainingPercent : window.usedPercent,
  );
  const displayedPercent = Math.round(percent);
  const modeLabel = displayMode === "remaining" ? copy.remaining : copy.used;
  const label = windowLabel(window.label, window.windowDurationMinutes, locale);
  const resetText = useFormattedResetTime(window.resetsAt, locale, timeZone);
  const duration =
    window.windowDurationMinutes === null
      ? null
      : formatDurationMinutes(window.windowDurationMinutes, locale);
  const accessibleName = [
    label,
    displayedPercent + "% " + modeLabel,
    showResetTime ? resetText : null,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <section className="tray-region quota-card" role="region" aria-label={label}>
      <div className="quota-card__row">
        <div className="quota-card__copy">
          <h2>{label}</h2>
          {showResetTime ? (
            <p className="quota-card__reset">
              {resetText}
              {duration ? " · " + duration : ""}
            </p>
          ) : null}
        </div>
        <p className="quota-card__value">
          <strong>{displayedPercent}% {modeLabel}</strong>
        </p>
      </div>
      <div
        className={"quota-card__progress quota-card--" + quotaStatus(window.remainingPercent)}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={displayedPercent}
        aria-label={accessibleName}
      >
        <span
          className="quota-card__progress-fill"
          style={{ width: percent + "%" }}
          aria-hidden="true"
        />
      </div>
    </section>
  );
}
