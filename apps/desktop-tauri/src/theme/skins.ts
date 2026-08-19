export type SkinId =
  | "system"
  | "ink-green"
  | "vscode"
  | "macos"
  | "pink"
  | "blue"
  | "custom";

export type SkinMode = "light" | "dark";

export interface SkinTokens {
  bg: string;
  surface: string;
  surfaceMuted: string;
  border: string;
  borderStrong: string;
  fg: string;
  muted: string;
  accent: string;
  warn: string;
  crit: string;
  radius: string;
  shadow: string;
  panelPadding: string;
  regionPadding: string;
  avatarRadius: string;
  capsuleRadius: string;
  buttonRadius: string;
  trayShape: "cockpit" | "editor" | "glass" | "petal" | "frost";
}

export interface CustomSkinDraft {
  mode: SkinMode;
  bg: string;
  surface: string;
  fg: string;
  muted: string;
  accent: string;
  warn: string;
  crit: string;
  radius: number;
}

export const SKIN_STORAGE_KEY = "codex-barbar.skin-id";
export const CUSTOM_SKIN_STORAGE_KEY = "codex-barbar.custom-skin";
export const SKIN_CHANGE_EVENT = "codex-barbar-skin-changed";

export const SKIN_IDS: readonly SkinId[] = [
  "system",
  "ink-green",
  "vscode",
  "macos",
  "pink",
  "blue",
  "custom",
];

const DARK_SYSTEM: SkinTokens = {
  bg: "#10131a",
  surface: "rgba(23, 28, 38, 0.96)",
  surfaceMuted: "rgba(28, 34, 48, 0.98)",
  border: "rgba(255, 255, 255, 0.11)",
  borderStrong: "rgba(255, 255, 255, 0.18)",
  fg: "#e8edf6",
  muted: "#8b95a7",
  accent: "#30d158",
  warn: "#ffd60a",
  crit: "#ff453a",
  radius: "12px",
  shadow: "0 10px 28px rgba(0, 0, 0, 0.28)",
  panelPadding: "12px",
  regionPadding: "10px 12px",
  avatarRadius: "50%",
  capsuleRadius: "999px",
  buttonRadius: "999px",
  trayShape: "cockpit" as const,
};

const LIGHT_SYSTEM: SkinTokens = {
  bg: "#eef2f6",
  surface: "rgba(255, 255, 255, 0.94)",
  surfaceMuted: "rgba(245, 246, 252, 0.98)",
  border: "rgba(43, 52, 77, 0.12)",
  borderStrong: "rgba(43, 52, 77, 0.2)",
  fg: "#17202b",
  muted: "#5d6a7a",
  accent: "#147a43",
  warn: "#9a6408",
  crit: "#b42318",
  radius: "12px",
  shadow: "0 14px 36px rgba(77, 91, 135, 0.14)",
  panelPadding: "12px",
  regionPadding: "10px 12px",
  avatarRadius: "50%",
  capsuleRadius: "999px",
  buttonRadius: "999px",
  trayShape: "cockpit" as const,
};

export const SKIN_CATALOG = {
  "ink-green": {
    id: "ink-green" as const,
    mode: "dark" as const,
    tokens: {
      ...DARK_SYSTEM,
      bg: "#07110c",
      surface: "rgba(12, 28, 20, 0.96)",
      surfaceMuted: "rgba(16, 38, 27, 0.98)",
      border: "rgba(86, 217, 138, 0.16)",
      borderStrong: "rgba(86, 217, 138, 0.32)",
      fg: "#e7fff1",
      muted: "#8fb8a0",
      accent: "#3ee07a",
      radius: "10px",
      buttonRadius: "10px",
      trayShape: "cockpit" as const,
    },
  },
  vscode: {
    id: "vscode" as const,
    mode: "dark" as const,
    tokens: {
      ...DARK_SYSTEM,
      bg: "#1e1e1e",
      surface: "#252526",
      surfaceMuted: "#2d2d30",
      border: "#3c3c3c",
      borderStrong: "#007acc",
      fg: "#d4d4d4",
      muted: "#9d9d9d",
      accent: "#007acc",
      warn: "#cca700",
      crit: "#f14c4c",
      radius: "4px",
      shadow: "none",
      panelPadding: "10px",
      regionPadding: "8px 10px",
      avatarRadius: "4px",
      capsuleRadius: "4px",
      buttonRadius: "4px",
      trayShape: "editor" as const,
    },
  },
  macos: {
    id: "macos" as const,
    mode: "dark" as const,
    tokens: {
      ...DARK_SYSTEM,
      bg: "rgba(28, 28, 30, 0.78)",
      surface: "rgba(44, 44, 46, 0.62)",
      surfaceMuted: "rgba(58, 58, 60, 0.7)",
      border: "rgba(255, 255, 255, 0.12)",
      borderStrong: "rgba(255, 255, 255, 0.22)",
      fg: "#f5f5f7",
      muted: "rgba(235, 235, 245, 0.62)",
      accent: "#0a84ff",
      radius: "18px",
      shadow: "0 18px 40px rgba(0, 0, 0, 0.28)",
      regionPadding: "11px 13px",
      trayShape: "glass" as const,
    },
  },
  pink: {
    id: "pink" as const,
    mode: "light" as const,
    tokens: {
      ...LIGHT_SYSTEM,
      bg: "#fff1f5",
      surface: "rgba(255, 255, 255, 0.88)",
      surfaceMuted: "#ffe4ee",
      border: "rgba(190, 64, 110, 0.16)",
      borderStrong: "rgba(190, 64, 110, 0.28)",
      fg: "#4a1630",
      muted: "#9a5673",
      accent: "#d45386",
      radius: "20px",
      shadow: "0 16px 32px rgba(190, 64, 110, 0.12)",
      trayShape: "petal" as const,
    },
  },
  blue: {
    id: "blue" as const,
    mode: "light" as const,
    tokens: {
      ...LIGHT_SYSTEM,
      bg: "#eef6ff",
      surface: "rgba(255, 255, 255, 0.9)",
      surfaceMuted: "#dcecff",
      border: "rgba(37, 99, 235, 0.14)",
      borderStrong: "rgba(37, 99, 235, 0.28)",
      fg: "#10243f",
      muted: "#4d6b8a",
      accent: "#2563eb",
      radius: "16px",
      shadow: "0 16px 32px rgba(37, 99, 235, 0.12)",
      buttonRadius: "14px",
      trayShape: "frost" as const,
    },
  },
};

export const DEFAULT_CUSTOM_SKIN: CustomSkinDraft = {
  mode: "dark",
  bg: "#10131a",
  surface: "#171c26",
  fg: "#e8edf6",
  muted: "#8b95a7",
  accent: "#30d158",
  warn: "#ffd60a",
  crit: "#ff453a",
  radius: 14,
};

export function isSkinId(value: unknown): value is SkinId {
  return typeof value === "string" && (SKIN_IDS as readonly string[]).includes(value);
}

export function resolveSkinId(value: unknown): SkinId {
  return isSkinId(value) ? value : "system";
}

export function resolveSkinMode(
  skinId: SkinId,
  custom: CustomSkinDraft,
  systemDark: boolean,
): SkinMode {
  if (skinId === "system") return systemDark ? "dark" : "light";
  if (skinId === "custom") return custom.mode;
  return SKIN_CATALOG[skinId].mode;
}

export function customSkinTokens(draft: CustomSkinDraft): SkinTokens {
  const radius = Math.max(4, Math.min(28, Math.round(draft.radius)));
  const light = draft.mode === "light";
  return {
    bg: draft.bg,
    surface: draft.surface,
    surfaceMuted: light ? "rgba(255,255,255,0.86)" : "rgba(255,255,255,0.06)",
    border: light ? "rgba(16, 24, 40, 0.12)" : "rgba(255,255,255,0.12)",
    borderStrong: draft.accent,
    fg: draft.fg,
    muted: draft.muted,
    accent: draft.accent,
    warn: draft.warn,
    crit: draft.crit,
    radius: radius + "px",
    shadow: light
      ? "0 14px 32px rgba(16, 24, 40, 0.12)"
      : "0 14px 32px rgba(0, 0, 0, 0.28)",
    panelPadding: "12px",
    regionPadding: "10px 12px",
    avatarRadius: "50%",
    capsuleRadius: Math.max(radius, 16) + "px",
    buttonRadius: radius + "px",
    trayShape: "cockpit" as const,
  };
}

export function tokensForSkin(
  skinId: SkinId,
  custom: CustomSkinDraft,
  systemDark: boolean,
): SkinTokens {
  if (skinId === "custom") return customSkinTokens(custom);
  if (skinId === "system") return systemDark ? DARK_SYSTEM : LIGHT_SYSTEM;
  return SKIN_CATALOG[skinId].tokens;
}

export function cssVarsFromTokens(tokens: SkinTokens): Record<string, string> {
  return {
    "--app-bg": tokens.bg,
    "--app-fg": tokens.fg,
    "--app-muted": tokens.muted,
    "--app-surface": tokens.surface,
    "--app-border": tokens.border,
    "--app-accent": tokens.accent,
    "--app-warn": tokens.warn,
    "--app-crit": tokens.crit,
    "--tray-bg": tokens.bg,
    "--tray-surface": tokens.surface,
    "--tray-surface-muted": tokens.surfaceMuted,
    "--tray-border": tokens.border,
    "--tray-border-strong": tokens.borderStrong,
    "--tray-fg": tokens.fg,
    "--tray-fg-muted": tokens.muted,
    "--tray-accent": tokens.accent,
    "--tray-warning": tokens.warn,
    "--tray-critical": tokens.crit,
    "--tray-radius": tokens.radius,
    "--tray-shadow": tokens.shadow,
    "--tray-panel-padding": tokens.panelPadding,
    "--tray-region-padding": tokens.regionPadding,
    "--tray-avatar-radius": tokens.avatarRadius,
    "--tray-capsule-radius": tokens.capsuleRadius,
    "--tray-button-radius": tokens.buttonRadius,
  };
}

export function applySkinVars(target: HTMLElement, tokens: SkinTokens): void {
  for (const [name, value] of Object.entries(cssVarsFromTokens(tokens))) {
    target.style.setProperty(name, value);
  }
}

export function readStoredSkinId(): SkinId {
  if (typeof localStorage === "undefined") return "system";
  try {
    return resolveSkinId(localStorage.getItem(SKIN_STORAGE_KEY));
  } catch {
    return "system";
  }
}

export function writeStoredSkinId(skinId: SkinId): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(SKIN_STORAGE_KEY, skinId);
}

export function readStoredCustomSkin(): CustomSkinDraft {
  if (typeof localStorage === "undefined") return DEFAULT_CUSTOM_SKIN;
  try {
    const raw = localStorage.getItem(CUSTOM_SKIN_STORAGE_KEY);
    if (!raw) return DEFAULT_CUSTOM_SKIN;
    const parsed = JSON.parse(raw) as Partial<CustomSkinDraft>;
    return {
      mode: parsed.mode === "light" ? "light" : "dark",
      bg: typeof parsed.bg === "string" ? parsed.bg : DEFAULT_CUSTOM_SKIN.bg,
      surface: typeof parsed.surface === "string" ? parsed.surface : DEFAULT_CUSTOM_SKIN.surface,
      fg: typeof parsed.fg === "string" ? parsed.fg : DEFAULT_CUSTOM_SKIN.fg,
      muted: typeof parsed.muted === "string" ? parsed.muted : DEFAULT_CUSTOM_SKIN.muted,
      accent: typeof parsed.accent === "string" ? parsed.accent : DEFAULT_CUSTOM_SKIN.accent,
      warn: typeof parsed.warn === "string" ? parsed.warn : DEFAULT_CUSTOM_SKIN.warn,
      crit: typeof parsed.crit === "string" ? parsed.crit : DEFAULT_CUSTOM_SKIN.crit,
      radius:
        typeof parsed.radius === "number" && Number.isFinite(parsed.radius)
          ? parsed.radius
          : DEFAULT_CUSTOM_SKIN.radius,
    };
  } catch {
    return DEFAULT_CUSTOM_SKIN;
  }
}

export function writeStoredCustomSkin(draft: CustomSkinDraft): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(CUSTOM_SKIN_STORAGE_KEY, JSON.stringify(draft));
}

export function notifySkinChanged(): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new Event(SKIN_CHANGE_EVENT));
}
