import { describe, expect, it } from "vitest";
import { surfaceAlphaFromTransparency } from "./surfaceTransparency";

describe("surfaceAlphaFromTransparency", () => {
  it.each([
    [0, 1],
    [20, 0.8],
    [40, 0.6],
    [60, 0.4],
    [80, 0.2],
  ])("maps %s%% transparency to %s alpha", (transparency, alpha) => {
    expect(surfaceAlphaFromTransparency(transparency)).toBe(alpha);
  });

  it.each([
    [-1, 1],
    [81, 0.2],
    [Number.NEGATIVE_INFINITY, 0.8],
    [Number.POSITIVE_INFINITY, 0.8],
    [Number.NaN, 0.8],
  ])("clamps or defaults %s to alpha %s", (transparency, alpha) => {
    expect(surfaceAlphaFromTransparency(transparency)).toBe(alpha);
  });

  it("decreases monotonically as transparency increases", () => {
    expect([0, 20, 40, 60, 80].map(surfaceAlphaFromTransparency)).toEqual([
      1,
      0.8,
      0.6,
      0.4,
      0.2,
    ]);
  });
});
