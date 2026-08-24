import { getCurrentWindow } from "@tauri-apps/api/window";
import { setFlyoutInteracting } from "../../lib/tauri";
import type { ProfileSummaryDto } from "../../types/bridge";
import { profileDisplayName } from "../../hooks/useStatusSurface";
import AccountAvatar from "../../components/AccountAvatar";
import type { TrayCopy } from "./copy";

interface TrayHeaderProps {
  productName: string;
  version: string;
  profile: ProfileSummaryDto | null;
  copy: TrayCopy;
  showAccountStatus?: boolean;
  onDismiss(): Promise<void> | void;
}

export default function TrayHeader({
  productName,
  version,
  profile,
  copy,
  showAccountStatus = true,
  onDismiss,
}: TrayHeaderProps) {
  const identity = profileDisplayName(profile);
  const secondary = !profile
    ? copy.signedOut
    : profile.accountStatus === "signedIn"
      ? copy.signedIn
      : profile.accountStatus === "signedOut"
        ? copy.signedOut
        : copy.accountUnavailable;

  const beginDrag = () => {
    void setFlyoutInteracting(true).catch(() => undefined);
    void getCurrentWindow().startDragging().finally(() => {
      window.setTimeout(() => {
        void setFlyoutInteracting(false).catch(() => undefined);
      }, 250);
    });
  };

  return (
    <header
      className="tray-panel__header"
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        const target = event.target as HTMLElement | null;
        if (target?.closest("button")) return;
        event.preventDefault();
        beginDrag();
      }}
    >
      <div className="tray-panel__identity">
        <span className="tray-panel__avatar" aria-hidden="true">
          <AccountAvatar identity={profile} size={36} decorative />
        </span>
        <div className="tray-panel__title-group">
          <p className="tray-panel__identity-name">{identity}</p>
          <h1>{productName}</h1>
          {showAccountStatus ? (
            <p className="tray-panel__identity-status">{secondary}</p>
          ) : null}
        </div>
      </div>
      <div className="tray-panel__header-actions">
        <span className="tray-panel__version">v{version}</span>
        <button
          type="button"
          className="tray-panel__close"
          aria-label={copy.hidePanel}
          title={copy.hidePanel}
          onClick={() => void onDismiss()}
        >
          <span aria-hidden="true">x</span>
        </button>
      </div>
    </header>
  );
}
