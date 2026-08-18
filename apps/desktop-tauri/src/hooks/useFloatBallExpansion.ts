import { useCallback, useEffect, useRef, useState } from "react";

const EXPAND_DELAY_MS = 180;
const COLLAPSE_DELAY_MS = 120;

export function useFloatBallExpansion({
  onExpandedChange,
}: {
  onExpandedChange(expanded: boolean): Promise<void>;
}) {
  const [expanded, setExpanded] = useState(false);
  const [expansionError, setExpansionError] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancelTimer = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  }, []);

  const request = useCallback(
    async (next: boolean): Promise<boolean> => {
      try {
        await onExpandedChange(next);
        setExpanded(next);
        setExpansionError(null);
        return true;
      } catch {
        setExpansionError("悬浮球尺寸切换失败");
        return false;
      }
    },
    [onExpandedChange],
  );

  const pointerEntered = useCallback(() => {
    cancelTimer();
    timer.current = setTimeout(() => {
      timer.current = null;
      void request(true);
    }, EXPAND_DELAY_MS);
  }, [cancelTimer, request]);

  const pointerLeft = useCallback(() => {
    cancelTimer();
    timer.current = setTimeout(() => {
      timer.current = null;
      void request(false);
    }, COLLAPSE_DELAY_MS);
  }, [cancelTimer, request]);

  const collapseNow = useCallback(() => {
    cancelTimer();
    return request(false);
  }, [cancelTimer, request]);

  useEffect(() => cancelTimer, [cancelTimer]);

  return {
    expanded,
    expansionError,
    pointerEntered,
    pointerLeft,
    collapseNow,
    cancelPending: cancelTimer,
  };
}
