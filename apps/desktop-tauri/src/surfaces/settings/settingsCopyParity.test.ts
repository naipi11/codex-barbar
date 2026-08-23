import { describe, expect, it } from "vitest";
import { settingsCopy } from "./settingsCopy";

function leafKeys(value: unknown, prefix = ""): string[] {
  if (value === null || typeof value !== "object") return [prefix];
  return Object.entries(value as Record<string, unknown>).flatMap(
    ([key, child]) => leafKeys(child, prefix ? `${prefix}.${key}` : key),
  );
}

describe("settings copy parity", () => {
  it("keeps every section and leaf key in both locales", () => {
    const en = settingsCopy("en-US") as unknown as Record<string, unknown>;
    const zh = settingsCopy("zh-CN") as unknown as Record<string, unknown>;
    expect(leafKeys(zh).sort()).toEqual(leafKeys(en).sort());
  });

  it("does not ship placeholder copy on implemented tabs", () => {
    for (const language of ["en-US", "zh-CN"] as const) {
      const copy = settingsCopy(language);
      expect(copy.placeholder).toMatch(/later release|后续版本/);
      expect(copy.menu.title).not.toBe(copy.placeholder);
      expect(copy.usageSpend.title).not.toBe(copy.placeholder);
      expect(copy.taskbarTray.title).not.toBe(copy.placeholder);
      expect(copy.notifications.title).not.toBe(copy.placeholder);
    }
  });

  it("keeps menu item label registries aligned between locales", () => {
    const en = settingsCopy("en-US");
    const zh = settingsCopy("zh-CN");
    expect(Object.keys(zh.menu.itemLabels).sort()).toEqual(
      Object.keys(en.menu.itemLabels).sort(),
    );
    for (const id of [
      "open_panel",
      "refresh",
      "accounts",
      "open_usage",
      "settings",
      "about",
      "quit",
      "dismiss",
    ]) {
      expect(en.menu.itemLabels[id]).toBeTruthy();
      expect(zh.menu.itemLabels[id]).toBeTruthy();
    }
  });

  it("uses locale-specific copy for usage spend states", () => {
    const en = settingsCopy("en-US");
    const zh = settingsCopy("zh-CN");
    expect(en.usageSpend.emptyState).toMatch(/no local codex/i);
    expect(zh.usageSpend.emptyState).toMatch(/未找到/);
    expect(en.usageSpend.cancelledState).toMatch(/cancelled/i);
    expect(zh.usageSpend.cancelledState).toMatch(/取消/);
    expect(en.usageSpend.unavailableState).toMatch(/unavailable/i);
    expect(zh.usageSpend.unavailableState).toMatch(/暂无法/);
  });
});

