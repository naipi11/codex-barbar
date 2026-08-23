import { useState } from "react";
import { useCommittedRange } from "../../../hooks/useCommittedRange";
import type {
  AppSettingsDto,
  SettingsPatchDto,
  StatusSurfaceKind,
  TaskbarTrayPreferencesDto,
} from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

const TASKBAR_ITEM_FIELDS = [
  "showTaskbarIcon",
  "showTaskbarAccount",
  "showWeeklyLabel",
  "showWeeklyPercent",
  "showResetDate",
] as const satisfies readonly (keyof TaskbarTrayPreferencesDto)[];

export default function TaskbarTrayTab({
  settings,
  update,
  setSurfaceEnabled,
  copy = settingsCopy("en-US"),
}: {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<AppSettingsDto>;
  setSurfaceEnabled(
    surface: StatusSurfaceKind,
    enabled: boolean,
  ): Promise<AppSettingsDto>;
  copy?: SettingsCopy;
}) {
  const [hasTransparencySaveError, setHasTransparencySaveError] = useState(false);
  const visibleTaskbarItems = TASKBAR_ITEM_FIELDS.filter(
    (field) => settings.taskbarTray[field],
  ).length;
  const transparency = useCommittedRange({
    value: settings.taskbarStatusOpacity,
    min: 0,
    max: 80,
    onCommit: async (nextValue) => {
      const saved = await update({ taskbarStatusOpacity: nextValue });
      return saved.taskbarStatusOpacity;
    },
    onError: () => setHasTransparencySaveError(true),
    onSuccess: () => setHasTransparencySaveError(false),
  });

  const patchTaskbarTray = <K extends keyof TaskbarTrayPreferencesDto>(
    field: K,
    value: TaskbarTrayPreferencesDto[K],
  ) => void update({ taskbarTray: { [field]: value } });

  const taskbarItems = [
    ["showTaskbarIcon", copy.taskbarTray.showIcon],
    ["showTaskbarAccount", copy.taskbarTray.showAccount],
    ["showWeeklyLabel", copy.taskbarTray.showWeeklyLabel],
    ["showWeeklyPercent", copy.taskbarTray.showWeeklyPercent],
    ["showResetDate", copy.taskbarTray.showResetDate],
  ] as const;

  const tooltipItems = [
    ["tooltipAccount", copy.taskbarTray.tooltipAccount],
    ["tooltipWeekly", copy.taskbarTray.tooltipWeekly],
    ["tooltipResetDate", copy.taskbarTray.tooltipResetDate],
    ["tooltipUpdatedAt", copy.taskbarTray.tooltipUpdatedAt],
  ] as const;

  return (
    <section aria-label={`${copy.taskbarTray.title} settings`}>
      <h2>{copy.taskbarTray.title}</h2>
      <div className="settings-preference-groups">
        <fieldset className="settings-preference-group">
          <legend>{copy.taskbarTray.taskbarLegend}</legend>
          <p className="settings-preference-group__description">
            {copy.taskbarTray.taskbarDescription}
          </p>
          <label className="settings-switch settings-switch--primary">
            <input
              type="checkbox"
              checked={settings.taskbarStatusEnabled}
              disabled={!settings.taskbarStatusEnabled && visibleTaskbarItems === 0}
              aria-describedby="taskbar-visible-item-help"
              onChange={(event) =>
                void setSurfaceEnabled("taskbarStatus", event.target.checked)
              }
            />
            {copy.taskbarTray.taskbarEnabled}
          </label>

          <div className="settings-preference-grid">
            {taskbarItems.map(([field, label]) => {
              const checked = settings.taskbarTray[field];
              return (
                <label className="settings-switch" key={field}>
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={
                      settings.taskbarStatusEnabled &&
                      checked &&
                      visibleTaskbarItems === 1
                    }
                    aria-describedby="taskbar-visible-item-help"
                    onChange={(event) => patchTaskbarTray(field, event.target.checked)}
                  />
                  {label}
                </label>
              );
            })}
          </div>
          <p
            id="taskbar-visible-item-help"
            className="settings-preference-group__hint"
          >
            {copy.taskbarTray.keepOneVisible}
          </p>

          <div className="settings-preference-grid settings-preference-grid--controls">
            <label className="settings-compact-field" htmlFor="taskbar-density">
              <span>{copy.taskbarTray.density}</span>
              <select
                id="taskbar-density"
                value={settings.taskbarTray.density}
                onChange={(event) =>
                  patchTaskbarTray(
                    "density",
                    event.target.value as TaskbarTrayPreferencesDto["density"],
                  )
                }
              >
                <option value="compact">{copy.taskbarTray.densityOptions[0]}</option>
                <option value="standard">{copy.taskbarTray.densityOptions[1]}</option>
              </select>
            </label>
            <div className="settings-compact-field settings-compact-field--range">
              <label className="settings-range" htmlFor="taskbar-transparency">
                <span>{copy.taskbarTray.transparency}</span>
                <output htmlFor="taskbar-transparency">{transparency.value}%</output>
              </label>
              <input
                id="taskbar-transparency"
                type="range"
                min="0"
                max="80"
                step="1"
                value={transparency.value}
                aria-label={copy.taskbarTray.transparency}
                aria-valuetext={copy.taskbarTray.transparencyValue(transparency.value)}
                onChange={() => undefined}
                onInput={transparency.onInput}
                onPointerDown={transparency.onPointerDown}
                onPointerUp={transparency.onPointerUp}
                onPointerCancel={transparency.onPointerCancel}
                onKeyDown={transparency.onKeyDown}
                onKeyUp={transparency.onKeyUp}
                onBlur={transparency.onBlur}
              />
              <div className="settings-range__ticks" aria-hidden="true">
                <span>0</span><span>20</span><span>40</span><span>60</span><span>80</span>
              </div>
              <p className="settings-preference-group__hint">
                {copy.taskbarTray.transparencyHelp}
              </p>
            </div>
          </div>
          {hasTransparencySaveError ? (
            <p className="settings-preference-group__error" role="alert">
              {copy.taskbarTray.transparencySaveFailed}
            </p>
          ) : null}
        </fieldset>

        <fieldset className="settings-preference-group">
          <legend>{copy.taskbarTray.trayLegend}</legend>
          <p className="settings-preference-group__description">
            {copy.taskbarTray.trayDescription}
          </p>
          <label className="settings-compact-field" htmlFor="tray-icon-mode">
            <span>{copy.taskbarTray.trayIconMode}</span>
            <select
              id="tray-icon-mode"
              value={settings.taskbarTray.trayIconMode}
              onChange={(event) =>
                patchTaskbarTray(
                  "trayIconMode",
                  event.target.value as TaskbarTrayPreferencesDto["trayIconMode"],
                )
              }
            >
              <option value="dynamic">{copy.taskbarTray.trayIconOptions[0]}</option>
              <option value="monochrome">{copy.taskbarTray.trayIconOptions[1]}</option>
            </select>
          </label>
          <p className="settings-preference-group__subheading">
            {copy.taskbarTray.tooltipRows}
          </p>
          <div className="settings-preference-grid">
            {tooltipItems.map(([field, label]) => (
              <label className="settings-switch" key={field}>
                <input
                  type="checkbox"
                  checked={settings.taskbarTray[field]}
                  onChange={(event) => patchTaskbarTray(field, event.target.checked)}
                />
                {label}
              </label>
            ))}
          </div>
        </fieldset>

        <fieldset className="settings-preference-group">
          <legend>{copy.taskbarTray.fullscreenLegend}</legend>
          <p className="settings-preference-group__description">
            {copy.taskbarTray.fullscreenDescription}
          </p>
          <label className="settings-switch">
            <input
              type="checkbox"
              checked={settings.taskbarTray.hideStatusSurfacesInFullscreen}
              onChange={(event) =>
                patchTaskbarTray(
                  "hideStatusSurfacesInFullscreen",
                  event.target.checked,
                )
              }
            />
            {copy.taskbarTray.hideInFullscreen}
          </label>
        </fieldset>
      </div>
    </section>
  );
}
