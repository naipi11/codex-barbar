import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useFormattedResetTime } from "./useFormattedResetTime";

describe("useFormattedResetTime", () => {
  it("returns a stable explicit-timezone value for deterministic callers", () => {
    const { result } = renderHook(() =>
      useFormattedResetTime(
        "2026-08-07T00:30:00Z",
        "en-US",
        "Asia/Shanghai",
        new Date("2026-08-06T23:30:00Z"),
      ),
    );

    expect(result.current).toMatch(/Aug 7/);
    expect(result.current).toMatch(/in 1h/);
  });
});
