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
  onCommit(value: number): void | Promise<unknown>;
  onError?(): void;
  onSuccess?(): void;
}) {
  const initialValue = clampRangeValue(value, min, max);
  const [draftValue, setDraftValue] = useState(initialValue);
  const activeRef = useRef(false);
  const mountedRef = useRef(true);
  const confirmedValueRef = useRef(initialValue);
  const draftValueRef = useRef(initialValue);
  const pendingFrameValueRef = useRef<number | null>(null);
  const pendingCommitRef = useRef<{ generation: number; value: number } | null>(null);
  const commitGenerationRef = useRef(0);
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

  const commit = useCallback(() => {
    if (!activeRef.current) return;
    const nextValue = flushPendingDraft();
    activeRef.current = false;
    const baseline = pendingCommitRef.current?.value ?? confirmedValueRef.current;
    if (nextValue === baseline) return;

    const generation = commitGenerationRef.current + 1;
    commitGenerationRef.current = generation;
    pendingCommitRef.current = { generation, value: nextValue };
    let acknowledgement: void | Promise<unknown>;
    try {
      acknowledgement = onCommitRef.current(nextValue);
    } catch {
      acknowledgement = Promise.reject();
    }
    void Promise.resolve(acknowledgement).then(
      () => {
        if (
          mountedRef.current &&
          pendingCommitRef.current?.generation === generation
        ) {
          onSuccessRef.current?.();
        }
      },
      () => {
        if (
          !mountedRef.current ||
          pendingCommitRef.current?.generation !== generation
        ) {
          return;
        }
        pendingCommitRef.current = null;
        if (!activeRef.current) {
          cancelFrame();
          pendingFrameValueRef.current = null;
          applyDraft(confirmedValueRef.current);
        }
        onErrorRef.current?.();
      },
    );
  }, [applyDraft, cancelFrame, flushPendingDraft]);

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
    applyDraft(pendingCommitRef.current?.value ?? confirmedValueRef.current);
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
    const pendingCommit = pendingCommitRef.current;
    if (pendingCommit !== null) {
      if (nextConfirmedValue !== pendingCommit.value) return;
      confirmedValueRef.current = nextConfirmedValue;
      pendingCommitRef.current = null;
      if (!activeRef.current) applyDraft(nextConfirmedValue);
      return;
    }

    confirmedValueRef.current = nextConfirmedValue;
    if (!activeRef.current) {
      cancelFrame();
      pendingFrameValueRef.current = null;
      applyDraft(nextConfirmedValue);
    }
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
