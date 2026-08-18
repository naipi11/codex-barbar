import { describe, expect, it } from "vitest";
import { ALL_LOCALE_KEYS, localeKeys } from "./keys";

describe("V1 locale keys", () => {
  it("requires the complete V1 locale key set", () => {
    expect(localeKeys).toEqual(
      expect.arrayContaining([
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
      ]),
    );
  });

  it("has no duplicate keys", () => {
    expect(new Set(ALL_LOCALE_KEYS).size).toBe(ALL_LOCALE_KEYS.length);
  });
});
