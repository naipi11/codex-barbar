import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  dismissTrayPanel,
  exportDiagnostics,
  getBootstrapState,
  openCodexUsagePage,
  openSettingsWindow,
  quitApp,
  setFlyoutSize,
} from "../lib/tauri";
import type { BootstrapDto } from "../types/bridge";
import { useProfileUsage } from "../hooks/useProfileUsage";
import { useTheme } from "../hooks/useTheme";
import ProfileSelector from "./tray/ProfileSelector";
import QuotaCard from "./tray/QuotaCard";
import TrayActions from "./tray/TrayActions";
import TrayHeader from "./tray/TrayHeader";
import UsageStatus from "./tray/UsageStatus";
import { trayCopy } from "./tray/copy";
import "./tray/TrayPanel.css";

const TRAY_PANEL_MIN_WIDTH = 320;
const TRAY_PANEL_MAX_WIDTH = 720;
const TRAY_PANEL_MIN_HEIGHT = 320;
const TRAY_PANEL_MAX_HEIGHT = 900;

export interface TrayPanelSize {
  width: number;
  height: number;
}

export function measureTrayPanelSize(
  element: Pick<HTMLElement, "clientWidth" | "scrollHeight">,
): TrayPanelSize {
  return {
    width: Math.max(
      TRAY_PANEL_MIN_WIDTH,
      Math.min(TRAY_PANEL_MAX_WIDTH, Math.ceil(element.clientWidth)),
    ),
    height: Math.max(
      TRAY_PANEL_MIN_HEIGHT,
      Math.min(TRAY_PANEL_MAX_HEIGHT, Math.ceil(element.scrollHeight)),
    ),
  };
}

function useTrayPanelAutoSize(ready: boolean) {
  const panelRef = useRef<HTMLElement | null>(null);

  useLayoutEffect(() => {
    const element = panelRef.current;
    if (!ready || !element) return;

    let lastSize: TrayPanelSize | null = null;
    const resize = () => {
      const size = measureTrayPanelSize(element);
      if (size.width === lastSize?.width && size.height === lastSize?.height) return;
      lastSize = size;
      void setFlyoutSize(size.width, size.height);
    };

    resize();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ready]);

  return panelRef;
}


function systemTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

function TrayDashboard({ bootstrap }: { bootstrap: BootstrapDto }) {
  const panelRef = useTrayPanelAutoSize(true);
  const usage = useProfileUsage(bootstrap);
  useTheme(bootstrap.settings.theme);
  const language = bootstrap.settings.language;
  const copy = useMemo(() => trayCopy(language), [language]);
  const locale =
    language === "zh-CN"
      ? "zh-CN"
      : language === "en-US"
        ? "en-US"
        : navigator.language || "en-US";
  const timeZone = systemTimeZone();
  const panel = bootstrap.settings.panel;

  const selectedProfile = usage.profiles.find(
    (profile) => profile.id === usage.selectedProfileId,
  );
  const primary = usage.state.primary;
  const secondary = usage.state.secondary;
  return (
    <main
      ref={panelRef}
      className={`tray-panel tray-panel--macos tray-panel--${panel.density}`}
      data-density={panel.density}
      aria-label="codex-barbar tray panel"
    >
      <TrayHeader
        productName="codex-barbar"
        version={bootstrap.version}
        profile={selectedProfile ?? null}
        copy={copy}
        showAccountStatus={panel.showAccountStatus}
        onDismiss={dismissTrayPanel}
      />

      <div className="tray-stack">
      <ProfileSelector
        profiles={usage.profiles}
        selectedProfileId={usage.selectedProfileId}
        copy={copy}
        onSelect={usage.selectProfile}
        autoFocus={usage.profiles.length > 0}
      />

      {primary ? (
        <QuotaCard
          window={primary}
          displayMode={bootstrap.settings.displayMode}
          copy={copy}
          locale={locale}
          timeZone={timeZone}
          showResetTime={panel.showResetTime}
        />
      ) : null}
      {secondary ? (
        <QuotaCard
          window={secondary}
          displayMode={bootstrap.settings.displayMode}
          copy={copy}
          locale={locale}
          timeZone={timeZone}
          showResetTime={panel.showResetTime}
        />
      ) : null}

      </div>

      <UsageStatus
        state={usage.state}
        isSwitching={usage.isSwitching}
        copy={copy}
        locale={locale}
        showFreshness={panel.showFreshness}
        onRefresh={usage.refresh}
        onOpenSettings={openSettingsWindow}
        onExportDiagnostics={async () => {
          await exportDiagnostics();
        }}
        onOpenUsage={openCodexUsagePage}
      />

      <TrayActions
        copy={copy}
        order={panel.actions.order}
        onRefresh={usage.refresh}
        onOpenUsage={openCodexUsagePage}
        onOpenSettings={openSettingsWindow}
        onDismiss={dismissTrayPanel}
        onQuit={quitApp}
        autoFocusRefresh={usage.profiles.length === 0}
      />

      {selectedProfile ? (
        <span className="sr-only" aria-live="polite">
          {selectedProfile.presentationName}
        </span>
      ) : null}
    </main>
  );
}

export default function TrayPanel() {
  const [bootstrap, setBootstrap] = useState<BootstrapDto | null>(null);
  const [bootstrapError, setBootstrapError] = useState(false);

  useEffect(() => {
    let active = true;
    getBootstrapState()
      .then((state) => {
        if (active) setBootstrap(state);
      })
      .catch(() => {
        if (active) setBootstrapError(true);
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void dismissTrayPanel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  if (!bootstrap) {
    return (
      <main className="tray-panel tray-panel--macos" aria-label="codex-barbar tray panel">
        <TrayHeader
          productName="codex-barbar"
          version="…"
          profile={null}
          copy={trayCopy("system")}
          onDismiss={dismissTrayPanel}
        />
        <p role={bootstrapError ? "alert" : undefined}>
          {bootstrapError ? "Unable to load CodexBar state." : "Loading…"}
        </p>
      </main>
    );
  }

  return <TrayDashboard bootstrap={bootstrap} />;
}
