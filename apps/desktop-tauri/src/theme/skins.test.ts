import { describe, expect, it } from "vitest";
import {
  DEFAULT_CUSTOM_SKIN,
  customSkinTokens,
  resolveSkinId,
  resolveSkinMode,
  tokensForSkin,
} from "./skins";

describe("skins", () => {
  it("falls back to system for unknown ids", () => {
    expect(resolveSkinId("nope")).toBe("system");
    expect(resolveSkinId("ink-green")).toBe("ink-green");
  });

  it("uses catalog modes and custom tokens", () => {
    expect(resolveSkinMode("pink", DEFAULT_CUSTOM_SKIN, true)).toBe("light");
    expect(resolveSkinMode("vscode", DEFAULT_CUSTOM_SKIN, false)).toBe("dark");
    expect(tokensForSkin("macos", DEFAULT_CUSTOM_SKIN, true).trayShape).toBe("glass");
    expect(customSkinTokens({ ...DEFAULT_CUSTOM_SKIN, radius: 20 }).radius).toBe("20px");
  });
});
