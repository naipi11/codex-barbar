import { useState } from "react";
import { exportDiagnostics, validateCodexExecutable } from "../../../lib/tauri";
import type { AppSettingsDto } from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

export default function AdvancedTab({ settings, copy = settingsCopy("en-US") }: { settings: AppSettingsDto; copy?: SettingsCopy }) {
  const [path, setPath] = useState(settings.codexExecutableOverride ?? "");
  const [result, setResult] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  return <section aria-label={`${copy.advanced.title} settings`}>
    <h2>{copy.advanced.title}</h2>
    <p className="settings-field"><label htmlFor="codex-path">{copy.advanced.executablePath}</label><input id="codex-path" value={path} onChange={(event) => setPath(event.target.value)} placeholder={copy.advanced.executablePlaceholder} />
      <button type="button" disabled={busy} onClick={() => { setBusy(true); void validateCodexExecutable(path).then((compatibility) => setResult(compatibility.status === "compatible" ? copy.advanced.compatible(compatibility.version ?? copy.advanced.unknownVersion) : compatibility.status === "notFound" ? copy.advanced.notFound : copy.advanced.unsupported)).catch(() => setResult(copy.advanced.validationFailed)).finally(() => setBusy(false)); }}>{copy.advanced.validateAndSave}</button>
    </p>
    {result ? <p role="status">{result}</p> : null}
    <p className="settings-field"><button type="button" disabled={busy} onClick={() => { setBusy(true); setResult(null); void exportDiagnostics().then((exported) => setResult(copy.advanced.exported(exported.path))).catch(() => setResult(copy.advanced.exportFailedFriendly)).finally(() => setBusy(false)); }}>{copy.advanced.exportDiagnostics}</button></p>
  </section>;
}
