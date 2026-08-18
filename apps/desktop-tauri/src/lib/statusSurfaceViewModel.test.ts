import { describe, expect, it } from "vitest";
import type { ProfileUsageStateDto } from "../types/bridge";
import {
  profileUsageFixture,
  readyTwoWindowFixture,
  staleOfflineFixture,
  usageWindow,
  weeklyOnlyUsage,
} from "../test/profileUsageFixtures";
import {
  buildStatusSurfaceViewModel,
  compactIdentity,
  quotaBandFor,
} from "./statusSurfaceViewModel";

function modelFor(
  displayMode: "remaining" | "used" = "remaining",
  stateOverride?: ProfileUsageStateDto,
) {
  const bootstrap = readyTwoWindowFixture();
  const profile = {
    ...bootstrap.profiles[0]!,
    accountDisplayName: "Ming Zhao",
    accountStatus: "signedIn" as const,
  };
  return buildStatusSurfaceViewModel({
    profile,
    state: stateOverride ?? bootstrap.usageByProfile.personal!,
    displayMode,
    nowMs: Date.parse("2026-08-06T02:08:00Z"),
  });
}

describe("buildStatusSurfaceViewModel", () => {
  it("returns only the weekly metric when that is the only backend window", () => {
    const state = weeklyOnlyUsage({
      remainingPercent: 98,
      resetsAt: "2026-08-20T00:00:00Z",
    });
    const model = modelFor("remaining", state);

    expect(model.metrics.map((metric) => metric.kind)).toEqual(["weekly"]);
    expect(model.metrics.some((metric) => metric.label === "5H")).toBe(false);
  });

  it("keeps unknown real windows and removes duplicate limit ids", () => {
    const state = weeklyOnlyUsage({ remainingPercent: 98 });
    state.additionalWindows = [
      { ...state.primary!, remainingPercent: 12, usedPercent: 88 },
      {
        ...state.primary!,
        limitId: "spark",
        label: "Spark",
        windowDurationMinutes: 1_440,
      },
    ];

    expect(modelFor("remaining", state).metrics.map((metric) => metric.limitId)).toEqual([
      "weekly",
      "spark",
    ]);
  });

  it("selects the lowest remaining real window as urgent with stable tie order", () => {
    const state = readyTwoWindowFixture().usageByProfile.personal!;
    state.primary!.remainingPercent = 34;
    state.secondary!.remainingPercent = 34;
    expect(modelFor("remaining", state).urgentMetric?.limitId).toBe(
      state.primary!.limitId,
    );

    state.secondary!.remainingPercent = 33;
    expect(modelFor("remaining", state).urgentMetric?.limitId).toBe(
      state.secondary!.limitId,
    );
  });

  it.each([
    [100, "high"],
    [67, "high"],
    [66, "medium"],
    [34, "medium"],
    [33, "low"],
    [0, "low"],
  ] as const)("maps %s remaining to %s", (remaining, expected) => {
    expect(quotaBandFor(remaining, "trusted")).toBe(expected);
  });

  it("colors cached or stale quota data by remaining percent", () => {
    const state = readyTwoWindowFixture().usageByProfile.personal!;
    expect(modelFor("remaining", { ...state, freshness: "stale" }).trustState).toBe(
      "cached",
    );
    expect(
      modelFor("remaining", { ...state, freshness: "stale" }).metrics[0]?.band,
    ).toBe("medium");
    expect(
      modelFor("remaining", { ...state, refreshStatus: "refreshing" }).metrics[0]
        ?.band,
    ).toBe("medium");
    expect(
      modelFor("remaining", {
        ...state,
        currentError: {
          kind: "offlineOrTimeout",
          userMessageKey: "errors.offlineOrTimeout",
          action: "retry",
          retryAfter: null,
        },
      }).metrics[0]?.band,
    ).toBe("medium");
    expect(
      modelFor("remaining", {
        ...state,
        primary: null,
        secondary: null,
        additionalWindows: [],
      }).trustState,
    ).toBe("missing");
  });

  it("compacts identities by Unicode code point", () => {
    expect(compactIdentity("naipi122899@gmail.com")).toBe("naipi1");
    expect(compactIdentity("😀ab😀cd")).toBe("😀ab😀cd");
  });

  it("identifies five-hour and weekly metrics by duration", () => {
    const bootstrap = readyTwoWindowFixture();
    const profile = bootstrap.profiles[0]!;
    const state = bootstrap.usageByProfile.personal!;

    const model = buildStatusSurfaceViewModel({
      profile,
      state,
      displayMode: "remaining",
      nowMs: Date.parse("2026-08-06T02:08:00Z"),
    });

    expect(model.primaryMetric).toMatchObject({
      kind: "fiveHour",
      displayedPercent: 42,
    });
    expect(model.secondaryMetric).toMatchObject({
      kind: "weekly",
      displayedPercent: 61,
    });
    expect(model.urgentMetric?.kind).toBe("fiveHour");
    expect(model.primaryMetric?.resetText).toBe("2h52m");
  });

  it("uses five-hour as the deterministic urgency tie-breaker", () => {
    const bootstrap = readyTwoWindowFixture();
    const state = bootstrap.usageByProfile.personal!;
    state.secondary = usageWindow(42, {
      limitId: "weekly",
      windowDurationMinutes: 10_080,
    });

    expect(modelFor("remaining", state).urgentMetric?.kind).toBe("fiveHour");
  });

  it("does not relabel an unknown duration", () => {
    const state = profileUsageFixture("personal", 42);
    state.primary = usageWindow(42, {
      limitId: "daily",
      windowDurationMinutes: 1_440,
    });
    state.secondary = null;

    const model = modelFor("remaining", state);
    expect(model.primaryMetric).toBeNull();
    expect(model.secondaryMetric).toBeNull();
  });

  it("finds exact quota durations in additional windows", () => {
    const state = profileUsageFixture("personal", 42);
    state.primary = usageWindow(73, {
      limitId: "daily",
      windowDurationMinutes: 1_440,
    });
    state.secondary = null;
    state.additionalWindows = [
      usageWindow(61, {
        limitId: "weekly",
        windowDurationMinutes: 10_080,
      }),
      usageWindow(42),
    ];

    const model = modelFor("remaining", state);
    expect(model.primaryMetric?.displayedPercent).toBe(42);
    expect(model.secondaryMetric?.displayedPercent).toBe(61);
  });

  it("changes displayed values without changing urgency", () => {
    const remaining = modelFor("remaining");
    const used = modelFor("used");

    expect(remaining.urgentMetric?.kind).toBe("fiveHour");
    expect(used.urgentMetric?.kind).toBe("fiveHour");
    expect(remaining.primaryMetric?.displayedPercent).toBe(42);
    expect(used.primaryMetric?.displayedPercent).toBe(58);
    expect(remaining.primaryMetric?.displayMode).toBe("remaining");
    expect(used.primaryMetric?.displayMode).toBe("used");
  });

  it("preserves identity and cached quota when stale", () => {
    const stale = staleOfflineFixture();
    const model = modelFor("remaining", stale.usageByProfile.personal!);

    expect(model.displayName).toBe("Ming Zhao");
    expect(model.primaryMetric?.displayedPercent).toBe(42);
    expect(model.status).toBe("critical");
    expect(model.freshness).toBe("stale");
  });

  it("never invents zero for missing usage", () => {
    const ready = readyTwoWindowFixture().usageByProfile.personal!;
    const model = modelFor("remaining", {
      ...ready,
      primary: null,
      secondary: null,
      additionalWindows: [],
      freshness: "missing",
    });

    expect(model.urgentMetric).toBeNull();
    expect(model.status).toBe("missing");
    expect(model.refreshStatus).toBe("等待数据");
  });

  it.each([
    [null, "—"],
    ["not-a-date", "—"],
    ["2026-08-06T02:08:00Z", "即将重置"],
    ["2026-08-06T02:37:00Z", "29m"],
    ["2026-08-08T02:07:00Z", "47h59m"],
    ["2026-08-08T02:08:00Z", "2天"],
  ])("formats reset countdown %s deterministically", (resetsAt, expected) => {
    const state = profileUsageFixture("personal", 42);
    state.primary = usageWindow(42, { resetsAt });

    expect(modelFor("remaining", state).primaryMetric?.resetText).toBe(expected);
  });

  it("derives updated text from the injected clock", () => {
    expect(modelFor().updatedText).toBe("2h08m前");
  });
});
