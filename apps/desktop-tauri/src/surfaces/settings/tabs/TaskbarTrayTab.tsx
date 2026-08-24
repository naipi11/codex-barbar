import { useEffect, useRef, useState } from "react";
import { CommittedRangeField } from "../CommittedRangeField";
import type { AppSettingsDto, SettingsPatchDto, StatusSurfaceKind, TaskbarPresentationPreferencesDto } from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

const TASKBAR_ITEM_FIELDS = ["showTaskbarIcon", "showTaskbarAccount", "showWeeklyLabel", "showWeeklyPercent", "showResetDate"] as const satisfies readonly (keyof TaskbarPresentationPreferencesDto)[];

interface TaskbarDraft {
  enabled: boolean;
  presentation: TaskbarPresentationPreferencesDto;
}

function draftFromSettings(settings: AppSettingsDto): TaskbarDraft {
  return { enabled: settings.taskbarStatusEnabled, presentation: settings.taskbarPresentation };
}

export default function TaskbarTrayTab({ settings, update, setSurfaceEnabled, copy = settingsCopy("en-US") }: {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<AppSettingsDto>;
  setSurfaceEnabled(surface: StatusSurfaceKind, enabled: boolean): Promise<AppSettingsDto>;
  copy?: SettingsCopy;
}) {
  const [hasPreferencesSaveError, setHasPreferencesSaveError] = useState(false);
  const [draft, setDraft] = useState<TaskbarDraft>(() => draftFromSettings(settings));
  const [isSavingPreferences, setIsSavingPreferences] = useState(false);
  const draftRef = useRef(draft);
  const isSavingPreferencesRef = useRef(false);

  useEffect(() => {
    if (isSavingPreferencesRef.current) return;
    const nextDraft = draftFromSettings(settings);
    draftRef.current = nextDraft;
    setDraft(nextDraft);
  }, [settings.taskbarStatusEnabled, settings.taskbarPresentation]);

  const visibleTaskbarItems = TASKBAR_ITEM_FIELDS.filter((field) => draft.presentation[field]).length;
  const commitDraft = (nextDraft: TaskbarDraft, persist: () => Promise<AppSettingsDto>) => {
    if (isSavingPreferencesRef.current) return;
    const previousDraft = draftRef.current;
    isSavingPreferencesRef.current = true;
    draftRef.current = nextDraft;
    setDraft(nextDraft);
    setIsSavingPreferences(true);
    void persist().then((saved) => {
      const acknowledgedDraft = draftFromSettings(saved);
      draftRef.current = acknowledgedDraft;
      setDraft(acknowledgedDraft);
      setHasPreferencesSaveError(false);
    }).catch(() => {
      draftRef.current = previousDraft;
      setDraft(previousDraft);
      setHasPreferencesSaveError(true);
    }).finally(() => {
      isSavingPreferencesRef.current = false;
      setIsSavingPreferences(false);
    });
  };
  const setTaskbarEnabled = (enabled: boolean) => {
    const nextDraft = { ...draftRef.current, enabled };
    commitDraft(nextDraft, () => setSurfaceEnabled("taskbarStatus", enabled));
  };
  const patchTaskbarPresentation = <K extends keyof TaskbarPresentationPreferencesDto>(field: K, value: TaskbarPresentationPreferencesDto[K]) => {
    const nextDraft = { ...draftRef.current, presentation: { ...draftRef.current.presentation, [field]: value } };
    commitDraft(nextDraft, () => update({ taskbarPresentation: { [field]: value } }));
  };
  const taskbarItems = [
    ["showTaskbarIcon", copy.taskbarPresentation.showIcon],
    ["showTaskbarAccount", copy.taskbarPresentation.showAccount],
    ["showWeeklyLabel", copy.taskbarPresentation.showWeeklyLabel],
    ["showWeeklyPercent", copy.taskbarPresentation.showWeeklyPercent],
    ["showResetDate", copy.taskbarPresentation.showResetDate],
  ] as const;

  return (
    <section aria-label={`${copy.taskbarPresentation.title} settings`}>
      <h2>{copy.taskbarPresentation.title}</h2>
      {hasPreferencesSaveError ? <p className="settings-preference-group__error" role="alert">{copy.taskbarPresentation.preferencesSaveFailed}</p> : null}
      <div className="settings-preference-groups">
        <fieldset className="settings-preference-group">
          <legend>{copy.taskbarPresentation.taskbarLegend}</legend>
          <p className="settings-preference-group__description">{copy.taskbarPresentation.taskbarDescription}</p>
          <label className="settings-switch settings-switch--primary"><input type="checkbox" checked={draft.enabled} disabled={isSavingPreferences || (!draft.enabled && visibleTaskbarItems === 0)} aria-describedby="taskbar-visible-item-help" onChange={(event) => setTaskbarEnabled(event.target.checked)} />{copy.taskbarPresentation.taskbarEnabled}</label>
          <div className="settings-preference-grid">
            {taskbarItems.map(([field, label]) => {
              const checked = draft.presentation[field];
              return <label className="settings-switch" key={field}><input type="checkbox" checked={checked} disabled={isSavingPreferences || (draft.enabled && checked && visibleTaskbarItems === 1)} aria-describedby="taskbar-visible-item-help" onChange={(event) => patchTaskbarPresentation(field, event.target.checked)} />{label}</label>;
            })}
          </div>
          <p id="taskbar-visible-item-help" className="settings-preference-group__hint">{copy.taskbarPresentation.keepOneVisible}</p>
          <div className="settings-preference-grid settings-preference-grid--controls">
            <label className="settings-compact-field" htmlFor="taskbar-density"><span>{copy.taskbarPresentation.density}</span><select id="taskbar-density" value={draft.presentation.density} disabled={isSavingPreferences} onChange={(event) => patchTaskbarPresentation("density", event.target.value as TaskbarPresentationPreferencesDto["density"])}><option value="compact">{copy.taskbarPresentation.densityOptions[0]}</option><option value="standard">{copy.taskbarPresentation.densityOptions[1]}</option></select></label>
            <CommittedRangeField id="taskbar-transparency" label={copy.taskbarPresentation.transparency} value={settings.taskbarTransparencyPercent} min={0} max={100} tickValues={[0, 25, 50, 75, 100]} valueText={copy.taskbarPresentation.transparencyValue} disabled={isSavingPreferences} errorMessage={copy.taskbarPresentation.transparencySaveFailed} help={copy.taskbarPresentation.transparencyHelp} onCommit={async (nextValue) => (await update({ taskbarTransparencyPercent: nextValue })).taskbarTransparencyPercent} />
          </div>
        </fieldset>
        <fieldset className="settings-preference-group">
          <legend>{copy.taskbarPresentation.floatBallLegend}</legend>
          <p className="settings-preference-group__description">{copy.taskbarPresentation.floatBallDescription}</p>
          <label className="settings-switch settings-switch--primary"><input type="checkbox" checked={settings.floatBallEnabled} onChange={(event) => void setSurfaceEnabled("floatBall", event.target.checked)} />{copy.taskbarPresentation.floatBallEnabled}</label>
          <CommittedRangeField id="float-ball-transparency" label={copy.taskbarPresentation.floatBallTransparency} value={settings.floatBallTransparencyPercent} min={0} max={100} tickValues={[0, 25, 50, 75, 100]} valueText={copy.taskbarPresentation.transparencyValue} errorMessage={copy.taskbarPresentation.transparencySaveFailed} onCommit={async (nextValue) => (await update({ floatBallTransparencyPercent: nextValue })).floatBallTransparencyPercent} />
          <CommittedRangeField id="float-ball-glow" label={copy.taskbarPresentation.floatBallGlow} value={settings.floatBallGlowPercent} min={0} max={100} tickValues={[0, 25, 50, 75, 100]} valueText={copy.taskbarPresentation.glowValue} errorMessage={copy.taskbarPresentation.glowSaveFailed} onCommit={async (nextValue) => (await update({ floatBallGlowPercent: nextValue })).floatBallGlowPercent} />
        </fieldset>
        <fieldset className="settings-preference-group">
          <legend>{copy.taskbarPresentation.fullscreenLegend}</legend>
          <p className="settings-preference-group__description">{copy.taskbarPresentation.fullscreenDescription}</p>
          <label className="settings-switch"><input type="checkbox" checked={draft.presentation.hideStatusSurfacesInFullscreen} disabled={isSavingPreferences} onChange={(event) => patchTaskbarPresentation("hideStatusSurfacesInFullscreen", event.target.checked)} />{copy.taskbarPresentation.hideInFullscreen}</label>
        </fieldset>
      </div>
    </section>
  );
}
