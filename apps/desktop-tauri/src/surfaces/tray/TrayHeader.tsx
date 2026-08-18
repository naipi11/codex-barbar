import type { ProfileSummaryDto } from "../../types/bridge";
import { profileDisplayName, profileStatusLabel } from "../../hooks/useStatusSurface";
import type { TrayCopy } from "./copy";

interface TrayHeaderProps {
  productName: string;
  version: string;
  profile: ProfileSummaryDto | null;
  copy: TrayCopy;
  onDismiss(): Promise<void> | void;
}

function initials(profile: ProfileSummaryDto | null): string {
  const name = profileDisplayName(profile).trim();
  const letters = name
    .split(/\s+/)
    .map((part) => part[0])
    .filter((value): value is string => Boolean(value))
    .slice(0, 2)
    .join("")
    .toUpperCase();
  return letters || "C";
}

export default function TrayHeader({
  productName,
  version,
  profile,
  copy,
  onDismiss,
}: TrayHeaderProps) {
  const identity = profileDisplayName(profile);
  const secondary =
    profile?.accountEmail && profile.accountEmail !== identity
      ? profile.accountEmail
      : profileStatusLabel(profile);

  return (
    <header className="tray-panel__header">
      <div className="tray-panel__identity">
        <span className="tray-panel__avatar" aria-hidden="true">
          {initials(profile)}
        </span>
        <div className="tray-panel__title-group">
          <h1>{productName}</h1>
          <p className="tray-panel__identity-name">{identity}</p>
          <p className="tray-panel__identity-status">{secondary}</p>
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
          <span aria-hidden="true">×</span>
        </button>
      </div>
    </header>
  );
}
