import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  cancelManagedLogin,
  closeSettingsWindow,
  getBootstrapState,
  renameManagedProfile,
  removeManagedProfile,
  selectProfile,
  startManagedLogin,
} from "../lib/tauri";
import { useProfileUsage } from "../hooks/useProfileUsage";
import { useSettings } from "../hooks/useSettings";
import { useTheme } from "../hooks/useTheme";
import type { BootstrapDto } from "../types/bridge";
import GeneralTab from "./settings/tabs/GeneralTab";
import NotificationsTab from "./settings/tabs/NotificationsTab";
import TaskbarTrayTab from "./settings/tabs/TaskbarTrayTab";
import MenuTab from "./settings/tabs/MenuTab";
import AccountsTab from "./settings/tabs/AccountsTab";
import AdvancedTab from "./settings/tabs/AdvancedTab";
import AboutTab from "./settings/tabs/AboutTab";
import {
  TAB_IDS,
  isSettingsTabId,
  type SettingsTabId,
} from "./settings/settingsTabs";
import { settingsCopy, type SettingsCopy } from "./settings/settingsCopy";

function initialTab(): SettingsTabId {
  const params = new URLSearchParams(window.location.search);
  const raw = params.get("tab");
  return isSettingsTabId(raw) ? raw : "general";
}

function PlaceholderTab({ tab, copy }: { tab: SettingsTabId; copy: SettingsCopy }) {
  return (
    <section aria-label={`${copy.tabs[tab]} settings`}>
      <h2>{copy.tabs[tab]}</h2>
      <p>{copy.placeholder}</p>
    </section>
  );
}

export default function Settings() {
  const [tab, setTab] = useState<SettingsTabId>(initialTab);
  const [bootstrap, setBootstrap] = useState<BootstrapDto | null>(null);
  const { settings, update, setSurfaceEnabled } = useSettings();
  useTheme(settings.theme);
  const copy = settingsCopy(settings.language);

  useEffect(() => {
    let active = true;
    void getBootstrapState()
      .then((state) => {
        if (active) setBootstrap(state);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const dismiss = () => void closeSettingsWindow();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const target = event.target;
      if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement || target instanceof HTMLTextAreaElement) return;
      dismiss();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    let unlisten: (() => void | Promise<void>) | undefined;
    void listen<string>("settings-change-tab", (event) => {
      if (isSettingsTabId(event.payload)) setTab(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) void unlisten();
    };
  }, []);

  const usage = useProfileUsage(
    bootstrap ?? {
      productName: "codex-barbar",
      version: "1.0.0",
      settings,
      profiles: [],
      selectedProfileId: "",
      usageByProfile: {},
      statusSurfaceFeedback: {
        taskbarStatusCloseFailed: false,
        floatBallCloseFailed: false,
      },
      codex: {
        status: "notChecked",
        installation: null,
        executablePath: null,
        version: null,
        capabilities: {
          accountRead: false,
          rateLimitsRead: false,
          managedLogin: false,
        },
      },
    },
  );

  return (
    <main className="settings-panel" aria-label={copy.title}>
      <header className="settings-panel__header">
        <h1>{copy.title}</h1>
        <button type="button" onClick={dismiss}>
          {copy.close}
        </button>
      </header>
      <div className="settings-panel__body">
        <nav className="settings-tabs" aria-label={copy.navigation}>
          {TAB_IDS.map((id) => (
            <button
              key={id}
              type="button"
              aria-pressed={tab === id}
              data-settings-tab={id}
              onClick={() => setTab(id)}
              onKeyDown={(event) => {
                if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                event.preventDefault();
                const index = TAB_IDS.indexOf(id);
                const offset = event.key === "ArrowDown" ? 1 : -1;
                const next = TAB_IDS[(index + offset + TAB_IDS.length) % TAB_IDS.length];
                setTab(next);
                document.querySelector<HTMLButtonElement>(`[data-settings-tab="${next}"]`)?.focus();
              }}
            >
              {copy.tabs[id]}
            </button>
          ))}
        </nav>
        <div className="settings-panel__pane">
      {tab === "general" ? (
        <GeneralTab
          settings={settings}
          update={update}
          setSurfaceEnabled={setSurfaceEnabled}
          copy={copy}
        />
      ) : null}
      {tab === "providers" ? (
        <AccountsTab
          profiles={usage.profiles}
          selectedProfileId={usage.selectedProfileId}
          loginState={usage.loginState}
          onSelect={(profileId) => void usage.selectProfile(profileId)}
          onRename={(profileId, label) => renameManagedProfile(profileId, label)}
          onRemove={(profileId) => removeManagedProfile(profileId)}
          onStartLogin={(method, label) =>
            void startManagedLogin({ label, method })
          }
          onCancelLogin={() => {
            if (usage.loginState) {
              void cancelManagedLogin(usage.loginState.operationId);
            }
          }}
          copy={copy}
        />
      ) : null}
      {tab === "notifications" ? (
        <NotificationsTab settings={settings} update={update} copy={copy} />
      ) : null}
      {tab === "menuBar" ? (
        <TaskbarTrayTab
          settings={settings}
          update={update}
          setSurfaceEnabled={setSurfaceEnabled}
          copy={copy}
        />
      ) : null}
      {tab === "menu" ? (
        <MenuTab settings={settings} copy={copy} />
      ) : null}
      {tab === "advanced" ? <AdvancedTab settings={settings} copy={copy} /> : null}
      {tab === "about" ? <AboutTab version={bootstrap?.version ?? "—"} copy={copy} /> : null}
      {tab !== "general" &&
       tab !== "providers" &&
       tab !== "notifications" &&
       tab !== "menuBar" &&
       tab !== "menu" &&
      tab !== "advanced" &&
      tab !== "about" ? (
        <PlaceholderTab tab={tab} copy={copy} />
      ) : null}
        </div>
      </div>
    </main>
  );
}
