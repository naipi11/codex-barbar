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
    language: bootstrap.settings.language,
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
    expect(presentation.surfaceAlpha).toBe("0.8");
    expect(presentation.ariaLabel).toBe(
      "打开完整面板，ProofUser，Wk 98%，6天，已更新，8天前",
    );
  });

  it("keeps only the universal weekly window and its reset", () => {
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
      "weekly",
    ]);
    expect(presentation.reset?.limitId).toBe("weekly");
    expect(presentation.ariaLabel).toContain("Wk 61%");
    expect(presentation.ariaLabel).not.toContain("5H");
    expect(presentation.ariaLabel).not.toContain("Spark");
  });

  it.each([
    ["remaining", "Wk 98%"],
    ["used", "Wk 2%"],
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
    const bootstrap = staleOfflineFixture();
    bootstrap.usageByProfile.personal!.secondary = weeklyOnlyUsage({
      remainingPercent: 42,
      usedPercent: 58,
    }).primary;
    const presentation = buildTaskbarStatusPresentation(surfaceFrom(bootstrap));

    expect(presentation.trustState).toBe("cached");
    expect(presentation.ariaLabel).toContain("缓存数据");
    expect(presentation.ariaLabel).toContain("8天前");
  });

  it.each([
    [-20, "1"],
    [0, "1"],
    [20, "0.8"],
    [80, "0.2"],
    [100, "0"],
    [140, "0"],
  ])("maps taskbar transparency %s to background alpha %s", (opacity, alpha) => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.taskbarTransparencyPercent = opacity;

    expect(
      buildTaskbarStatusPresentation(surfaceFrom(bootstrap)).surfaceAlpha,
    ).toBe(alpha);
  });

  it("exposes preference-aware fields with default all-visible behavior", () => {
    const presentation = buildTaskbarStatusPresentation(weeklySurface());

    expect(presentation.showIcon).toBe(true);
    expect(presentation.showAccount).toBe(true);
    expect(presentation.showWeeklyLabel).toBe(true);
    expect(presentation.showWeeklyPercent).toBe(true);
    expect(presentation.showResetDate).toBe(true);
    expect(presentation.density).toBe("compact");
    expect(presentation.compactIdentity).toBe("ProofU");
    expect(presentation.weeklyText).toBe("Wk 98%");
    expect(presentation.resetDateText).toBe("8/20");
  });

  it("filters every taskbar field from the shared presentation", () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "ProofUser";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    bootstrap.settings.taskbarPresentation = {
      showTaskbarIcon: false,
      showTaskbarAccount: false,
      showWeeklyLabel: false,
      showWeeklyPercent: false,
      showResetDate: false,
      density: "standard",
      hideStatusSurfacesInFullscreen: true,
    };

    const presentation = buildTaskbarStatusPresentation(surfaceFrom(bootstrap));

    expect(presentation.showIcon).toBe(false);
    expect(presentation.showAccount).toBe(false);
    expect(presentation.showWeeklyLabel).toBe(false);
    expect(presentation.showWeeklyPercent).toBe(false);
    expect(presentation.showResetDate).toBe(false);
    expect(presentation.density).toBe("standard");
    expect(presentation.compactIdentity).toBeNull();
    expect(presentation.weeklyText).toBeNull();
    expect(presentation.resetDateText).toBeNull();
  });

  it("combines label and percent independently", () => {
    const base = weeklySurface();
    const prefs = base.bootstrap!.settings.taskbarPresentation;

    const labelOnly = buildTaskbarStatusPresentation({
      ...base,
      bootstrap: {
        ...base.bootstrap!,
        settings: { ...base.bootstrap!.settings, taskbarPresentation: { ...prefs, showWeeklyPercent: false } },
      },
    });
    expect(labelOnly.weeklyText).toBe("Wk");

    const percentOnly = buildTaskbarStatusPresentation({
      ...base,
      bootstrap: {
        ...base.bootstrap!,
        settings: { ...base.bootstrap!.settings, taskbarPresentation: { ...prefs, showWeeklyLabel: false } },
      },
    });
    expect(percentOnly.weeklyText).toBe("98%");
  });
});
