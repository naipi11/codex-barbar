import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getUsageSpend } from "../../../lib/tauri";
import type {
  LocalUsageSpendDto,
  OfficialUsageDto,
  UsageSpendDto,
  UsageSpendRange,
} from "../../../types/bridge";
import { settingsCopy } from "../settingsCopy";
import UsageSpendTab, { buildHeatmapCells, formatTokenCount } from "./UsageSpendTab";

vi.mock("../../../lib/tauri", () => ({
  getUsageSpend: vi.fn(),
}));

const getUsageSpendMock = vi.mocked(getUsageSpend);

function official(overrides: Partial<OfficialUsageDto> = {}): OfficialUsageDto {
  return {
    remainingPercent: 66,
    resetsAt: "2026-08-30T00:00:00Z",
    fetchedAt: "2026-08-23T01:02:03Z",
    freshness: "fresh",
    resetCredits: { state: "available", availableCount: 2, observedAt: "2026-08-23T01:02:03Z" },
    ...overrides,
  };
}

function local(overrides: Partial<LocalUsageSpendDto> = {}): LocalUsageSpendDto {
  return {
    attribution: "deviceCombined",
    range: "last7Days",
    inputTokens: 125,
    cachedInputTokens: 25,
    outputTokens: 15,
    totalTokens: 165,
    sessionsCount: 2,
    estimatedCost: { amount: 0.42, currency: "USD", provenance: "officialDirect", canonicalModel: null, sourceUpdatedAt: null }, displayCurrency: "USD", pricingStatus: "partial", partialEstimate: true, unpricedModelCount: 1,
    unknownModels: [],
    daily: [
      { date: "2026-08-20", totalTokens: 80, estimatedCost: { amount: 0.2, currency: "USD", provenance: "officialDirect", canonicalModel: null, sourceUpdatedAt: null } },
      { date: "2026-08-21", totalTokens: 85, estimatedCost: { amount: 0.22, currency: "USD", provenance: "officialDirect", canonicalModel: null, sourceUpdatedAt: null } },
    ],
    activity: [
      { date: "2026-08-20", totalTokens: 80, estimatedCost: { amount: 0.2, currency: "USD", provenance: "officialDirect", canonicalModel: null, sourceUpdatedAt: null } },
      { date: "2026-08-21", totalTokens: 85, estimatedCost: { amount: 0.22, currency: "USD", provenance: "officialDirect", canonicalModel: null, sourceUpdatedAt: null } },
    ],
    models: [
      {
        model: "gpt-5",
        inputTokens: 100,
        cachedInputTokens: 25,
        outputTokens: 10,
        totalTokens: 135,
        estimatedCost: { amount: 0.4, currency: "USD", provenance: "officialEquivalent", canonicalModel: "gpt-5.6-sol", sourceUpdatedAt: null },
      },
      {
        model: "gpt-mystery",
        inputTokens: 25,
        cachedInputTokens: 0,
        outputTokens: 5,
        totalTokens: 30,
        estimatedCost: { amount: null, currency: "USD", provenance: "unpriced", canonicalModel: null, sourceUpdatedAt: null },
      },
    ],
    state: "ready",
    malformedRecordsSkipped: 1,
    ...overrides,
  };
}

function dto(overrides: Partial<UsageSpendDto> = {}): UsageSpendDto {
  return { official: official(), local: local(), ...overrides };
}

function renderWith(
  payload: UsageSpendDto,
  language: "en-US" | "zh-CN" = "en-US",
) {
  getUsageSpendMock.mockResolvedValue(payload);
  return render(<UsageSpendTab copy={settingsCopy(language)} language={language} />);
}

describe("UsageSpendTab", () => {
  beforeEach(() => {
    getUsageSpendMock.mockReset();
  });

  it("renders the read-only dashboard with honest labels and no reset action", async () => {
    renderWith(dto());

    expect(await screen.findByTestId("usage-spend-tab")).toBeInTheDocument();
    expect(
      screen.getByText("Local estimate, not an OpenAI bill"),
    ).toBeInTheDocument();
    expect(screen.getByText(/This device combined/)).toBeInTheDocument();
    expect(screen.getByText("2 reset credit(s) available")).toBeInTheDocument();
    expect(screen.getByText("66% remaining")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /use reset/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /redeem/i })).not.toBeInTheDocument();
    expect(screen.getByText(/1 malformed log line\(s\) skipped/i)).toBeInTheDocument();
  });

  it("distinguishes zero, unsupported, and stale reset credits", async () => {
    const first = renderWith(
      dto({ official: official({ resetCredits: { state: "available", availableCount: 0, observedAt: null } }) }),
    );
    expect(await screen.findByText("0 reset credit(s) available")).toBeInTheDocument();
    first.unmount();

    const second = renderWith(
      dto({ official: official({ resetCredits: { state: "unsupported", availableCount: null, observedAt: null } }) }),
    );
    expect(
      await screen.findByText("Reset credits are not reported for this account."),
    ).toBeInTheDocument();
    second.unmount();

    renderWith(
      dto({ official: official({ resetCredits: { state: "stale", availableCount: 1, observedAt: "2026-08-22T00:00:00Z" } }) }),
    );
    expect(
      await screen.findByText("Reset-credit count is from a cached snapshot."),
    ).toBeInTheDocument();
  });

  it("renders unknown models without a guessed aggregate cost", async () => {
    renderWith(
      dto({
        local: local({
          estimatedCost: { amount: null, currency: "USD", provenance: "unpriced", canonicalModel: null, sourceUpdatedAt: null },
          unknownModels: ["gpt-mystery"],
          models: [
            {
              model: "gpt-mystery",
              inputTokens: 25,
              cachedInputTokens: 0,
              outputTokens: 5,
              totalTokens: 30,
              estimatedCost: { amount: null, currency: "USD", provenance: "unpriced", canonicalModel: null, sourceUpdatedAt: null },
            },
          ],
        }),
      }),
    );

    expect(await screen.findByText(/Unpriced models/)).toBeInTheDocument();
    expect(screen.getAllByText(/gpt-mystery/).length).toBeGreaterThan(0);
    const costCells = screen.getAllByText("—");
    expect(costCells.length).toBeGreaterThan(0);
  });

  it("shows empty and cancelled local states", async () => {
    const first = renderWith(
      dto({ local: local({ state: "empty", sessionsCount: 0, totalTokens: 0 }) }),
    );
    expect(
      await screen.findByText("No local Codex session logs found in this range."),
    ).toBeInTheDocument();
    first.unmount();

    renderWith(
      dto({ local: local({ state: "cancelled", sessionsCount: 0, totalTokens: 0 }) }),
    );
    expect(await screen.findByText("Local scan was cancelled.")).toBeInTheDocument();
  });

  it("requests a different range when the selector changes", async () => {
    getUsageSpendMock.mockResolvedValue(dto());
    render(<UsageSpendTab copy={settingsCopy("en-US")} language="en-US" />);
    await screen.findByTestId("usage-spend-tab");

    fireEvent.change(screen.getByRole("combobox", { name: "Local range" }), {
      target: { value: "today" },
    });

    await waitFor(() =>
      expect(getUsageSpendMock).toHaveBeenCalledWith("today"),
    );
  });

  it("labels a mapped gateway row as an official-equivalent estimate", async () => {
    getUsageSpendMock.mockResolvedValue(dto());
    render(<UsageSpendTab copy={settingsCopy("en-US")} language="en-US" />);
    await screen.findByTestId("usage-spend-tab");
    expect(screen.getByText(/Official-equivalent estimate/)).toBeInTheDocument();
    expect(screen.getAllByRole("columnheader", { name: "Cost" }).length).toBeGreaterThan(0);
  });

  it("sorts daily rows and model rows deterministically", async () => {
    getUsageSpendMock.mockResolvedValue(dto());
    render(<UsageSpendTab copy={settingsCopy("en-US")} language="en-US" />);
    await screen.findByTestId("usage-spend-tab");

    const rows = within(screen.getByText("Daily trend").closest("fieldset")!).getAllByRole("row");
    expect(rows[1]).toHaveTextContent("Aug 20");
    expect(rows[2]).toHaveTextContent("Aug 21");
  });

  it("localizes the dashboard in Simplified Chinese", async () => {
    renderWith(dto(), "zh-CN");

    expect(await screen.findByRole("heading", { name: "用量与费用" })).toBeInTheDocument();
    expect(screen.getByText("本地估算，并非 OpenAI 账单")).toBeInTheDocument();
    expect(screen.getByText(/此设备合计/)).toBeInTheDocument();
    expect(screen.getByText("可用重置额度 2 个")).toBeInTheDocument();
    expect(screen.getByText("剩余 66%")).toBeInTheDocument();
  });

  it("shows a localized load failure with a retry", async () => {
    getUsageSpendMock.mockRejectedValueOnce(new Error("raw failure"))
      .mockResolvedValueOnce(dto());
    render(<UsageSpendTab copy={settingsCopy("en-US")} language="en-US" />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Usage data could not be loaded. Try again.",
    );
    expect(screen.queryByText("raw failure")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Refresh local data" }));
    expect(await screen.findByText("66% remaining")).toBeInTheDocument();
  });

  it("renders a continuous 365-day cost heatmap with accessible day details", async () => {
    const dateForDaysAgo = (daysAgo: number) => {
      const date = new Date();
      date.setHours(12, 0, 0, 0);
      date.setDate(date.getDate() - daysAgo);
      return [date.getFullYear(), date.getMonth() + 1, date.getDate()]
        .map((part, index) => (index === 0 ? String(part) : String(part).padStart(2, "0")))
        .join("-");
    };
    const paidDate = dateForDaysAgo(6);
    const neutralDate = dateForDaysAgo(5);
    const paidLabel = new Intl.DateTimeFormat("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    }).format(new Date(`${paidDate}T00:00:00`));
    const neutralLabel = new Intl.DateTimeFormat("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    }).format(new Date(`${neutralDate}T00:00:00`));
    renderWith(
      dto({
        local: local({
          range: "last7Days",
          daily: [],
          activity: [
            {
              date: paidDate,
              totalTokens: 80,
              estimatedCost: {
                amount: 0.2,
                currency: "USD",
                provenance: "officialDirect",
                canonicalModel: null,
                sourceUpdatedAt: null,
              },
            },
            {
              date: neutralDate,
              totalTokens: 1_234_567,
              estimatedCost: {
                amount: null,
                currency: "USD",
                provenance: "unpriced",
                canonicalModel: null,
                sourceUpdatedAt: null,
              },
            },
          ],
        }),
      }),
    );

    await screen.findByTestId("usage-spend-tab");
    expect(getUsageSpendMock).toHaveBeenCalledWith("last7Days");
    const grid = screen.queryByRole("grid", { name: "Daily cost heatmap" });
    expect(grid).toBeInTheDocument();
    if (!grid) return;
    expect(within(grid).getAllByRole("gridcell")).toHaveLength(365);
    expect(
      within(grid).getByRole("gridcell", {
        name: new RegExp(`${paidLabel}.*\\$0\\.20.*80 tokens`, "i"),
      }),
    ).toHaveClass("usage-spend-heatmap__cell--cost");
    expect(
      within(grid).getByRole("gridcell", {
        name: new RegExp(`${neutralLabel}.*—.*1\\.23M tokens`, "i"),
      }),
    ).toHaveClass("usage-spend-heatmap__cell--neutral");
  });

  it("formats token totals with compact English and Chinese units", async () => {
    expect(formatTokenCount(1_234, "en-US")).toBe("1.23K");
    expect(formatTokenCount(1_234_567_890, "en-US")).toBe("1.23B");
    expect(formatTokenCount(12_345, "zh-CN")).toBe("1.23万");
    renderWith(
      dto({
        local: local({
          inputTokens: 1_234_567,
          cachedInputTokens: 10_000,
          outputTokens: 100_000_000,
          totalTokens: 101_244_567,
        }),
      }),
    );
    await screen.findByTestId("usage-spend-tab");
    expect(screen.getByText("101.24M")).toBeInTheDocument();

    const zh = renderWith(
      dto({
        local: local({
          inputTokens: 1_234_567,
          cachedInputTokens: 10_000,
          outputTokens: 100_000_000,
          totalTokens: 123_456_789,
        }),
      }),
      "zh-CN",
    );
    await within(zh.container).findByTestId("usage-spend-tab");
    expect(within(zh.container).getByText("1.23亿")).toBeInTheDocument();
    zh.unmount();
  });

  it("pads the heatmap to complete Sunday-first calendar weeks", () => {
    const cells = buildHeatmapCells(365);
    expect(cells.length % 7).toBe(0);
    expect(cells.filter((date): date is string => date !== null)).toHaveLength(365);
    const firstDate = cells.find((date): date is string => date !== null);
    expect(firstDate).toBeTruthy();
    expect(cells.indexOf(firstDate ?? "")).toBe(
      new Date(`${firstDate}T12:00:00`).getDay(),
    );
  });
});
