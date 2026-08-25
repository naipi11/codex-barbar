import { useCallback, useEffect, useRef, useState } from "react";
import { getUsageSpend } from "../../../lib/tauri";
import type {
  UsageSpendDto,
  UsageSpendRange,
} from "../../../types/bridge";
import type { SettingsCopy } from "../settingsCopy";

const RANGES: readonly UsageSpendRange[] = [
  "today",
  "last7Days",
  "last30Days",
  "currentWeekly",
];

function formatDate(date: string, language: string): string {
  const parsed = new Date(`${date}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return date;
  return new Intl.DateTimeFormat(language === "zh-CN" ? "zh-CN" : "en-US", {
    month: "short",
    day: "numeric",
  }).format(parsed);
}

function formatDateTime(value: string | null, language: string): string {
  if (!value) return "—";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(language === "zh-CN" ? "zh-CN" : "en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(parsed);
}

function formatCost(cost: { amount: number | null; provenance: string } | undefined, usageCopy: SettingsCopy["usageSpend"]): string {
  if (!cost || cost.amount == null) return usageCopy.costUnknown;
  const amount = usageCopy.costUsd(cost.amount);
  return cost.provenance === "officialEquivalent" ? `${amount} (${usageCopy.officialEquivalent})` : amount;
}

export default function UsageSpendTab({
  copy,
  language,
}: {
  copy: SettingsCopy;
  language: string;
}) {
  const usageCopy = copy.usageSpend;
  const [range, setRange] = useState<UsageSpendRange>("last7Days");
  const [data, setData] = useState<UsageSpendDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(false);
  const requestId = useRef(0);

  const load = useCallback(
    async (nextRange: UsageSpendRange, force = false) => {
      const current = ++requestId.current;
      setLoading(true);
      setFailed(false);
      try {
        const next = await getUsageSpend(nextRange);
        if (requestId.current === current) {
          setData(next);
          setLoading(false);
        }
      } catch {
        if (requestId.current === current) {
          setFailed(true);
          setLoading(false);
        }
      }
      void force;
    },
    [],
  );

  useEffect(() => {
    void load(range);
  }, [load, range]);

  if (loading && !data) {
    return (
      <section data-testid="usage-spend-tab" aria-label={usageCopy.title}>
        <h2>{usageCopy.title}</h2>
        <p aria-live="polite">{usageCopy.loading}</p>
      </section>
    );
  }

  if (failed && !data) {
    return (
      <section data-testid="usage-spend-tab" aria-label={usageCopy.title}>
        <h2>{usageCopy.title}</h2>
        <p className="settings-preference-group__error" role="alert">
          {usageCopy.loadFailed}
        </p>
        <button
          type="button"
          className="settings-button"
          onClick={() => void load(range, true)}
        >
          {usageCopy.refreshLocal}
        </button>
      </section>
    );
  }

  const official = data?.official;
  const local = data?.local;

  return (
    <section data-testid="usage-spend-tab" aria-label={usageCopy.title}>
      <h2>{usageCopy.title}</h2>
      <div className="settings-preference-groups">
        <fieldset className="settings-preference-group">
          <legend>{usageCopy.officialTitle}</legend>
          <p className="settings-preference-group__description">
            {usageCopy.officialDescription}
          </p>
          <div className="usage-spend-official">
            <span className="usage-spend-official__label">
              {usageCopy.weeklyAllowance}
            </span>
            <strong className="usage-spend-official__percent">
              {official?.remainingPercent == null
                ? "—"
                : usageCopy.remainingPercent(official.remainingPercent)}
            </strong>
            <span>
              {usageCopy.resetsAt}:{" "}
              {formatDateTime(official?.resetsAt ?? null, language)}
            </span>
            <span>
              {usageCopy.lastUpdated}:{" "}
              {formatDateTime(official?.fetchedAt ?? null, language)}
            </span>
            <span>
              {official
                ? usageCopy.freshness[official.freshness]
                : usageCopy.freshness.missing}
            </span>
          </div>
          <p className="settings-preference-group__subheading">
            {usageCopy.resetCreditsTitle}
          </p>
          <p>
            {official?.resetCredits.state === "available"
              ? usageCopy.resetCreditsAvailable(
                  official.resetCredits.availableCount ?? 0,
                )
              : official?.resetCredits.state === "stale"
                ? usageCopy.resetCreditsStale
                : usageCopy.resetCreditsUnsupported}
          </p>
        </fieldset>

        <fieldset className="settings-preference-group">
          <legend>{usageCopy.localTitle}</legend>
          <p className="settings-preference-group__description">
            {usageCopy.deviceCombined} ·{" "}
            <span className="usage-spend-badge">
              {usageCopy.localEstimateBadge}
            </span>
          </p>

          <div className="settings-preference-grid">
            <label className="settings-compact-field" htmlFor="usage-spend-range">
              <span>{usageCopy.rangeLabel}</span>
              <select
                id="usage-spend-range"
                value={range}
                onChange={(event) =>
                  setRange(event.target.value as UsageSpendRange)
                }
              >
                {RANGES.map((value, index) => (
                  <option key={value} value={value}>
                    {usageCopy.ranges[index]}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className="settings-button"
              disabled={loading}
              onClick={() => void load(range, true)}
            >
              {loading ? usageCopy.refreshingLocal : usageCopy.refreshLocal}
            </button>
          </div>

          {local?.state === "unavailable" ? (
            <p className="settings-preference-group__hint">
              {usageCopy.unavailableState}
            </p>
          ) : null}
          {local?.state === "cancelled" ? (
            <p className="settings-preference-group__hint">
              {usageCopy.cancelledState}
            </p>
          ) : null}
          {local?.state === "empty" ? (
            <p className="settings-preference-group__hint">
              {usageCopy.emptyState}
            </p>
          ) : null}

          <div className="usage-spend-token-grid" aria-live="polite">
            <span>{usageCopy.inputTokens}: <strong>{local?.inputTokens ?? 0}</strong></span>
            <span>{usageCopy.cachedInputTokens}: <strong>{local?.cachedInputTokens ?? 0}</strong></span>
            <span>{usageCopy.outputTokens}: <strong>{local?.outputTokens ?? 0}</strong></span>
            <span>{usageCopy.totalTokens}: <strong>{local?.totalTokens ?? 0}</strong></span>
            <span>{usageCopy.sessions}: <strong>{local?.sessionsCount ?? 0}</strong></span>
            <span>
              {usageCopy.cost}:{" "}
              <strong>
                {local?.partialEstimate
                  ? `${usageCopy.partialEstimate}: ${formatCost(local.estimatedCost, usageCopy)}`
                  : formatCost(local?.estimatedCost, usageCopy)}
              </strong>
            </span>
          </div>

          {local?.malformedRecordsSkipped ? (
            <p className="settings-preference-group__hint">
              {usageCopy.malformedSkipped(local.malformedRecordsSkipped)}
            </p>
          ) : null}

          {local && local.daily.length > 0 ? (
            <>
              <p className="settings-preference-group__subheading">
                {usageCopy.dailyTrendTitle}
              </p>
              <table className="usage-spend-table">
                <thead>
                  <tr>
                    <th scope="col">{usageCopy.dateColumn}</th>
                    <th scope="col">{usageCopy.totalTokens}</th>
                    <th scope="col">{usageCopy.cost}</th>
                  </tr>
                </thead>
                <tbody>
                  {local.daily.map((row) => (
                    <tr key={row.date}>
                      <td>{formatDate(row.date, language)}</td>
                      <td>{row.totalTokens}</td>
                      <td>
                        {formatCost(row.estimatedCost, usageCopy)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          ) : null}

          {local && local.models.length > 0 ? (
            <>
              <p className="settings-preference-group__subheading">
                {usageCopy.modelTableTitle}
              </p>
              <table className="usage-spend-table">
                <thead>
                  <tr>
                    <th scope="col">{usageCopy.modelColumn}</th>
                    <th scope="col">{usageCopy.inputTokens}</th>
                    <th scope="col">{usageCopy.cachedInputTokens}</th>
                    <th scope="col">{usageCopy.outputTokens}</th>
                    <th scope="col">{usageCopy.totalTokens}</th>
                    <th scope="col">{usageCopy.cost}</th>
                  </tr>
                </thead>
                <tbody>
                  {local.models.map((row) => (
                    <tr key={row.model}>
                      <td>{row.model}</td>
                      <td>{row.inputTokens}</td>
                      <td>{row.cachedInputTokens}</td>
                      <td>{row.outputTokens}</td>
                      <td>{row.totalTokens}</td>
                      <td>
                        {formatCost(row.estimatedCost, usageCopy)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          ) : null}

          {local && local.unknownModels.length > 0 ? (
            <p className="settings-preference-group__hint">
              <strong>{usageCopy.unknownModelsTitle}:</strong>{" "}
              {local.unknownModels.join(", ")}. {usageCopy.unknownModelsHelp}
            </p>
          ) : null}
        </fieldset>
      </div>
    </section>
  );
}
