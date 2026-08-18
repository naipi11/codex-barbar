import { useEffect, useState } from "react";
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
  const [now, setNow] = useState(() => nowOverride ?? new Date());

  useEffect(() => {
    if (nowOverride) return;
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, [nowOverride]);

  return formatResetTime(
    resetsAt,
    nowOverride ?? now,
    locale,
    timeZone,
  );
}
