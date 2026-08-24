import { useMemo, useState } from "react";
import { CommittedRangeField } from "../CommittedRangeField";
import type {
  AppSettingsDto,
  SettingsPatchDto,
  StatusSurfaceKind,
} from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";
import {
  DEFAULT_CUSTOM_SKIN,
  SKIN_IDS,
  notifySkinChanged,
  readStoredCustomSkin,
  readStoredSkinId,
  resolveSkinId,
  writeStoredCustomSkin,
  writeStoredSkinId,
  type CustomSkinDraft,
  type SkinId,
} from "../../../theme/skins";

const REFRESH_VALUES = [0, 60, 300, 900, 1800] as const;
const SKIN_VALUES: readonly SkinId[] = SKIN_IDS;

function refreshOption(value: number): 0 | 60 | 300 | 900 | 1800 {
  return REFRESH_VALUES.includes(value as (typeof REFRESH_VALUES)[number])
    ? (value as 0 | 60 | 300 | 900 | 1800)
    : 300;
}

function backendThemeForSkin(skinId: SkinId, custom: CustomSkinDraft): "system" | "light" | "dark" {
  if (skinId === "system") return "system";
  if (skinId === "custom") return custom.mode;
  if (skinId === "pink" || skinId === "blue") return "light";
  return "dark";
}

function ColorField({
  id,
  label,
  value,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  onChange(value: string): void;
}) {
  return (
    <label className="settings-color-field" htmlFor={id}>
      <span>{label}</span>
      <input id={id} type="color" value={value} onChange={(event) => onChange(event.target.value)} />
      <code>{value}</code>
    </label>
  );
}

export default function GeneralTab({
  settings,
  update,
  setSurfaceEnabled: _setSurfaceEnabled,
  copy = settingsCopy("en-US"),
}: {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<AppSettingsDto>;
  setSurfaceEnabled(surface: StatusSurfaceKind, enabled: boolean): Promise<AppSettingsDto>;
  copy?: SettingsCopy;
}) {
  const [skinId, setSkinId] = useState<SkinId>(() => readStoredSkinId());
  const [custom, setCustom] = useState<CustomSkinDraft>(() => readStoredCustomSkin());
  const themeOptions = useMemo(
    () => SKIN_VALUES.map((id, index) => ({ id, label: copy.general.themeOptions[index] ?? id })),
    [copy.general.themeOptions],
  );

  const applySkin = (nextSkin: SkinId, nextCustom = custom) => {
    setSkinId(nextSkin);
    writeStoredSkinId(nextSkin);
    writeStoredCustomSkin(nextCustom);
    notifySkinChanged();
    void update({ theme: backendThemeForSkin(nextSkin, nextCustom) });
  };

  return (
    <section aria-label={`${copy.general.title} settings`}>
      <h2>{copy.general.title}</h2>
      <p className="settings-field">
        <label>
          <input type="checkbox" checked={settings.autostartEnabled} onChange={(event) => void update({ autostartEnabled: event.target.checked })} />
          {copy.general.autostart}
        </label>
      </p>
      <p className="settings-field">
        <label htmlFor="refresh-interval">{copy.general.refreshInterval}</label>
        <select id="refresh-interval" value={settings.refreshIntervalSeconds} onChange={(event) => void update({ refreshIntervalSeconds: refreshOption(Number(event.target.value)) })}>
          {REFRESH_VALUES.map((value, index) => <option key={value} value={value}>{copy.general.refreshOptions[index]}</option>)}
        </select>
      </p>
      <p className="settings-field">
        <label htmlFor="display-mode">{copy.general.displayMode}</label>
        <select id="display-mode" value={settings.displayMode} onChange={(event) => void update({ displayMode: event.target.value as "remaining" | "used" })}>
          <option value="remaining">{copy.general.displayOptions[0]}</option>
          <option value="used">{copy.general.displayOptions[1]}</option>
        </select>
      </p>
      <p className="settings-field">
        <label htmlFor="theme">{copy.general.theme}</label>
        <select
          id="theme"
          value={skinId}
          onChange={(event) => applySkin(resolveSkinId(event.target.value))}
        >
          {themeOptions.map((option) => (
            <option key={option.id} value={option.id}>{option.label}</option>
          ))}
        </select>
      </p>
      {skinId === "custom" ? (
        <fieldset className="settings-custom-skin" aria-label={copy.general.customTheme}>
          <legend>{copy.general.customTheme}</legend>
          <p className="settings-field">
            <label htmlFor="custom-mode">{copy.general.customMode}</label>
            <select
              id="custom-mode"
              value={custom.mode}
              onChange={(event) => setCustom({ ...custom, mode: event.target.value as "light" | "dark" })}
            >
              <option value="dark">{copy.general.themeOptions[1].includes("黑") ? "深色" : "Dark"}</option>
              <option value="light">{copy.general.themeOptions[1].includes("黑") ? "浅色" : "Light"}</option>
            </select>
          </p>
          <div className="settings-color-grid">
            <ColorField id="custom-bg" label={copy.general.customBg} value={custom.bg} onChange={(bg) => setCustom({ ...custom, bg })} />
            <ColorField id="custom-surface" label={copy.general.customSurface} value={custom.surface} onChange={(surface) => setCustom({ ...custom, surface })} />
            <ColorField id="custom-fg" label={copy.general.customFg} value={custom.fg} onChange={(fg) => setCustom({ ...custom, fg })} />
            <ColorField id="custom-muted" label={copy.general.customMuted} value={custom.muted} onChange={(muted) => setCustom({ ...custom, muted })} />
            <ColorField id="custom-accent" label={copy.general.customAccent} value={custom.accent} onChange={(accent) => setCustom({ ...custom, accent })} />
          </div>
          <CommittedRangeField
            id="custom-radius"
            label={copy.general.customRadius}
            value={custom.radius}
            min={4}
            max={28}
            tickValues={[4, 10, 16, 22, 28]}
            valueText={(value) => `${value}px`}
            onCommit={async (radius) => {
              const next = { ...custom, radius };
              setCustom(next);
              writeStoredCustomSkin(next);
              return radius;
            }}
          />
          <div className="settings-custom-skin__actions">
            <button type="button" onClick={() => applySkin("custom", custom)}>{copy.general.applyCustom}</button>
            <button type="button" onClick={() => { setCustom(DEFAULT_CUSTOM_SKIN); applySkin("custom", DEFAULT_CUSTOM_SKIN); }}>{copy.general.resetCustom}</button>
          </div>
        </fieldset>
      ) : null}
      <p className="settings-field">
        <label htmlFor="language">{copy.general.language}</label>
        <select id="language" value={settings.language} onChange={(event) => void update({ language: event.target.value as "system" | "zh-CN" | "en-US" })}>
          <option value="system">{copy.general.system}</option>
          <option value="zh-CN">{copy.general.simplifiedChinese}</option>
          <option value="en-US">English</option>
        </select>
      </p>
    </section>
  );
}
