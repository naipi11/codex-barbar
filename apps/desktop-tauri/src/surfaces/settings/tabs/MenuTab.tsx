import { useEffect, useRef, useState } from "react";
import { applyPanelPreferences } from "../../../lib/tauri";
import type {
  AppSettingsDto,
  MenuLayoutDto,
  PanelPreferencesPatchDto,
} from "../../../types/bridge";
import type { SettingsCopy } from "../settingsCopy";

export const PANEL_ACTION_ORDER = [
  "refresh",
  "open_usage",
  "settings",
  "dismiss",
  "quit",
] as const;

const PANEL_DEFAULTS: PanelPreferencesPatchDto = {
  density: "compact",
  showResetTime: true,
  showFreshness: true,
  showAccountStatus: true,
  actions: {
    order: [...PANEL_ACTION_ORDER],
    hidden: [],
  },
};

interface ActionRow {
  id: (typeof PANEL_ACTION_ORDER)[number];
  label: string;
  hidden: boolean;
  required: boolean;
}

function actionRows(
  layout: MenuLayoutDto,
  labels: Record<string, string>,
): ActionRow[] {
  const hidden = new Set(layout.hidden);
  const knownOrder = layout.order.filter((id) =>
    PANEL_ACTION_ORDER.includes(id as ActionRow["id"]),
  );
  const missing = PANEL_ACTION_ORDER.filter((id) => !knownOrder.includes(id));
  return [...knownOrder, ...missing].map((id) => ({
    id: id as ActionRow["id"],
    label: labels[id] ?? id,
    hidden: hidden.has(id),
    required: id === "refresh",
  }));
}

export default function MenuTab({
  settings,
  copy,
}: {
  settings: AppSettingsDto;
  copy: SettingsCopy;
}) {
  const panelCopy = copy.menu;
  const panel = settings.panel;
  const [saveError, setSaveError] = useState(false);
  const [saving, setSaving] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const commit = async (patch: PanelPreferencesPatchDto) => {
    if (saving) return;
    setSaving(true);
    setSaveError(false);
    try {
      await applyPanelPreferences(patch);
    } catch {
      if (mounted.current) setSaveError(true);
    } finally {
      if (mounted.current) setSaving(false);
    }
  };

  const rows = actionRows(panel.actions, panelCopy.itemLabels);
  const visibleOrder = panel.actions.order.filter(
    (id): id is ActionRow["id"] =>
      PANEL_ACTION_ORDER.includes(id as ActionRow["id"]) &&
      !panel.actions.hidden.includes(id),
  );

  const toggleAction = (id: ActionRow["id"], visible: boolean) => {
    if (id === "refresh") return;
    const hidden = new Set(panel.actions.hidden);
    if (visible) hidden.delete(id);
    else hidden.add(id);
    void commit({ actions: { hidden: [...hidden] } });
  };

  const moveAction = (id: ActionRow["id"], direction: -1 | 1) => {
    const index = visibleOrder.indexOf(id);
    const target = index + direction;
    if (index <= 0 || target <= 0 || target >= visibleOrder.length) return;
    const next = [...visibleOrder];
    [next[index], next[target]] = [next[target], next[index]];
    void commit({ actions: { order: next } });
  };

  return (
    <section data-testid="menu-tab" aria-label={panelCopy.title}>
      <h2>{panelCopy.title}</h2>
      <p className="settings-preference-group__hint">
        {panelCopy.noCustomCommands}
      </p>
      {saveError ? (
        <p className="settings-preference-group__error" role="alert">
          {panelCopy.saveFailed}
        </p>
      ) : null}

      <div className="settings-preference-groups">
        <fieldset className="settings-preference-group panel-layout-settings">
          <legend>{panelCopy.layoutLegend}</legend>
          <p className="settings-preference-group__description">
            {panelCopy.layoutDescription}
          </p>
          <label className="settings-compact-field">
            <span>{panelCopy.density}</span>
            <select
              aria-label={panelCopy.density}
              value={panel.density}
              disabled={saving}
              onChange={(event) =>
                void commit({
                  density: event.target.value as "compact" | "standard",
                })
              }
            >
              <option value="compact">{panelCopy.densityOptions[0]}</option>
              <option value="standard">{panelCopy.densityOptions[1]}</option>
            </select>
          </label>
          <div className="settings-preference-grid panel-detail-grid">
            <PanelToggle
              label={panelCopy.showResetTime}
              checked={panel.showResetTime}
              disabled={saving}
              onChange={(value) => void commit({ showResetTime: value })}
            />
            <PanelToggle
              label={panelCopy.showFreshness}
              checked={panel.showFreshness}
              disabled={saving}
              onChange={(value) => void commit({ showFreshness: value })}
            />
            <PanelToggle
              label={panelCopy.showAccountStatus}
              checked={panel.showAccountStatus}
              disabled={saving}
              onChange={(value) => void commit({ showAccountStatus: value })}
            />
          </div>
        </fieldset>

        <fieldset className="settings-preference-group">
          <legend>{panelCopy.actionsLegend}</legend>
          <p className="settings-preference-group__description">
            {panelCopy.actionsDescription}
          </p>
          <p className="settings-preference-group__hint">
            {panelCopy.refreshRequired}
          </p>
          <ul className="panel-action-editor">
            {rows.map((row) => {
              const visibleIndex = visibleOrder.indexOf(row.id);
              const canMoveUp = visibleIndex > 1;
              const canMoveDown =
                visibleIndex > 0 && visibleIndex < visibleOrder.length - 1;
              return (
                <li key={row.id} data-panel-action={row.id}>
                  <label className="settings-switch settings-switch--inline">
                    <input
                      type="checkbox"
                      checked={!row.hidden}
                      disabled={row.required || saving}
                      aria-label={row.label}
                      onChange={(event) => toggleAction(row.id, event.target.checked)}
                    />
                    {row.label}
                  </label>
                  {!row.hidden && !row.required ? (
                    <span className="panel-action-editor__move">
                      <button
                        type="button"
                        disabled={saving || !canMoveUp}
                        aria-label={`${panelCopy.moveUp} ${row.label}`}
                        onClick={() => moveAction(row.id, -1)}
                      >
                        {panelCopy.moveUp}
                      </button>
                      <button
                        type="button"
                        disabled={saving || !canMoveDown}
                        aria-label={`${panelCopy.moveDown} ${row.label}`}
                        onClick={() => moveAction(row.id, 1)}
                      >
                        {panelCopy.moveDown}
                      </button>
                    </span>
                  ) : null}
                </li>
              );
            })}
          </ul>
          <button
            type="button"
            className="settings-button"
            disabled={saving}
            onClick={() => void commit(PANEL_DEFAULTS)}
          >
            {panelCopy.restoreDefaults}
          </button>
        </fieldset>
      </div>
    </section>
  );
}

function PanelToggle({
  label,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled: boolean;
  onChange(value: boolean): void;
}) {
  return (
    <label className="settings-switch settings-switch--inline">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-label={label}
        onChange={(event) => onChange(event.target.checked)}
      />
      {label}
    </label>
  );
}
