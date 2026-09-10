import { useState } from "react";
import {
  exportDiagnostics,
  getDiagnosticsSummary,
  validateCodexExecutable,
} from "../../../lib/tauri";
import type {
  AppSettingsDto,
  DiagnosticsSummaryDto,
} from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

function formatCounts(values: Record<string, number>, empty: string): string {
  const entries = Object.entries(values);
  return entries.length
    ? entries.map(([key, value]) => `${key}: ${value}`).join(", ")
    : empty;
}

export default function AdvancedTab({
  settings,
  copy = settingsCopy("en-US"),
}: {
  settings: AppSettingsDto;
  copy?: SettingsCopy;
}) {
  const [path, setPath] = useState(settings.codexExecutableOverride ?? "");
  const [result, setResult] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsSummaryDto | null>(
    null,
  );
  const [busy, setBusy] = useState(false);

  const inspectEnvironment = () => {
    setBusy(true);
    setResult(null);
    void getDiagnosticsSummary()
      .then(setDiagnostics)
      .catch(() => setResult(copy.advanced.diagnosticsFailed))
      .finally(() => setBusy(false));
  };

  return (
    <section aria-label={`${copy.advanced.title} settings`}>
      <h2>{copy.advanced.title}</h2>
      <p className="settings-field">
        <label htmlFor="codex-path">{copy.advanced.executablePath}</label>
        <input
          id="codex-path"
          value={path}
          onChange={(event) => setPath(event.target.value)}
          placeholder={copy.advanced.executablePlaceholder}
        />
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void validateCodexExecutable(path)
              .then((compatibility) =>
                setResult(
                  compatibility.status === "compatible"
                    ? copy.advanced.compatible(
                        compatibility.version ?? copy.advanced.unknownVersion,
                      )
                    : compatibility.status === "notFound"
                      ? copy.advanced.notFound
                      : copy.advanced.unsupported,
                ),
              )
              .catch(() => setResult(copy.advanced.validationFailed))
              .finally(() => setBusy(false));
          }}
        >
          {copy.advanced.validateAndSave}
        </button>
      </p>
      {result ? <p role="status">{result}</p> : null}
      <p className="settings-field">
        <button type="button" disabled={busy} onClick={inspectEnvironment}>
          {busy ? copy.advanced.diagnosing : copy.advanced.diagnoseEnvironment}
        </button>
      </p>
      {diagnostics ? (
        <section
          className="settings-diagnostics"
          aria-label={copy.advanced.diagnosticsTitle}
        >
          <h3>{copy.advanced.diagnosticsTitle}</h3>
          <dl>
            <div>
              <dt>{copy.advanced.diagnosticsVersion}</dt>
              <dd>{diagnostics.codexVersion ?? copy.advanced.unknownVersion}</dd>
            </div>
            <div>
              <dt>{copy.advanced.diagnosticsPath}</dt>
              <dd>{diagnostics.resolvedPathClass}</dd>
            </div>
            <div>
              <dt>{copy.advanced.diagnosticsProfiles}</dt>
              <dd>{diagnostics.profileCount}</dd>
            </div>
            <div>
              <dt>{copy.advanced.diagnosticsAccountStatuses}</dt>
              <dd>
                {formatCounts(
                  diagnostics.accountStatuses,
                  copy.advanced.diagnosticsNone,
                )}
              </dd>
            </div>
            <div>
              <dt>{copy.advanced.diagnosticsErrors}</dt>
              <dd>
                {diagnostics.errorCodes.length
                  ? diagnostics.errorCodes.join(", ")
                  : copy.advanced.diagnosticsNone}
              </dd>
            </div>
          </dl>
        </section>
      ) : null}
      <p className="settings-field">
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setResult(null);
            void exportDiagnostics()
              .then((exported) => setResult(copy.advanced.exported(exported.path)))
              .catch(() => setResult(copy.advanced.exportFailedFriendly))
              .finally(() => setBusy(false));
          }}
        >
          {copy.advanced.exportDiagnostics}
        </button>
      </p>
    </section>
  );
}
