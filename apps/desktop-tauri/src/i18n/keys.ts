/**
 * V1 desktop locale keys shared with the Rust shell.
 *
 * Keep this list in sync with `V1_LOCALE_KEYS` in rust/src/locale.rs;
 * `pnpm run check-locale` verifies the drift automatically.
 */
export const ALL_LOCALE_KEYS = [
  "app.name",
  "usage.fiveHours",
  "usage.weekly",
  "usage.remaining",
  "usage.used",
  "usage.awaitingRefresh",
  "status.updated",
  "status.cached",
  "status.refreshing",
  "error.codexNotFound",
  "error.unsupportedCodexVersion",
  "error.notSignedIn",
  "error.apiKeyNoQuota",
  "error.authExpired",
  "error.offlineOrTimeout",
  "error.rateLimited",
  "error.protocolMismatch",
  "error.vaultFailure",
  "error.storageFailure",
  "action.refresh",
  "action.openUsage",
  "action.settings",
  "action.exportDiagnostics",
  "settings.general",
  "settings.accounts",
  "settings.advanced",
  "settings.about",
  "accounts.add",
  "accounts.rename",
  "accounts.relogin",
  "accounts.remove",
  "accounts.currentCli",
] as const;

export type LocaleKey = (typeof ALL_LOCALE_KEYS)[number];

export const localeKeys: readonly string[] = ALL_LOCALE_KEYS;
