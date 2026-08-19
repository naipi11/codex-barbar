import { useEffect, useState } from "react";
import type { AppSettingsDto } from "../types/bridge";
import {
  SKIN_CHANGE_EVENT,
  applySkinVars,
  readStoredCustomSkin,
  readStoredSkinId,
  resolveSkinMode,
  tokensForSkin,
  type CustomSkinDraft,
  type SkinId,
} from "../theme/skins";

export type ThemePreference = AppSettingsDto["theme"];

export function resolveTheme(
  preference: ThemePreference,
  systemDark: boolean,
  skinId: SkinId = readStoredSkinId(),
  custom: CustomSkinDraft = readStoredCustomSkin(),
): "light" | "dark" {
  if (skinId !== "system") {
    return resolveSkinMode(skinId, custom, systemDark);
  }
  if (preference === "light") return "light";
  if (preference === "dark") return "dark";
  return systemDark ? "dark" : "light";
}

function systemPrefersDark(): boolean {
  try {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return false;
    }
    return Boolean(window.matchMedia("(prefers-color-scheme: dark)").matches);
  } catch {
    return false;
  }
}

function applyDocumentTheme(
  preference: ThemePreference,
  systemDark: boolean,
): "light" | "dark" {
  const skinId = readStoredSkinId();
  const custom = readStoredCustomSkin();
  const theme = resolveTheme(preference, systemDark, skinId, custom);
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.dataset.skin = skinId;
  applySkinVars(root, tokensForSkin(skinId, custom, systemDark));
  return theme;
}

/**
 * Apply an explicit `data-theme` / `data-skin` pair so one WebView never flips
 * another through the shared WebView2 profile. `system` follows the OS signal.
 */
export function useTheme(preference: ThemePreference): "light" | "dark" {
  const [theme, setTheme] = useState<"light" | "dark">(() =>
    resolveTheme(preference, systemPrefersDark()),
  );

  useEffect(() => {
    const apply = () => setTheme(applyDocumentTheme(preference, systemPrefersDark()));
    apply();
    let media: MediaQueryList | null = null;
    try {
      if (typeof window.matchMedia === "function") {
        media = window.matchMedia("(prefers-color-scheme: dark)");
        media.addEventListener("change", apply);
      }
    } catch {
      media = null;
    }
    window.addEventListener(SKIN_CHANGE_EVENT, apply);
    window.addEventListener("storage", apply);
    return () => {
      media?.removeEventListener("change", apply);
      window.removeEventListener(SKIN_CHANGE_EVENT, apply);
      window.removeEventListener("storage", apply);
    };
  }, [preference]);

  return theme;
}
