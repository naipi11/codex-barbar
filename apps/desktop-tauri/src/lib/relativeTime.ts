const MINUTES_PER_HOUR = 60;
const WEEK_MINUTES = 10_080;

function isChinese(locale: string): boolean {
  return locale.toLowerCase().startsWith("zh");
}

export function formatDurationMinutes(
  minutes: number | null,
  locale = "en-US",
): string {
  if (minutes === null || !Number.isFinite(minutes) || minutes < 0) {
    return isChinese(locale) ? "未知时长" : "Unknown duration";
  }

  const rounded = Math.round(minutes);
  if (rounded === 300) return isChinese(locale) ? "5小时" : "5 hours";
  if (rounded === WEEK_MINUTES) return isChinese(locale) ? "每周" : "Weekly";
  if (rounded % MINUTES_PER_HOUR === 0) {
    const hours = rounded / MINUTES_PER_HOUR;
    return isChinese(locale) ? `${hours}小时` : `${hours} hours`;
  }
  return isChinese(locale) ? `${rounded}分钟` : `${rounded} minutes`;
}

export function formatRelativeDuration(
  milliseconds: number,
  locale = "en-US",
): string {
  const totalMinutes = Math.max(0, Math.ceil(milliseconds / 60_000));
  if (isChinese(locale)) {
    if (totalMinutes < 1) return "不到1分钟";
    const hours = Math.floor(totalMinutes / MINUTES_PER_HOUR);
    const minutes = totalMinutes % MINUTES_PER_HOUR;
    if (hours === 0) return `${minutes}分钟`;
    return minutes === 0 ? `${hours}小时` : `${hours}小时${minutes}分钟`;
  }

  if (totalMinutes < 1) return "<1m";
  const hours = Math.floor(totalMinutes / MINUTES_PER_HOUR);
  const minutes = totalMinutes % MINUTES_PER_HOUR;
  if (hours === 0) return `${minutes}m`;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

function localResetLabel(
  reset: Date,
  locale: string,
  timeZone: string,
): string {
  try {
    return new Intl.DateTimeFormat(locale, {
      timeZone,
      month: isChinese(locale) ? "numeric" : "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(reset);
  } catch {
    return reset.toISOString();
  }
}

export function formatResetTime(
  resetsAt: string | null,
  now: Date,
  locale = "en-US",
  timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone,
): string {
  if (!resetsAt) return isChinese(locale) ? "等待刷新" : "Awaiting refresh";
  const reset = new Date(resetsAt);
  if (!Number.isFinite(reset.getTime()) || reset.getTime() <= now.getTime()) {
    return isChinese(locale) ? "等待刷新" : "Awaiting refresh";
  }

  const duration = formatRelativeDuration(reset.getTime() - now.getTime(), locale);
  const local = localResetLabel(reset, locale, timeZone);
  return isChinese(locale)
    ? `${local}后重置（${duration}）`
    : `Resets ${local} (in ${duration})`;
}
