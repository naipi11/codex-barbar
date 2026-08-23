import { useEffect, useRef, useState } from "react";
import { applyMenuPreferences } from "../../../lib/tauri";
import type {
  AppSettingsDto,
  MenuLayoutDto,
  MenuPreferencesPatchDto,
} from "../../../types/bridge";
import type { SettingsCopy } from "../settingsCopy";

export const NATIVE_TRAY_ORDER = [
  "open_panel",
  "refresh",
  "accounts",
  "open_usage",
  "settings",
  "about",
  "quit",
] as const;

export const TRAY_PANEL_ORDER = [
  "refresh",
  "open_usage",
  "settings",
  "dismiss",
  "quit",
] as const;

const REQUIRED_NATIVE_ITEMS = new Set(["settings", "quit"]);

interface MenuRow {
  id: string;
  label: string;
  hidden: boolean;
  required: boolean;
}

function rowsFor(
  layout: MenuLayoutDto,
  registry: readonly string[],
  required: ReadonlySet<string>,
  labels: Record<string, string>,
): MenuRow[] {
  const hidden = new Set(layout.hidden);
  const requiredVisible = registry.filter((id) => required.has(id));
  const visibleOrder = layout.order.filter(
    (id) => registry.includes(id) && !hidden.has(id) && !required.has(id),
  );
  const hiddenOrder = layout.order.filter(
    (id) => registry.includes(id) && hidden.has(id) && !required.has(id),
  );
  const remainingHidden = registry.filter(
    (id) => !required.has(id) && !layout.order.includes(id) && hidden.has(id),
  );
  const ordered = [...requiredVisible, ...visibleOrder, ...hiddenOrder, ...remainingHidden];
  const seen = new Set<string>();
  const uniqueOrdered = ordered.filter((id) => {
    if (seen.has(id)) return false;
    seen.add(id);
    return true;
  });
  return uniqueOrdered.map((id) => ({
    id,
    label: labels[id] ?? id,
    hidden: hidden.has(id) && !required.has(id),
    required: required.has(id),
  }));
}

export default function MenuTab({
  settings,
  copy,
}: {
  settings: AppSettingsDto;
  copy: SettingsCopy;
}) {
  const menuCopy = copy.menu;
  const [saveError, setSaveError] = useState(false);
  const [saving, setSaving] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const commit = async (patch: MenuPreferencesPatchDto) => {
    if (saving) return;
    setSaving(true);
    setSaveError(false);
    try {
      await applyMenuPreferences(patch);
    } catch {
      if (mounted.current) setSaveError(true);
    } finally {
      if (mounted.current) setSaving(false);
    }
  };

  const moveRow = (
    key: "nativeTray" | "trayPanel",
    layout: MenuLayoutDto,
    id: string,
    direction: -1 | 1,
  ) => {
    const order = [...layout.order];
    const index = order.indexOf(id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    void commit({ [key]: { order } });
  };

  const reorderRow = (
    key: "nativeTray" | "trayPanel",
    layout: MenuLayoutDto,
    nextVisibleOrder: string[],
  ) => {
    const required = key === "nativeTray" ? REQUIRED_NATIVE_ITEMS : new Set<string>();
    const hidden = new Set(layout.hidden);
    const visible = nextVisibleOrder.filter(
      (id) => !required.has(id) && !hidden.has(id) && layout.order.includes(id),
    );
    const result: string[] = [];
    let visibleIndex = 0;
    for (const id of layout.order) {
      if (!required.has(id) && !hidden.has(id)) {
        result.push(visible[visibleIndex] ?? id);
        visibleIndex += 1;
      } else {
        result.push(id);
      }
    }
    void commit({ [key]: { order: result } });
  };

  const toggleRow = (
    key: "nativeTray" | "trayPanel",
    layout: MenuLayoutDto,
    id: string,
    visible: boolean,
  ) => {
    const hidden = new Set(layout.hidden);
    if (visible) hidden.delete(id);
    else hidden.add(id);
    void commit({ [key]: { hidden: [...hidden] } });
  };

  const restoreDefaults = (key: "nativeTray" | "trayPanel", registry: readonly string[]) => {
    void commit({ [key]: { order: [...registry], hidden: [] } });
  };

  return (
    <section data-testid="menu-tab" aria-label={menuCopy.title + " settings"}>
      <h2>{menuCopy.title}</h2>
      <p className="settings-preference-group__hint">{menuCopy.noCustomCommands}</p>
      {saveError ? (
        <p className="settings-preference-group__error" role="alert">
          {menuCopy.saveFailed}
        </p>
      ) : null}

      <div className="settings-preference-groups">
        <LayoutEditor
          legend={menuCopy.nativeTrayLegend}
          description={menuCopy.nativeTrayDescription}
          layout={settings.menu.nativeTray}
          registry={NATIVE_TRAY_ORDER}
          required={REQUIRED_NATIVE_ITEMS}
          labels={menuCopy.itemLabels}
          copy={menuCopy}
          saving={saving}
          requiredHint={menuCopy.requiredItems}
          onMove={(id, direction) =>
            moveRow("nativeTray", settings.menu.nativeTray, id, direction)
          }
          onReorder={(order) => reorderRow("nativeTray", settings.menu.nativeTray, order)}
          onToggle={(id, visible) =>
            toggleRow("nativeTray", settings.menu.nativeTray, id, visible)
          }
          onRestore={() => restoreDefaults("nativeTray", NATIVE_TRAY_ORDER)}
        />
        <LayoutEditor
          legend={menuCopy.trayPanelLegend}
          description={menuCopy.trayPanelDescription}
          layout={settings.menu.trayPanel}
          registry={TRAY_PANEL_ORDER}
          required={new Set()}
          labels={menuCopy.itemLabels}
          copy={menuCopy}
          saving={saving}
          requiredHint={null}
          onMove={(id, direction) =>
            moveRow("trayPanel", settings.menu.trayPanel, id, direction)
          }
          onReorder={(order) => reorderRow("trayPanel", settings.menu.trayPanel, order)}
          onToggle={(id, visible) =>
            toggleRow("trayPanel", settings.menu.trayPanel, id, visible)
          }
          onRestore={() => restoreDefaults("trayPanel", TRAY_PANEL_ORDER)}
        />
      </div>
    </section>
  );
}

function LayoutEditor({
  legend,
  description,
  layout,
  registry,
  required,
  labels,
  copy,
  saving,
  requiredHint,
  onMove,
  onReorder,
  onToggle,
  onRestore,
}: {
  legend: string;
  description: string;
  layout: MenuLayoutDto;
  registry: readonly string[];
  required: ReadonlySet<string>;
  labels: Record<string, string>;
  copy: SettingsCopy["menu"];
  saving: boolean;
  requiredHint: string | null;
  onMove(id: string, direction: -1 | 1): void;
  onReorder(order: string[]): void;
  onToggle(id: string, visible: boolean): void;
  onRestore(): void;
}) {
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const rows = rowsFor(layout, registry, required, labels);
  const visibleCount = rows.filter((row) => !row.hidden).length;

  const dropRow = (targetId: string) => {
    if (!draggedId || draggedId === targetId) return;
    const visibleOrder = layout.order.filter(
      (id) => !required.has(id) && !layout.hidden.includes(id),
    );
    const from = visibleOrder.indexOf(draggedId);
    const to = visibleOrder.indexOf(targetId);
    if (from < 0 || to < 0) return;
    const next = [...visibleOrder];
    next.splice(from, 1);
    next.splice(to, 0, draggedId);
    onReorder(next);
    setDraggedId(null);
  };

  return (
    <fieldset className="settings-preference-group">
      <legend>{legend}</legend>
      <p className="settings-preference-group__description">{description}</p>
      {requiredHint ? <p className="settings-preference-group__hint">{requiredHint}</p> : null}
      <ul className="menu-layout-editor">
        {rows.map((row, index) => (
          <li
            key={row.id}
            data-menu-row={row.id}
            draggable={!row.hidden && !saving && visibleCount > 1}
            onDragStart={(event) => {
              setDraggedId(row.id);
              event.dataTransfer.effectAllowed = "move";
            }}
            onDragOver={(event) => event.preventDefault()}
            onDrop={() => dropRow(row.id)}
          >
            <label className="settings-switch settings-switch--inline">
              <input
                type="checkbox"
                checked={!row.hidden}
                disabled={row.required || saving}
                aria-label={row.label}
                onChange={(event) => onToggle(row.id, event.target.checked)}
              />
              {row.label}
            </label>
            {!row.hidden ? (
              <span className="menu-layout-editor__move">
                <button
                  type="button"
                  disabled={saving || index === 0 || visibleCount <= 1}
                  aria-label={copy.moveUp + " " + row.label}
                  onClick={() => onMove(row.id, -1)}
                >
                  {copy.moveUp}
                </button>
                <button
                  type="button"
                  disabled={saving || index === rows.length - 1 || visibleCount <= 1}
                  aria-label={copy.moveDown + " " + row.label}
                  onClick={() => onMove(row.id, 1)}
                >
                  {copy.moveDown}
                </button>
              </span>
            ) : null}
          </li>
        ))}
      </ul>
      <button type="button" className="settings-button" disabled={saving} onClick={onRestore}>
        {copy.restoreDefaults}
      </button>
    </fieldset>
  );
}

