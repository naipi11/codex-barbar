import { describe, expect, it, vi } from "vitest";
import type { UseStatusSurfaceResult } from "../hooks/useStatusSurface";
import { buildStatusSurfaceViewModel } from "../lib/statusSurfaceViewModel";
import {
  bootstrapWithTwoProfiles,
  readyTwoWindowFixture,
  staleOfflineFixture,
  weeklyOnlyUsage,
} from "../test/profileUsageFixtures";
import { buildTaskbarStatusPresentation } from "./taskbarStatusPresentation";

function surfaceFrom(
  bootstrap = bootstrapWithTwoProfiles(),
): UseStatusSurfaceResult {
  const profile = bootstrap.profiles[0]!;
  const state = bootstrap.usageByProfile.personal!;
  const model = buildStatusSurfaceViewModel({
    profile,
    state,
    displayMode: bootstrap.settings.displayMode,
    nowMs: Date.parse("2026-08-14T00:00:00Z"),
  });

  return {
    ...model,
    bootstrap,
    profile,
    state,
    isDragging: false,
    closeFailedBySurface: {
      taskbarStatus: false,
      floatBall: false,
    },
    setIsDragging: vi.fn(),
    openPanel: vi.fn(async () => {}),
    disableSurface: vi.fn(async () => {}),
    setFloatBallExpanded: vi.fn(async () => {}),
  };
}

function weeklySurface(): UseStatusSurfaceResult {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.profiles[0]!.accountDisplayName = "ProofUser";
  bootstrap.usageByProfile.personal = weeklyOnlyUsage();
  return surfaceFrom(bootstrap);
}

describe("buildTaskbarStatusPresentation", () => {
  it("derives the weekly proof fields once for visible and measurement routes", () => {
    const presentation = buildTaskbarStatusPresentation(weeklySurface());

    expect(presentation.displayName).toBe("ProofUser");
    expect(presentation.compactIdentity).toBe("ProofU");
    expect(presentation.reset?.resetsAt).toBe("2026-08-20T00:00:00Z");
    expect(presentation.surfaceAlpha).toBe("0.2");
    expect(presentation.ariaLabel).toBe(
      "打开完整面板，ProofUser，周 98%，6天，已更新，8天前",
    );
  });

  it("keeps every real window in backend order and selects the nearest reset", () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.usageByProfile.personal!.primary!.resetsAt =
      "2026-08-21T00:00:00Z";
    bootstrap.usageByProfile.personal!.secondary!.resetsAt =
      "2026-08-20T00:00:00Z";
    bootstrap.usageByProfile.personal!.additionalWindows = [
      {
        limitId: "spark",
        label: "Spark",
        usedPercent: 12,
        remainingPercent: 88,
        windowDurationMinutes: 1_440,
        resetsAt: "2026-08-19T00:00:00Z",
        reachedType: null,
      },
    ];

    const presentation = buildTaskbarStatusPresentation(surfaceFrom(bootstrap));

    expect(presentation.metrics.map((metric) => metric.limitId)).toEqual([
      "five-hour",
      "weekly",
      "spark",
    ]);
    expect(presentation.reset?.limitId).toBe("spark");
    expect(presentation.ariaLabel).toContain("5H 42%，周 61%，Spark 88%");
  });

  it.each([
    ["remaining", "周 98%"],
    ["used", "周 2%"],
  ] as const)(
    "formats %s mode once in the shared aria label",
    (displayMode, text) => {
      const bootstrap = bootstrapWithTwoProfiles();
      bootstrap.settings.displayMode = displayMode;
      bootstrap.usageByProfile.personal = weeklyOnlyUsage();

      expect(
        buildTaskbarStatusPresentation(surfaceFrom(bootstrap)).ariaLabel,
      ).toContain(text);
    },
  );

  it("announces cached data and update age from the shared model", () => {
    const presentation = buildTaskbarStatusPresentation(
      surfaceFrom(staleOfflineFixture()),
    );

    expect(presentation.trustState).toBe("cached");
    expect(presentation.ariaLabel).toContain("缓存数据");
    expect(presentation.ariaLabel).toContain("8天前");
  });

  it.each([
    [-20, "0"],
    [0, "0"],
    [20, "0.2"],
    [100, "1"],
    [140, "1"],
  ])("clamps taskbar opacity %s to background alpha %s", (opacity, alpha) => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.taskbarStatusOpacity = opacity;

    expect(
      buildTaskbarStatusPresentation(surfaceFrom(bootstrap)).surfaceAlpha,
    ).toBe(alpha);
  });
});
