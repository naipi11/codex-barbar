import { useRef, useState } from "react";
import type { ManagedLoginStateDto, ProfileSummaryDto } from "../../../types/bridge";
import ManagedLoginDialog from "../accounts/ManagedLoginDialog";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

function cleanIdentity(value: string | null): string | null {
  const trimmed = value?.trim();
  return !trimmed || /^current[\s_-]*cli$/i.test(trimmed) ? null : trimmed;
}

function profileIdentity(profile: ProfileSummaryDto): string | null {
  return cleanIdentity(profile.accountDisplayName) ?? cleanIdentity(profile.accountEmail) ?? cleanIdentity(profile.email);
}

export interface AccountsTabProps {
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  loginState: ManagedLoginStateDto | null;
  onSelect(profileId: string): void;
  onRename(profileId: string, label: string): Promise<unknown>;
  onRemove(profileId: string): Promise<unknown>;
  onStartLogin(method: "browser" | "deviceCode", label: string): void;
  onCancelLogin(): void;
  copy?: SettingsCopy;
}

export default function AccountsTab({ profiles, selectedProfileId, loginState, onSelect, onRename, onRemove, onStartLogin, onCancelLogin, copy = settingsCopy("en-US") }: AccountsTabProps) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [newLabel, setNewLabel] = useState(copy.accounts.managed);
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const profileLabel = (profile: ProfileSummaryDto) => (profile.kind === "currentCli" ? profileIdentity(profile) : null) ?? cleanIdentity(profile.label) ?? (profile.kind === "currentCli" ? copy.accounts.signedOut : copy.accounts.managed);

  return <section aria-label={`${copy.accounts.title} settings`}>
    <h2>{copy.accounts.title}</h2>
    <ul className="account-list">
      {profiles.map((profile) => <li key={profile.id} className="account-row">
        <button type="button" aria-pressed={profile.id === selectedProfileId} onClick={() => onSelect(profile.id)}>
          {profileLabel(profile)}{profile.id === selectedProfileId ? ` (${copy.accounts.selected})` : ""}
        </button>
        {profile.kind === "currentCli" ? <span className="account-row__hint">{profileIdentity(profile) ?? copy.accounts.signedOut}</span> : <>
          {profileIdentity(profile) ? <span className="account-row__hint">{profileIdentity(profile)}</span> : null}
          <span className="account-row__actions">
            <button type="button" onClick={() => { const label = window.prompt(copy.accounts.renamePrompt, profile.label); if (label?.trim()) void onRename(profile.id, label.trim()); }}>{copy.accounts.rename}</button>
            <button type="button" onClick={() => { if (window.confirm(copy.accounts.removeConfirm(profile.label))) void onRemove(profile.id); }}>{copy.accounts.remove}</button>
          </span>
        </>}
      </li>)}
    </ul>
    <p className="settings-field">
      <label htmlFor="new-account-label">{copy.accounts.newAccountLabel}</label>
      <input id="new-account-label" value={newLabel} onChange={(event) => setNewLabel(event.target.value)} />
      <button ref={addButtonRef} type="button" onClick={() => setDialogOpen(true)}>{copy.accounts.addAccount}</button>
    </p>
    <ManagedLoginDialog open={dialogOpen} state={loginState} copy={copy} onStart={(method) => onStartLogin(method, newLabel.trim() || copy.accounts.managed)} onCancel={onCancelLogin} onClose={() => { setDialogOpen(false); addButtonRef.current?.focus(); }} />
  </section>;
}
