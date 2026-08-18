export type SettingsTabId =
  | "general"
  | "providers"
  | "notifications"
  | "menuBar"
  | "menu"
  | "usageSpend"
  | "advanced"
  | "about";

export const TAB_IDS: readonly SettingsTabId[] = [
  "general",
  "providers",
  "notifications",
  "menuBar",
  "menu",
  "usageSpend",
  "advanced",
  "about",
];

export function isSettingsTabId(value: string | null): value is SettingsTabId {
  return value !== null && TAB_IDS.includes(value as SettingsTabId);
}
