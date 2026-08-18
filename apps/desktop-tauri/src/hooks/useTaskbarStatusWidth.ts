import { useEffect } from "react";
import type { RefObject } from "react";
import { setTaskbarStatusWidth } from "../lib/tauri";

function borderBoxWidth(entry: ResizeObserverEntry): number | null {
  const borderBoxSize = entry.borderBoxSize;
  const box = Array.isArray(borderBoxSize) ? borderBoxSize[0] : borderBoxSize;
  return box && Number.isFinite(box.inlineSize) && box.inlineSize > 0
    ? box.inlineSize
    : null;
}

function measuredReplicaWidth(
  element: HTMLElement,
  entry?: ResizeObserverEntry,
): number | null {
  const border = entry ? borderBoxWidth(entry) : null;
  const width = Math.max(
    border ?? 0,
    element.getBoundingClientRect().width,
    element.scrollWidth,
  );
  return Number.isFinite(width) && width > 0 ? Math.round(width) : null;
}

export function useTaskbarStatusWidth(ref: RefObject<HTMLElement>) {
  useEffect(() => {
    const element = ref.current;
    if (!element || typeof ResizeObserver === "undefined") return;

    let appliedWidth: number | null = null;
    let desiredWidth: number | null = null;
    let blockedWidth: number | null = null;
    let inFlight = false;
    let mounted = true;

    const submitLatest = () => {
      if (
        !mounted ||
        inFlight ||
        desiredWidth === null ||
        desiredWidth === appliedWidth ||
        desiredWidth === blockedWidth
      ) {
        return;
      }
      const submittedWidth = desiredWidth;
      inFlight = true;
      void setTaskbarStatusWidth(submittedWidth)
        .then(() => {
          if (!mounted) return;
          appliedWidth = submittedWidth;
          inFlight = false;
          submitLatest();
        })
        .catch(() => {
          if (!mounted) return;
          inFlight = false;
          blockedWidth = submittedWidth;
          if (desiredWidth !== submittedWidth) {
            submitLatest();
          }
        });
    };

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const width = measuredReplicaWidth(element, entry);
      if (width === null) return;
      desiredWidth = width;
      // A future observation is the explicit retry signal after a failed
      // command; resolving one request can still drain a newer queued width.
      blockedWidth = null;
      submitLatest();
    });
    observer.observe(element);
    const initialWidth = measuredReplicaWidth(element);
    if (initialWidth !== null) {
      desiredWidth = initialWidth;
      submitLatest();
    }
    return () => {
      mounted = false;
      desiredWidth = null;
      observer.disconnect();
    };
  }, [ref]);
}
