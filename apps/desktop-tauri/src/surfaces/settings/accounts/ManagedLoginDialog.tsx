import { useEffect, useRef } from "react";
import type { ManagedLoginStateDto } from "../../../types/bridge";
import { settingsCopy, type SettingsCopy } from "../settingsCopy";

export interface ManagedLoginDialogProps { open: boolean; state: ManagedLoginStateDto | null; onStart(method: "browser" | "deviceCode"): void; onCancel(): void; onClose(): void; copy?: SettingsCopy; }

export default function ManagedLoginDialog({ open, state, onStart, onCancel, onClose, copy = settingsCopy("en-US") }: ManagedLoginDialogProps) {
  const browserButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => { if (!open) return; const previous = document.activeElement as HTMLElement | null; browserButtonRef.current?.focus(); return () => previous?.focus(); }, [open]);
  if (!open) return null;
  const stage = state?.stage ?? "starting";
  const failedBrowser = stage === "failed";
  return <div role="dialog" aria-modal="true" aria-label={copy.login.dialogLabel} className="login-dialog">
    <h2>{copy.login.title}</h2>
    {stage === "starting" ? <p>{copy.login.starting}</p> : null}
    {stage === "awaitingUser" ? <div><p>{copy.login.returnAfterSignIn}</p>{state?.verificationUrl ? <p className="login-dialog__url">{state.verificationUrl}</p> : null}{state?.userCode ? <p className="login-dialog__code">{copy.login.code}: <strong>{state.userCode}</strong></p> : null}<button type="button" onClick={onCancel}>{copy.login.cancel}</button></div> : null}
    {stage === "succeeded" ? <p>{copy.login.succeeded}</p> : null}
    {stage === "cancelled" ? <p>{copy.login.cancelled}</p> : null}
    {failedBrowser ? <div><p>{copy.login.failed}</p><button type="button" onClick={() => onStart("deviceCode")}>{copy.login.retryWithDeviceCode}</button></div> : null}
    {stage === "starting" || stage === "failed" ? <p className="settings-field"><button ref={browserButtonRef} type="button" onClick={() => onStart("browser")}>{copy.login.browser}</button><button type="button" onClick={() => onStart("deviceCode")}>{copy.login.deviceCode}</button></p> : null}
    <button type="button" onClick={onClose}>{copy.login.close}</button>
  </div>;
}
