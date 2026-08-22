import { useState } from "react";
import { sendTestNotification } from "../../../lib/tauri";
import type { AppSettingsDto, SettingsPatchDto } from "../../../types/bridge";
import type { SettingsCopy } from "../settingsCopy";

type NotificationBooleanField =
  | "enabled"
  | "playSound"
  | "warningEnabled"
  | "dangerEnabled"
  | "weeklyResetEnabled"
  | "resetCreditIncreaseEnabled"
  | "refreshFailureEnabled"
  | "updateAvailableEnabled";

function NotificationSwitch({
  label,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange(checked: boolean): void;
}) {
  return (
    <label className="settings-switch">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      {label}
    </label>
  );
}

export default function NotificationsTab({
  settings,
  update,
  copy,
  sendTest = sendTestNotification,
}: {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<unknown>;
  copy: SettingsCopy;
  sendTest?: () => Promise<void>;
}) {
  const [error, setError] = useState<string | null>(null);
  const [testSent, setTestSent] = useState(false);
  const [testing, setTesting] = useState(false);
  const notifications = settings.notifications;

  const saveBoolean = (field: NotificationBooleanField, value: boolean) => {
    setError(null);
    setTestSent(false);
    void update({ notifications: { [field]: value } }).catch(() => {
      setError(copy.notifications.saveFailed);
    });
  };

  const saveThreshold = (
    field: "warningRemainingPercent" | "dangerRemainingPercent",
    value: number,
  ) => {
    setError(null);
    setTestSent(false);
    void update({ notifications: { [field]: value } }).catch(() => {
      setError(copy.notifications.thresholdInvalid);
    });
  };

  const runTest = () => {
    setError(null);
    setTestSent(false);
    setTesting(true);
    void sendTest()
      .then(() => setTestSent(true))
      .catch(() => setError(copy.notifications.testFailed))
      .finally(() => setTesting(false));
  };

  return (
    <section aria-label={`${copy.notifications.title} settings`}>
      <h2>{copy.notifications.title}</h2>

      <article className="settings-status-card">
        <div className="settings-status-card__heading">
          <h3>{copy.notifications.masterTitle}</h3>
          <p>{copy.notifications.masterDescription}</p>
        </div>
        <NotificationSwitch
          label={copy.notifications.enable}
          checked={notifications.enabled}
          onChange={(value) => saveBoolean("enabled", value)}
        />
      </article>

      <article className="settings-status-card">
        <div className="settings-status-card__heading">
          <h3>{copy.notifications.eventsTitle}</h3>
          <p>{copy.notifications.eventsDescription}</p>
        </div>
        <NotificationSwitch
          label={copy.notifications.warning}
          checked={notifications.warningEnabled}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("warningEnabled", value)}
        />
        <NotificationSwitch
          label={copy.notifications.danger}
          checked={notifications.dangerEnabled}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("dangerEnabled", value)}
        />
        <NotificationSwitch
          label={copy.notifications.weeklyReset}
          checked={notifications.weeklyResetEnabled}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("weeklyResetEnabled", value)}
        />
        <NotificationSwitch
          label={copy.notifications.resetCreditIncrease}
          checked={notifications.resetCreditIncreaseEnabled}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("resetCreditIncreaseEnabled", value)}
        />
        <NotificationSwitch
          label={copy.notifications.refreshFailure}
          checked={notifications.refreshFailureEnabled}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("refreshFailureEnabled", value)}
        />
        <NotificationSwitch
          label={copy.notifications.updateAvailable}
          checked={notifications.updateAvailableEnabled}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("updateAvailableEnabled", value)}
        />
      </article>

      <article className="settings-status-card">
        <div className="settings-status-card__heading">
          <h3>{copy.notifications.thresholdsTitle}</h3>
          <p>{copy.notifications.thresholdHelp}</p>
        </div>
        <p className="settings-field">
          <label htmlFor="notification-warning-threshold">
            {copy.notifications.warningThreshold}
          </label>
          <input
            id="notification-warning-threshold"
            type="number"
            min="0"
            max="100"
            step="1"
            value={notifications.warningRemainingPercent}
            disabled={!notifications.enabled}
            onChange={(event) =>
              saveThreshold("warningRemainingPercent", Number(event.target.value))
            }
          />
        </p>
        <p className="settings-field">
          <label htmlFor="notification-danger-threshold">
            {copy.notifications.dangerThreshold}
          </label>
          <input
            id="notification-danger-threshold"
            type="number"
            min="0"
            max="100"
            step="1"
            value={notifications.dangerRemainingPercent}
            disabled={!notifications.enabled}
            onChange={(event) =>
              saveThreshold("dangerRemainingPercent", Number(event.target.value))
            }
          />
        </p>
        <NotificationSwitch
          label={copy.notifications.playSound}
          checked={notifications.playSound}
          disabled={!notifications.enabled}
          onChange={(value) => saveBoolean("playSound", value)}
        />
      </article>

      <article className="settings-status-card">
        <div className="settings-status-card__heading">
          <h3>{copy.notifications.sendTest}</h3>
          <p>{copy.notifications.testDescription}</p>
        </div>
        <button type="button" disabled={testing} onClick={runTest}>
          {copy.notifications.sendTest}
        </button>
        {testSent ? <p role="status">{copy.notifications.testSent}</p> : null}
      </article>

      {error ? <p role="alert">{error}</p> : null}
    </section>
  );
}
