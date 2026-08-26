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
  "last365Days",
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

function formatLongDate(date: string, language: string): string {
  const parsed = new Date(`${date}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return date;
  return new Intl.DateTimeFormat(language === "zh-CN" ? "zh-CN" : "en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(parsed);
}

export function formatTokenCount(value: number, language: string): string {
  const amount = Number.isFinite(value) ? Math.max(0, value) : 0;
  const chinese = language === "zh-CN";
  const units = chinese
    ? ([
        [100_000_000, "亿"],
        [10_000, "万"],
      ] as const)
    : ([
        [1_000_000_000, "B"],
        [1_000_000, "M"],
        [1_000, "K"],
      ] as const);
  for (const [threshold, suffix] of units) {
    if (amount >= threshold) {
      return `${(amount / threshold).toFixed(2).replace(/\.00$|(?<=\.[0-9])0$/, "")}${suffix}`;
    }
  }
  return Math.round(amount).toLocaleString(chinese ? "zh-CN" : "en-US");
}

function recentDateKeys(count: number): string[] {
  const end = new Date();
  end.setHours(12, 0, 0, 0);
  return Array.from({ length: count }, (_, index) => {
    const date = new Date(end);
    date.setDate(end.getDate() - (count - index - 1));
    return [date.getFullYear(), date.getMonth() + 1, date.getDate()]
      .map((part, partIndex) => (partIndex === 0 ? String(part) : String(part).padStart(2, "0")))
      .join("-");
  });
}

export function buildHeatmapCells(count: number): Array<string | null> {
  const dates = recentDateKeys(count);
  const first = dates[0];
  const leading = first
    ? new Date(`${first}T12:00:00`).getDay()
    : 0;
  const totalCells = Math.ceil((leading + dates.length) / 7) * 7;
  return [
    ...Array.from({ length: leading }, () => null),
    ...dates,
    ...Array.from({ length: totalCells - leading - dates.length }, () => null),
  ];
}

function heatmapMonthLabels(
  cells: Array<string | null>,
  language: string,
): Array<{ column: number; label: string }> {
  const formatter = new Intl.DateTimeFormat(
    language === "zh-CN" ? "zh-CN" : "en-US",
    { month: "short" },
  );
  let previousMonth = "";
  return cells.reduce<Array<{ column: number; label: string }>>(
    (labels, date, index) => {
      if (date === null) return labels;
      const month = date.slice(0, 7);
      if (month === previousMonth) return labels;
      previousMonth = month;
      labels.push({
        column: Math.floor(index / 7) + 1,
        label: formatter.format(new Date(`${date}T12:00:00`)),
      });
      return labels;
    },
    [],
  );
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

function heatmapLevel(amount: number | null, maxCost: number): string {
  if (amount == null || amount <= 0 || maxCost <= 0) return "neutral";
  return `cost-${Math.min(4, Math.max(1, Math.ceil((amount / maxCost) * 4)))}`;
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
            <span>{usageCopy.inputTokens}: <strong>{formatTokenCount(local?.inputTokens ?? 0, language)}</strong></span>
            <span>{usageCopy.cachedInputTokens}: <strong>{formatTokenCount(local?.cachedInputTokens ?? 0, language)}</strong></span>
            <span>{usageCopy.outputTokens}: <strong>{formatTokenCount(local?.outputTokens ?? 0, language)}</strong></span>
            <span>{usageCopy.totalTokens}: <strong>{formatTokenCount(local?.totalTokens ?? 0, language)}</strong></span>
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

          {local && local.state !== "unavailable" && local.state !== "cancelled" ? (
            <>
              <p className="settings-preference-group__subheading">
                {usageCopy.heatmapTitle}
              </p>
              <p className="settings-preference-group__description">
                {usageCopy.heatmapDescription}
              </p>
              <div className="usage-spend-heatmap-wrap">
                {(() => {
                  const byDate = new Map(local.activity.map((row) => [row.date, row]));
                  const maxCost = Math.max(
                    0,
                    ...local.activity.map((row) => row.estimatedCost.amount ?? 0),
                  );
                  const cells = buildHeatmapCells(365);
                  return (
                    <>
                      <div className="usage-spend-heatmap__months" aria-hidden="true">
                        {heatmapMonthLabels(cells, language).map(({ column, label }) => (
                          <span key={`${column}-${label}`} style={{ gridColumn: column }}>
                            {label}
                          </span>
                        ))}
                      </div>
                      <div className="usage-spend-heatmap" role="grid" aria-label={usageCopy.heatmapTitle}>
                        {cells.map((date, index) => {
                          if (date === null) {
                            return <span key={`empty-${index}`} className="usage-spend-heatmap__cell usage-spend-heatmap__cell--empty" aria-hidden="true" />;
                          }
                          const row = byDate.get(date);
                          const cost = formatCost(row?.estimatedCost, usageCopy);
                          const tokens = formatTokenCount(row?.totalTokens ?? 0, language);
                          const label = usageCopy.heatmapCell(
                            formatLongDate(date, language),
                            cost,
                            tokens,
                          );
                          const level = heatmapLevel(
                            row?.estimatedCost.amount ?? null,
                            maxCost,
                          );
                          return (
                            <span
                              key={date}
                              role="gridcell"
                              tabIndex={0}
                              title={label}
                              aria-label={label}
                              className={`usage-spend-heatmap__cell usage-spend-heatmap__cell--${level === "neutral" ? "neutral" : "cost"} ${level === "neutral" ? "" : `usage-spend-heatmap__cell--${level}`}`}
                            />
                          );
                        })}
                      </div>
                    </>
                  );
                })()}
              </div>
            </>
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
                      <td>{formatTokenCount(row.totalTokens, language)}</td>
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
                      <td>{formatTokenCount(row.inputTokens, language)}</td>
                      <td>{formatTokenCount(row.cachedInputTokens, language)}</td>
                      <td>{formatTokenCount(row.outputTokens, language)}</td>
                      <td>{formatTokenCount(row.totalTokens, language)}</td>
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
