import { useState } from "react";
import { checkForUpdates, openReleasePage } from "../../../lib/tauri";
import type { ManualUpdateResult } from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

export default function AboutTab({ copy = settingsCopy("en-US") }: { copy?: SettingsCopy }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ManualUpdateResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  return <section aria-label={copy.about.title}>
    <h2>{copy.about.title}</h2><p>{copy.about.description}</p><p>{copy.about.license}</p>
    <p className="settings-field"><button type="button" disabled={busy} onClick={() => { setBusy(true); setError(null); void checkForUpdates().then(setResult).catch(() => setError(copy.about.updateCheckFailed)).finally(() => setBusy(false)); }}>{busy ? copy.about.checking : copy.about.checkForUpdates}</button><button type="button" onClick={() => void openReleasePage()}>{copy.about.openReleases}</button></p>
    {error ? <p role="alert">{error}</p> : null}
    {result ? <p role="status">{result.status === "available" ? copy.about.updateAvailable(result.latestVersion) : result.status === "current" ? copy.about.updateCurrent : copy.about.updateUnavailable}</p> : null}
  </section>;
}
