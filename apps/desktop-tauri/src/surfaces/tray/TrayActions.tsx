import type { TrayCopy } from "./copy";

interface TrayActionsProps {
  copy: TrayCopy;
  onRefresh(): Promise<void> | void;
  onOpenUsage(): Promise<void> | void;
  onOpenSettings(): Promise<void> | void;
  onDismiss(): Promise<void> | void;
  onQuit(): Promise<void> | void;
  autoFocusRefresh?: boolean;
}

export default function TrayActions({
  copy,
  onRefresh,
  onOpenUsage,
  onOpenSettings,
  onDismiss,
  onQuit,
  autoFocusRefresh = false,
}: TrayActionsProps) {
  return (
    <section className="tray-region tray-actions" role="region" aria-label={copy.actions}>
      <h2>{copy.actions}</h2>
      <div className="tray-actions__buttons">
        <button
          type="button"
          autoFocus={autoFocusRefresh}
          onClick={() => void onRefresh()}
        >
          {copy.refresh}
        </button>
        <button type="button" onClick={() => void onOpenUsage()}>
          {copy.openUsage}
        </button>
        <button type="button" onClick={() => void onOpenSettings()}>
          {copy.settings}
        </button>
        <button type="button" onClick={() => void onDismiss()}>
          {copy.dismiss}
        </button>
        <button type="button" onClick={() => void onQuit()}>
          {copy.quit}
        </button>
      </div>
    </section>
  );
}
