import type { TrayCopy } from "./copy";

const ACTION_IDS = ["refresh", "open_usage", "settings", "dismiss", "quit"] as const;

interface TrayActionsProps {
  copy: TrayCopy;
  order?: readonly string[];
  onRefresh(): Promise<void> | void;
  onOpenUsage(): Promise<void> | void;
  onOpenSettings(): Promise<void> | void;
  onDismiss(): Promise<void> | void;
  onQuit(): Promise<void> | void;
  autoFocusRefresh?: boolean;
}

export default function TrayActions({
  copy,
  order = ACTION_IDS,
  onRefresh,
  onOpenUsage,
  onOpenSettings,
  onDismiss,
  onQuit,
  autoFocusRefresh = false,
}: TrayActionsProps) {
  const visible = order.filter((id): id is (typeof ACTION_IDS)[number] =>
    ACTION_IDS.includes(id as (typeof ACTION_IDS)[number]),
  );
  return (
    <section className="tray-region tray-actions" role="region" aria-label={copy.actions}>
      <h2>{copy.actions}</h2>
      <div className="tray-actions__buttons tray-actions__buttons--single">
        {visible.map((id) => {
          switch (id) {
            case "refresh":
              return (
                <button key={id} type="button" autoFocus={autoFocusRefresh} onClick={() => void onRefresh()}>
                  {copy.refresh}
                </button>
              );
            case "open_usage":
              return (
                <button key={id} type="button" onClick={() => void onOpenUsage()}>
                  {copy.openUsage}
                </button>
              );
            case "settings":
              return (
                <button key={id} type="button" onClick={() => void onOpenSettings()}>
                  {copy.settings}
                </button>
              );
            case "dismiss":
              return (
                <button key={id} type="button" onClick={() => void onDismiss()}>
                  {copy.dismiss}
                </button>
              );
            case "quit":
              return (
                <button key={id} type="button" onClick={() => void onQuit()}>
                  {copy.quit}
                </button>
              );
            default:
              return null;
          }
        })}
      </div>
    </section>
  );
}
