import { useEffect, useMemo, useState } from "react";
import {
  dismissTrayPanel,
  getBootstrapState,
  openCodexUsagePage,
  openSettingsWindow,
  quitApp,
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

function systemTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

function TrayDashboard({ bootstrap }: { bootstrap: BootstrapDto }) {
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

  const selectedProfile = usage.profiles.find(
    (profile) => profile.id === usage.selectedProfileId,
  );
  const primary = usage.state.primary;
  const secondary = usage.state.secondary;

  return (
    <main className="tray-panel tray-panel--macos" aria-label="codex-barbar tray panel">
      <TrayHeader
        productName="codex-barbar"
        version={bootstrap.version}
        profile={selectedProfile ?? null}
        copy={copy}
        onDismiss={dismissTrayPanel}
      />

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
        />
      ) : null}
      {secondary ? (
        <QuotaCard
          window={secondary}
          displayMode={bootstrap.settings.displayMode}
          copy={copy}
          locale={locale}
          timeZone={timeZone}
        />
      ) : null}

      <UsageStatus
        state={usage.state}
        isSwitching={usage.isSwitching}
        copy={copy}
        locale={locale}
        onRefresh={usage.refresh}
        onOpenSettings={openSettingsWindow}
        onOpenUsage={openCodexUsagePage}
      />

      <TrayActions
        copy={copy}
        onRefresh={usage.refresh}
        onOpenUsage={openCodexUsagePage}
        onOpenSettings={openSettingsWindow}
        onDismiss={dismissTrayPanel}
        onQuit={quitApp}
        autoFocusRefresh={usage.profiles.length === 0}
      />

      {selectedProfile ? (
        <span className="sr-only" aria-live="polite">
          {selectedProfile.label}
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
