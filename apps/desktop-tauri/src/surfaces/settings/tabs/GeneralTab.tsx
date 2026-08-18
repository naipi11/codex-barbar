import type {
  AppSettingsDto,
  SettingsPatchDto,
  StatusSurfaceKind,
} from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

const REFRESH_VALUES = [0, 60, 300, 900, 1800] as const;

function refreshOption(value: number): 0 | 60 | 300 | 900 | 1800 {
  return REFRESH_VALUES.includes(value as (typeof REFRESH_VALUES)[number])
    ? (value as 0 | 60 | 300 | 900 | 1800)
    : 300;
}

function StatusCard({
  title,
  description,
  enabled,
  enabledLabel,
  surface,
  opacity,
  opacityLabel,
  opacityField,
  update,
  setSurfaceEnabled,
}: {
  title: string;
  description: string;
  enabled: boolean;
  enabledLabel: string;
  surface: StatusSurfaceKind;
  opacity: number;
  opacityLabel: string;
  opacityField: "taskbarStatusOpacity" | "floatBallOpacity";
  update(patch: SettingsPatchDto): Promise<unknown>;
  setSurfaceEnabled(surface: StatusSurfaceKind, enabled: boolean): Promise<AppSettingsDto>;
}) {
  const inputId = `${surface}-opacity`;
  return (
    <article className="settings-status-card">
      <div className="settings-status-card__heading">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
      <label className="settings-switch">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => void setSurfaceEnabled(surface, event.target.checked)}
        />
        {enabledLabel}
      </label>
      <label className="settings-range" htmlFor={inputId}>
        <span>{opacityLabel}</span>
        <output htmlFor={inputId}>{opacity}%</output>
      </label>
      <input
        id={inputId}
        type="range"
        min="0"
        max="80"
        step="1"
        value={opacity}
        aria-label={opacityLabel}
        aria-valuetext={`${opacity}%`}
        onChange={(event) => void update({ [opacityField]: Number.parseInt(event.target.value, 10) })}
      />
      <div className="settings-range__ticks" aria-hidden="true">
        <span>0</span><span>20</span><span>40</span><span>60</span><span>80</span>
      </div>
    </article>
  );
}

export default function GeneralTab({
  settings,
  update,
  setSurfaceEnabled,
  copy = settingsCopy("en-US"),
}: {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<unknown>;
  setSurfaceEnabled(surface: StatusSurfaceKind, enabled: boolean): Promise<AppSettingsDto>;
  copy?: SettingsCopy;
}) {
  return (
    <section aria-label={`${copy.general.title} settings`}>
      <h2>{copy.general.title}</h2>
      <p className="settings-field">
        <label>
          <input type="checkbox" checked={settings.autostartEnabled} onChange={(event) => void update({ autostartEnabled: event.target.checked })} />
          {copy.general.autostart}
        </label>
      </p>
      <StatusCard
        title={copy.general.taskbarTitle}
        description={copy.general.taskbarDescription}
        enabled={settings.taskbarStatusEnabled}
        enabledLabel={copy.general.taskbarEnabled}
        surface="taskbarStatus"
        opacity={settings.taskbarStatusOpacity}
        opacityLabel={copy.general.taskbarOpacity}
        opacityField="taskbarStatusOpacity"
        update={update}
        setSurfaceEnabled={setSurfaceEnabled}
      />
      <StatusCard
        title={copy.general.floatBallTitle}
        description={copy.general.floatBallDescription}
        enabled={settings.floatBallEnabled}
        enabledLabel={copy.general.floatBallEnabled}
        surface="floatBall"
        opacity={settings.floatBallOpacity}
        opacityLabel={copy.general.floatBallOpacity}
        opacityField="floatBallOpacity"
        update={update}
        setSurfaceEnabled={setSurfaceEnabled}
      />
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
        <select id="theme" value={settings.theme} onChange={(event) => void update({ theme: event.target.value as "system" | "light" | "dark" })}>
          <option value="system">{copy.general.themeOptions[0]}</option>
          <option value="light">{copy.general.themeOptions[1]}</option>
          <option value="dark">{copy.general.themeOptions[2]}</option>
        </select>
      </p>
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
