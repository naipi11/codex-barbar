import { useCallback, useEffect, useRef, useState } from "react";
import type {
  FocusEventHandler,
  FormEventHandler,
  KeyboardEventHandler,
  PointerEventHandler,
} from "react";

const RANGE_KEYS = new Set([
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "End",
  "Home",
  "PageDown",
  "PageUp",
]);

function clampRangeValue(value: number, min: number, max: number): number {
  const finiteValue = Number.isFinite(value) ? value : min;
  return Math.max(min, Math.min(max, finiteValue));
}

export function useCommittedRange({
  value,
  min,
  max,
  onCommit,
  onError,
  onSuccess,
}: {
  value: number;
  min: number;
  max: number;
  onCommit(value: number): Promise<number>;
  onError?(): void;
  onSuccess?(): void;
}) {
  const initialValue = clampRangeValue(value, min, max);
  const [draftValue, setDraftValue] = useState(initialValue);
  const activeRef = useRef(false);
  const mountedRef = useRef(true);
  const confirmedValueRef = useRef(initialValue);
  const draftValueRef = useRef(initialValue);
  const latestPropValueRef = useRef(initialValue);
  const pendingFrameValueRef = useRef<number | null>(null);
  const inFlightCommitRef = useRef<{ generation: number; value: number } | null>(null);
  const commitQueueRef = useRef<Array<{ generation: number; value: number }>>([]);
  const commitGenerationRef = useRef(0);
  const propBarrierRef = useRef<number | null>(null);
  const pumpCommitQueueRef = useRef<() => void>(() => undefined);
  const frameRef = useRef<number | null>(null);
  const onCommitRef = useRef(onCommit);
  const onErrorRef = useRef(onError);
  const onSuccessRef = useRef(onSuccess);

  onCommitRef.current = onCommit;
  onErrorRef.current = onError;
  onSuccessRef.current = onSuccess;

  const cancelFrame = useCallback(() => {
    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
  }, []);

  const applyDraft = useCallback((nextValue: number) => {
    draftValueRef.current = nextValue;
    setDraftValue(nextValue);
  }, []);

  const flushPendingDraft = useCallback(() => {
    cancelFrame();
    const nextValue = pendingFrameValueRef.current ?? draftValueRef.current;
    pendingFrameValueRef.current = null;
    applyDraft(nextValue);
    return nextValue;
  }, [applyDraft, cancelFrame]);

  const pumpCommitQueue = useCallback(() => {
    if (inFlightCommitRef.current !== null) return;
    const request = commitQueueRef.current.shift();
    if (request === undefined) return;
    inFlightCommitRef.current = request;

    let acknowledgement: Promise<number>;
    try {
      acknowledgement = onCommitRef.current(request.value);
    } catch {
      acknowledgement = Promise.reject();
    }
    void Promise.resolve(acknowledgement).then(
      (savedValue) => {
        if (
          !mountedRef.current ||
          inFlightCommitRef.current?.generation !== request.generation
        ) {
          return;
        }

        const nextConfirmedValue = clampRangeValue(savedValue, min, max);
        confirmedValueRef.current = nextConfirmedValue;
        inFlightCommitRef.current = null;
        propBarrierRef.current =
          latestPropValueRef.current === nextConfirmedValue
            ? null
            : nextConfirmedValue;
        const hasNewerCommit = commitQueueRef.current.length > 0;
        if (!activeRef.current && !hasNewerCommit) {
          cancelFrame();
          pendingFrameValueRef.current = null;
          applyDraft(nextConfirmedValue);
        }
        onSuccessRef.current?.();
        pumpCommitQueueRef.current();
      },
      () => {
        if (
          !mountedRef.current ||
          inFlightCommitRef.current?.generation !== request.generation
        ) {
          return;
        }

        inFlightCommitRef.current = null;
        if (commitQueueRef.current.length > 0) {
          pumpCommitQueueRef.current();
          return;
        }

        propBarrierRef.current =
          latestPropValueRef.current === confirmedValueRef.current
            ? null
            : confirmedValueRef.current;
        if (!activeRef.current) {
          cancelFrame();
          pendingFrameValueRef.current = null;
          applyDraft(confirmedValueRef.current);
        }
        onErrorRef.current?.();
      },
    );
  }, [applyDraft, cancelFrame, max, min]);
  pumpCommitQueueRef.current = pumpCommitQueue;

  const commit = useCallback(() => {
    if (!activeRef.current) return;
    const nextValue = flushPendingDraft();
    activeRef.current = false;
    const queuedCommits = commitQueueRef.current;
    const baseline =
      queuedCommits[queuedCommits.length - 1]?.value ??
      inFlightCommitRef.current?.value ??
      confirmedValueRef.current;
    if (nextValue === baseline) return;

    const generation = commitGenerationRef.current + 1;
    commitGenerationRef.current = generation;
    commitQueueRef.current.push({ generation, value: nextValue });
    pumpCommitQueueRef.current();
  }, [flushPendingDraft]);

  const onInput = useCallback<FormEventHandler<HTMLInputElement>>(
    (event) => {
      activeRef.current = true;
      pendingFrameValueRef.current = clampRangeValue(
        Number.parseFloat(event.currentTarget.value),
        min,
        max,
      );
      if (frameRef.current !== null) return;
      frameRef.current = window.requestAnimationFrame(() => {
        frameRef.current = null;
        const nextValue = pendingFrameValueRef.current;
        pendingFrameValueRef.current = null;
        if (nextValue !== null) applyDraft(nextValue);
      });
    },
    [applyDraft, max, min],
  );

  const onPointerDown = useCallback<PointerEventHandler<HTMLInputElement>>(() => {
    activeRef.current = true;
  }, []);

  const onPointerUp = useCallback<PointerEventHandler<HTMLInputElement>>(() => {
    commit();
  }, [commit]);

  const onPointerCancel = useCallback<PointerEventHandler<HTMLInputElement>>(() => {
    cancelFrame();
    pendingFrameValueRef.current = null;
    activeRef.current = false;
    const queuedCommits = commitQueueRef.current;
    applyDraft(
      queuedCommits[queuedCommits.length - 1]?.value ??
        inFlightCommitRef.current?.value ??
        confirmedValueRef.current,
    );
  }, [applyDraft, cancelFrame]);

  const onKeyDown = useCallback<KeyboardEventHandler<HTMLInputElement>>((event) => {
    if (RANGE_KEYS.has(event.key)) activeRef.current = true;
  }, []);

  const onKeyUp = useCallback<KeyboardEventHandler<HTMLInputElement>>(
    (event) => {
      if (RANGE_KEYS.has(event.key)) commit();
    },
    [commit],
  );

  const onBlur = useCallback<FocusEventHandler<HTMLInputElement>>(() => {
    commit();
  }, [commit]);

  useEffect(() => {
    const nextConfirmedValue = clampRangeValue(value, min, max);
    latestPropValueRef.current = nextConfirmedValue;
    if (
      inFlightCommitRef.current !== null ||
      commitQueueRef.current.length > 0
    ) {
      return;
    }

    const propBarrier = propBarrierRef.current;
    if (propBarrier !== null) {
      if (nextConfirmedValue !== propBarrier) return;
      propBarrierRef.current = null;
    }

    confirmedValueRef.current = nextConfirmedValue;
    if (activeRef.current) return;
    cancelFrame();
    pendingFrameValueRef.current = null;
    applyDraft(nextConfirmedValue);
  }, [applyDraft, cancelFrame, max, min, value]);

  useEffect(
    () => {
      mountedRef.current = true;
      return () => {
        mountedRef.current = false;
        cancelFrame();
      };
    },
    [cancelFrame],
  );

  return {
    value: draftValue,
    onInput,
    onPointerDown,
    onPointerUp,
    onPointerCancel,
    onKeyDown,
    onKeyUp,
    onBlur,
  };
}
