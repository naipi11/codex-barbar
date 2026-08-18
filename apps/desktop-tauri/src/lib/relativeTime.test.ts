import { describe, expect, it } from "vitest";
import { formatDurationMinutes, formatResetTime } from "./relativeTime";

describe("relative time formatting", () => {
  const now = new Date("2026-08-06T23:30:00Z");

  it("formats common and unknown window durations", () => {
    expect(formatDurationMinutes(300, "en-US")).toBe("5 hours");
    expect(formatDurationMinutes(10_080, "en-US")).toBe("Weekly");
    expect(formatDurationMinutes(37, "en-US")).toBe("37 minutes");
    expect(formatDurationMinutes(37, "zh-CN")).toBe("37分钟");
  });

  it("uses the local timezone when formatting a reset near a UTC day boundary", () => {
    const reset = "2026-08-07T00:30:00Z";
    const shanghai = formatResetTime(reset, now, "en-US", "Asia/Shanghai");
    const losAngeles = formatResetTime(
      reset,
      now,
      "en-US",
      "America/Los_Angeles",
    );

    expect(shanghai).toMatch(/Aug 7/);
    expect(losAngeles).toMatch(/Aug 6/);
    expect(shanghai).toMatch(/in 1h/);
  });

  it("never renders a negative duration after a reset has passed", () => {
    expect(
      formatResetTime(
        "2026-08-06T22:00:00Z",
        now,
        "en-US",
        "America/Los_Angeles",
      ),
    ).toBe("Awaiting refresh");
  });
});
