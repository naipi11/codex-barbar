import type { AppErrorKind, ProfileUsageStateDto, RecoveryAction } from "../../types/bridge";
import type { TrayCopy } from "./copy";

interface UsageStatusProps {
  state: ProfileUsageStateDto;
  isSwitching: boolean;
  copy: TrayCopy;
  locale: string;
  showFreshness?: boolean;
  onRefresh(): Promise<void> | void;
  onOpenSettings(): Promise<void> | void;
  onOpenUsage(): Promise<void> | void;
}

function formatUpdatedAt(value: string | null, locale: string): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return null;
  try {
    return new Intl.DateTimeFormat(locale, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
  } catch {
    return date.toISOString();
  }
}

function errorAction(
  kind: AppErrorKind,
  action: RecoveryAction,
  copy: TrayCopy,
): { message: string; label: string; kind: "refresh" | "settings" | "usage" | "none" } {
  const kindLabel = copy.errorMessages[kind];
  switch (action) {
    case "retry":
      return { message: kindLabel, label: copy.retry, kind: "refresh" };
    case "waitAndRetry":
      return { message: kindLabel, label: copy.waitAndRetry, kind: "none" };
    case "explainApiBilling":
      return { message: kindLabel, label: copy.explainApiBilling, kind: "usage" };
    case "signIn":
      return { message: kindLabel, label: copy.signIn, kind: "settings" };
    case "reloginManagedProfile":
      return { message: kindLabel, label: copy.reLogin, kind: "settings" };
    case "selectCodexExecutable":
      return { message: kindLabel, label: copy.selectExecutable, kind: "settings" };
    case "installTestedCodex":
      return { message: kindLabel, label: copy.installTestedCodex, kind: "settings" };
    case "exportDiagnostics":
      return { message: kindLabel, label: copy.exportDiagnostics, kind: "none" };
  }
}

export default function UsageStatus({
  state,
  isSwitching,
  copy,
  locale,
  showFreshness = true,
  onRefresh,
  onOpenSettings,
  onOpenUsage,
}: UsageStatusProps) {
  const updated = formatUpdatedAt(state.fetchedAt, locale);
  const error = state.currentError
    ? errorAction(state.currentError.kind, state.currentError.action, copy)
    : null;
  const showProtocolAnomaly =
    state.protocolAnomaly && !state.primary && !state.secondary;
  const showTransientState =
    isSwitching ||
    state.refreshStatus === "refreshing" ||
    state.refreshStatus === "cooldown";

  if (!showFreshness && !showTransientState && !showProtocolAnomaly && !error) {
    return null;
  }

  return (
    <section className="tray-region usage-status" role="region" aria-label={copy.dataStatus}>
      <h2>{copy.dataStatus}</h2>
      {isSwitching ? (
        <p className="usage-status__state">{copy.switching}</p>
      ) : null}
      {state.refreshStatus === "refreshing" ? (
        <p className="usage-status__state">{copy.refreshing}</p>
      ) : null}
      {state.refreshStatus === "cooldown" ? (
        <p className="usage-status__state">{copy.waitAndRetry}</p>
      ) : null}
      {showFreshness && state.freshness === "missing" ? (
        <p className="usage-status__state">{copy.missing}</p>
      ) : null}
      {showFreshness && state.freshness === "stale" && !error ? (
        <p className="usage-status__state">{copy.cached}</p>
      ) : null}
      {showFreshness && state.freshness === "fresh" && !error ? (
        <p className="usage-status__state">{copy.fresh}</p>
      ) : null}
      {showFreshness && updated ? (
        <p className="usage-status__updated">
          {copy.lastUpdated}: {updated}
        </p>
      ) : null}
      {showProtocolAnomaly ? (
        <p className="usage-status__anomaly">{copy.protocolAnomaly}</p>
      ) : null}
      {error ? (
        <div className="usage-status__error">
          <p role="alert">{error.message}</p>
          {error.kind === "refresh" ? (
            <button type="button" onClick={() => void onRefresh()}>
              {error.label}
            </button>
          ) : null}
          {error.kind === "settings" ? (
            <button type="button" onClick={() => void onOpenSettings()}>
              {error.label}
            </button>
          ) : null}
          {error.kind === "usage" ? (
            <button type="button" onClick={() => void onOpenUsage()}>
              {error.label}
            </button>
          ) : null}
          {error.kind === "none" ? (
            <span className="usage-status__action">{error.label}</span>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
