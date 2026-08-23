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
}: {
  value: number;
  min: number;
  max: number;
  onCommit(value: number): void;
}) {
  const initialValue = clampRangeValue(value, min, max);
  const [draftValue, setDraftValue] = useState(initialValue);
  const activeRef = useRef(false);
  const savedValueRef = useRef(initialValue);
  const draftValueRef = useRef(initialValue);
  const pendingValueRef = useRef<number | null>(null);
  const frameRef = useRef<number | null>(null);

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
    const nextValue = pendingValueRef.current ?? draftValueRef.current;
    pendingValueRef.current = null;
    applyDraft(nextValue);
    return nextValue;
  }, [applyDraft, cancelFrame]);

  const commit = useCallback(() => {
    if (!activeRef.current) return;
    const nextValue = flushPendingDraft();
    activeRef.current = false;
    if (nextValue !== savedValueRef.current) {
      savedValueRef.current = nextValue;
      onCommit(nextValue);
    }
  }, [flushPendingDraft, onCommit]);

  const onInput = useCallback<FormEventHandler<HTMLInputElement>>(
    (event) => {
      activeRef.current = true;
      pendingValueRef.current = clampRangeValue(
        Number.parseFloat(event.currentTarget.value),
        min,
        max,
      );
      if (frameRef.current !== null) return;
      frameRef.current = window.requestAnimationFrame(() => {
        frameRef.current = null;
        const nextValue = pendingValueRef.current;
        pendingValueRef.current = null;
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
    pendingValueRef.current = null;
    activeRef.current = false;
    applyDraft(savedValueRef.current);
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
    const nextSavedValue = clampRangeValue(value, min, max);
    savedValueRef.current = nextSavedValue;
    if (!activeRef.current) {
      cancelFrame();
      pendingValueRef.current = null;
      applyDraft(nextSavedValue);
    }
  }, [applyDraft, cancelFrame, max, min, value]);

  useEffect(() => cancelFrame, [cancelFrame]);

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
