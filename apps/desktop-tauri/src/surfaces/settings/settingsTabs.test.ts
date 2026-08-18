import { describe, expect, it } from "vitest";
import { TAB_IDS, isSettingsTabId } from "./settingsTabs";
import { settingsCopy } from "./settingsCopy";

describe("settings tab contract", () => {
  it("keeps all shell-supported tab ids in the frontend whitelist", () => {
    expect(TAB_IDS).toEqual([
      "general",
      "providers",
      "notifications",
      "menuBar",
      "menu",
      "usageSpend",
      "advanced",
      "about",
    ]);
    for (const id of TAB_IDS) {
      expect(isSettingsTabId(id)).toBe(true);
    }
  });

  it("keeps labels at the localized settings-copy boundary", () => {
    expect(TAB_IDS.map((id) => settingsCopy("zh-CN").tabs[id])).toEqual([
      "通用",
      "账户",
      "通知",
      "菜单栏",
      "菜单",
      "用量与费用",
      "高级",
      "关于",
    ]);
  });
});
