import { useEffect, useState } from "react";
import type { AppSettingsDto } from "../types/bridge";

export type ThemePreference = AppSettingsDto["theme"];

export function resolveTheme(
  preference: ThemePreference,
  systemDark: boolean,
): "light" | "dark" {
  if (preference === "light") return "light";
  if (preference === "dark") return "dark";
  return systemDark ? "dark" : "light";
}

function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/**
 * Apply an explicit `data-theme` attribute so one WebView never flips another
 * through the shared WebView2 profile. `system` follows the OS signal live.
 */
export function useTheme(preference: ThemePreference): "light" | "dark" {
  const [theme, setTheme] = useState<"light" | "dark">(() =>
    resolveTheme(preference, systemPrefersDark()),
  );

  useEffect(() => {
    const apply = () => setTheme(resolveTheme(preference, systemPrefersDark()));
    apply();
    if (
      preference !== "system" ||
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    ) {
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [preference]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  return theme;
}
