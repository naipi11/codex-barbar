import { describe, expect, it } from "vitest";
import { surfaceAlphaFromTransparency } from "./surfaceTransparency";

describe("surfaceAlphaFromTransparency", () => {
  it.each([
    [0, 1],
    [50, 0.5],
    [100, 0],
  ])("maps %s%% transparency to %s alpha", (transparency, alpha) => {
    expect(surfaceAlphaFromTransparency(transparency)).toBe(alpha);
  });

  it.each([
    [-1, 1],
    [101, 0],
    [Number.NEGATIVE_INFINITY, 0.8],
    [Number.POSITIVE_INFINITY, 0.8],
    [Number.NaN, 0.8],
  ])("clamps or defaults %s to alpha %s", (transparency, alpha) => {
    expect(surfaceAlphaFromTransparency(transparency)).toBe(alpha);
  });

  it("decreases monotonically as transparency increases", () => {
    expect([0, 25, 50, 75, 100].map(surfaceAlphaFromTransparency)).toEqual([
      1,
      0.75,
      0.5,
      0.25,
      0,
    ]);
  });
});
