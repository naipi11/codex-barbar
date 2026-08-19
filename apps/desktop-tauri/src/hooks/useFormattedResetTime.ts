import { formatResetTime } from "../lib/relativeTime";

function systemTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

export function useFormattedResetTime(
  resetsAt: string | null,
  locale = "en-US",
  timeZone = systemTimeZone(),
  nowOverride?: Date,
): string {
  return formatResetTime(
    resetsAt,
    nowOverride ?? new Date(),
    locale,
    timeZone,
  );
}
