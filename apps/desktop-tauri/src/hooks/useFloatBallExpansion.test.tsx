import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useFloatBallExpansion } from "./useFloatBallExpansion";

describe("useFloatBallExpansion", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("expands after 180ms and cancels a pending collapse when re-entered", async () => {
    vi.useFakeTimers();
    const onExpandedChange = vi.fn().mockResolvedValue(undefined);
    const { result, unmount } = renderHook(() =>
      useFloatBallExpansion({ onExpandedChange }),
    );

    act(() => result.current.pointerEntered());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(179);
    });
    expect(onExpandedChange).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(onExpandedChange).toHaveBeenCalledWith(true);

    act(() => result.current.pointerLeft());
    act(() => result.current.pointerEntered());
    await act(async () => {
      await vi.runAllTimersAsync();
    });
    expect(onExpandedChange).not.toHaveBeenCalledWith(false);

    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("collapses after 120ms and reports native expansion failures", async () => {
    vi.useFakeTimers();
    const onExpandedChange = vi
      .fn()
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("native failure"));
    const { result } = renderHook(() =>
      useFloatBallExpansion({ onExpandedChange }),
    );

    act(() => result.current.pointerEntered());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(180);
    });
    expect(result.current.expanded).toBe(true);

    act(() => result.current.pointerLeft());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(119);
    });
    expect(onExpandedChange).toHaveBeenCalledTimes(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(result.current.expanded).toBe(true);
    expect(result.current.expansionError).toBe("悬浮球尺寸切换失败");
  });
});
