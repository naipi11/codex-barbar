import { useEffect, useRef, useState } from "react";
import type { ManagedLoginStateDto, ProfileSummaryDto } from "../../../types/bridge";
import ManagedLoginDialog from "../accounts/ManagedLoginDialog";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";
import AccountAvatar from "../../../components/AccountAvatar";
import { clearProfileAvatar, saveProfileAvatar } from "../../../lib/tauri";

const MAX_AVATAR_BYTES = 1024 * 1024;

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => {
      const value = reader.result;
      if (typeof value === "string") resolve(value);
      else reject(new Error("avatar data unavailable"));
    });
    reader.addEventListener("error", () => reject(reader.error ?? new Error("avatar read failed")));
    reader.readAsDataURL(file);
  });
}

function cleanIdentity(value: string | null): string | null {
  const trimmed = value?.trim();
  return !trimmed || /^current[\s_-]*cli$/i.test(trimmed) ? null : trimmed;
}

function profileIdentity(profile: ProfileSummaryDto): string | null {
  return cleanIdentity(profile.presentationName);
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
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;
  const [avatarPreviewUri, setAvatarPreviewUri] = useState<string | null>(
    selectedProfile?.avatarAssetUri ?? null,
  );
  const [avatarStatus, setAvatarStatus] = useState<string | null>(null);
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const profileLabel = (profile: ProfileSummaryDto) => profileIdentity(profile) ?? (profile.kind === "currentCli" ? copy.accounts.signedOut : copy.accounts.managed);

  useEffect(() => {
    setAvatarPreviewUri(selectedProfile?.avatarAssetUri ?? null);
    setAvatarStatus(null);
  }, [selectedProfile?.id, selectedProfile?.avatarAssetUri]);

  const handleAvatarFile = async (file: File) => {
    if (!selectedProfile) {
      setAvatarStatus(copy.accounts.avatarUnavailable);
      return;
    }
    if (file.type !== "image/png" || !file.name.toLowerCase().endsWith(".png")) {
      setAvatarStatus(copy.accounts.avatarInvalid);
      return;
    }
    if (file.size > MAX_AVATAR_BYTES) {
      setAvatarStatus(copy.accounts.avatarTooLarge);
      return;
    }
    setAvatarStatus(null);
    try {
      const dataUrl = await readFileAsDataUrl(file);
      if (!dataUrl.startsWith("data:image/png;base64,")) {
        setAvatarStatus(copy.accounts.avatarInvalid);
        return;
      }
      await saveProfileAvatar(selectedProfile.id, dataUrl);
      setAvatarPreviewUri(dataUrl);
      setAvatarStatus(copy.accounts.avatarSaved);
    } catch {
      setAvatarStatus(copy.accounts.avatarFailed);
    }
  };

  const handleClearAvatar = async () => {
    if (!selectedProfile) {
      setAvatarStatus(copy.accounts.avatarUnavailable);
      return;
    }
    try {
      await clearProfileAvatar(selectedProfile.id);
      setAvatarPreviewUri(null);
      setAvatarStatus(copy.accounts.avatarRestored);
    } catch {
      setAvatarStatus(copy.accounts.avatarFailed);
    }
  };

  return <section aria-label={`${copy.accounts.title} settings`}>
    <h2>{copy.accounts.title}</h2>
    <ul className="account-list">
      {profiles.map((profile) => <li key={profile.id} className="account-row">
        <button type="button" aria-pressed={profile.id === selectedProfileId} onClick={() => onSelect(profile.id)}>
          {profileLabel(profile)}{profile.id === selectedProfileId ? ` (${copy.accounts.selected})` : ""}
        </button>
        {profile.kind === "managed" ? <>
          <span className="account-row__actions">
            <button type="button" onClick={() => { const label = window.prompt(copy.accounts.renamePrompt, profileLabel(profile)); if (label?.trim()) void onRename(profile.id, label.trim()); }}>{copy.accounts.rename}</button>
            <button type="button" onClick={() => { if (window.confirm(copy.accounts.removeConfirm(profileLabel(profile)))) void onRemove(profile.id); }}>{copy.accounts.remove}</button>
          </span>
        </> : null}
      </li>)}
    </ul>
    <p className="settings-field">
      <label htmlFor="new-account-label">{copy.accounts.newAccountLabel}</label>
      <input id="new-account-label" value={newLabel} onChange={(event) => setNewLabel(event.target.value)} />
      <button ref={addButtonRef} type="button" onClick={() => setDialogOpen(true)}>{copy.accounts.addAccount}</button>
    </p>
    <fieldset className="settings-preference-group account-avatar-settings">
      <legend>{copy.accounts.avatarTitle}</legend>
      <p className="settings-preference-group__description">{copy.accounts.avatarDescription}</p>
      <div className="account-avatar-settings__content">
        <AccountAvatar
          identity={selectedProfile ? { ...selectedProfile, avatarKind: avatarPreviewUri ? "manual" : selectedProfile.avatarKind, avatarAssetUri: avatarPreviewUri } : null}
          size={64}
          decorative
        />
        <div className="account-avatar-settings__actions">
          <label className="settings-button" htmlFor="profile-avatar-input">{copy.accounts.avatarInput}</label>
          <input
            id="profile-avatar-input"
            type="file"
            accept="image/png,.png"
            hidden
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) void handleAvatarFile(file);
            }}
            disabled={!selectedProfile}
          />
          <button
            type="button"
            onClick={() => void handleClearAvatar()}
            disabled={!selectedProfile || (!avatarPreviewUri && selectedProfile.avatarKind === "default")}
          >
            {copy.accounts.avatarRestore}
          </button>
          {avatarStatus ? <span role="status">{avatarStatus}</span> : null}
        </div>
      </div>
    </fieldset>
    <ManagedLoginDialog open={dialogOpen} state={loginState} copy={copy} onStart={(method) => onStartLogin(method, newLabel.trim() || copy.accounts.managed)} onCancel={onCancelLogin} onClose={() => { setDialogOpen(false); addButtonRef.current?.focus(); }} />
  </section>;
}
