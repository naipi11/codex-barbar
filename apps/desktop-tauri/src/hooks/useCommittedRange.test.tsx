import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCommittedRange } from "./useCommittedRange";

function RangeHarness({
  value,
  onCommit,
  onError,
  onSuccess,
}: {
  value: number;
  onCommit(value: number): Promise<number>;
  onError?(): void;
  onSuccess?(): void;
}) {
  const range = useCommittedRange({
    value,
    min: 0,
    max: 80,
    onCommit,
    onError,
    onSuccess,
  });
  return (
    <input
      aria-label="Transparency"
      type="range"
      min="0"
      max="80"
      value={range.value}
      onChange={() => undefined}
      onInput={range.onInput}
      onPointerDown={range.onPointerDown}
      onPointerUp={range.onPointerUp}
      onPointerCancel={range.onPointerCancel}
      onKeyDown={range.onKeyDown}
      onKeyUp={range.onKeyUp}
      onBlur={range.onBlur}
    />
  );
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function installAnimationFrameHarness() {
  let nextId = 1;
  const frames = new Map<number, FrameRequestCallback>();
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
    const id = nextId++;
    frames.set(id, callback);
    return id;
  });
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation((id) => {
    frames.delete(id);
  });
  return {
    flush() {
      const callbacks = [...frames.values()];
      frames.clear();
      act(() => callbacks.forEach((callback) => callback(0)));
    },
  };
}

describe("useCommittedRange", () => {
  afterEach(() => vi.restoreAllMocks());

  it("coalesces input frames, protects an active draft, and commits the final value once", () => {
    const animationFrames = installAnimationFrameHarness();
    const onCommit = vi.fn().mockResolvedValue(30);
    const view = render(<RangeHarness value={20} onCommit={onCommit} />);
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 7 });
    for (let value = 21; value <= 30; value += 1) {
      fireEvent.input(range, { target: { value: String(value) } });
    }
    expect(onCommit).not.toHaveBeenCalled();
    animationFrames.flush();
    expect(range).toHaveValue("30");

    view.rerender(<RangeHarness value={25} onCommit={onCommit} />);
    expect(range).toHaveValue("30");
    fireEvent.pointerUp(range, { pointerId: 7 });
    fireEvent.blur(range);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith(30);
  });

  it("cancels a pointer draft back to the latest saved value without committing", () => {
    const animationFrames = installAnimationFrameHarness();
    const onCommit = vi.fn().mockResolvedValue(20);
    const view = render(<RangeHarness value={20} onCommit={onCommit} />);
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 8 });
    fireEvent.input(range, { target: { value: "50" } });
    animationFrames.flush();
    view.rerender(<RangeHarness value={25} onCommit={onCommit} />);
    fireEvent.pointerCancel(range, { pointerId: 8 });

    expect(range).toHaveValue("25");
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("commits keyboard edits on key release and unbound input edits on blur", async () => {
    const animationFrames = installAnimationFrameHarness();
    const onCommit = vi.fn((nextValue: number) => Promise.resolve(nextValue));
    const view = render(<RangeHarness value={20} onCommit={onCommit} />);
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.keyDown(range, { key: "ArrowRight" });
    fireEvent.input(range, { target: { value: "21" } });
    animationFrames.flush();
    fireEvent.keyUp(range, { key: "ArrowRight" });
    fireEvent.blur(range);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenLastCalledWith(21);
    await act(async () => {
      await Promise.resolve();
    });

    view.rerender(<RangeHarness value={21} onCommit={onCommit} />);
    fireEvent.input(range, { target: { value: "35" } });
    fireEvent.blur(range);
    expect(onCommit).toHaveBeenCalledTimes(2);
    expect(onCommit).toHaveBeenLastCalledWith(35);
  });

  it("does not treat a matching prop echo as acknowledgement before the Promise settles", async () => {
    const animationFrames = installAnimationFrameHarness();
    const commit = deferred<number>();
    const onError = vi.fn();
    const onSuccess = vi.fn();
    const view = render(
      <RangeHarness
        value={20}
        onCommit={() => commit.promise}
        onError={onError}
        onSuccess={onSuccess}
      />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 9 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 9 });
    animationFrames.flush();
    view.rerender(
      <RangeHarness
        value={30}
        onCommit={() => commit.promise}
        onError={onError}
        onSuccess={onSuccess}
      />,
    );

    expect(range).toHaveValue("30");
    expect(onSuccess).not.toHaveBeenCalled();

    await act(async () => {
      commit.reject(new Error("save failed after echo"));
      await Promise.resolve();
    });

    expect(range).toHaveValue("20");
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it("rolls the latest rejected commit back, reports a sanitized error, and consumes rejection", async () => {
    const animationFrames = installAnimationFrameHarness();
    const commit = deferred<number>();
    const onError = vi.fn();
    const unhandled = vi.fn();
    window.addEventListener("unhandledrejection", unhandled);
    render(
      <RangeHarness
        value={20}
        onCommit={() => commit.promise}
        onError={onError}
      />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 11 });
    fireEvent.input(range, { target: { value: "30" } });
    animationFrames.flush();
    fireEvent.pointerUp(range, { pointerId: 11 });
    expect(range).toHaveValue("30");

    await act(async () => {
      commit.reject(new Error("raw backend detail"));
      await Promise.resolve();
    });

    expect(range).toHaveValue("20");
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledWith();
    expect(unhandled).not.toHaveBeenCalled();
    window.removeEventListener("unhandledrejection", unhandled);
  });

  it("accepts a later external value after a rejection with no stale echo", async () => {
    const commit = deferred<number>();
    const view = render(
      <RangeHarness value={20} onCommit={() => commit.promise} />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 19 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 19 });
    await act(async () => {
      commit.reject(new Error("save failed"));
      await Promise.resolve();
    });
    expect(range).toHaveValue("20");

    view.rerender(
      <RangeHarness value={25} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("25");
  });

  it("serializes two deferred boundary commits and keeps the queued draft visible", async () => {
    const animationFrames = installAnimationFrameHarness();
    const first = deferred<number>();
    const second = deferred<number>();
    const onError = vi.fn();
    const onSuccess = vi.fn();
    const onCommit = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const view = render(
      <RangeHarness
        value={20}
        onCommit={onCommit}
        onError={onError}
        onSuccess={onSuccess}
      />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 12 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 12 });
    fireEvent.pointerDown(range, { pointerId: 13 });
    fireEvent.input(range, { target: { value: "40" } });
    animationFrames.flush();
    fireEvent.pointerUp(range, { pointerId: 13 });
    expect(onCommit).toHaveBeenNthCalledWith(1, 30);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(range).toHaveValue("40");

    await act(async () => {
      first.resolve(30);
      await Promise.resolve();
    });
    expect(onCommit).toHaveBeenNthCalledWith(2, 40);
    expect(range).toHaveValue("40");

    await act(async () => {
      second.resolve(40);
      await Promise.resolve();
    });
    expect(range).toHaveValue("40");
    expect(onError).not.toHaveBeenCalled();
    expect(onSuccess).toHaveBeenCalledTimes(2);
  });

  it("handles ABA commits and rolls the newest rejection back to the last Promise acknowledgement", async () => {
    const animationFrames = installAnimationFrameHarness();
    const first = deferred<number>();
    const second = deferred<number>();
    const onError = vi.fn();
    const onCommit = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const view = render(
      <RangeHarness value={30} onCommit={onCommit} onError={onError} />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 14 });
    fireEvent.input(range, { target: { value: "40" } });
    fireEvent.pointerUp(range, { pointerId: 14 });
    fireEvent.pointerDown(range, { pointerId: 15 });
    fireEvent.input(range, { target: { value: "30" } });
    animationFrames.flush();
    fireEvent.pointerUp(range, { pointerId: 15 });
    expect(onCommit).toHaveBeenCalledTimes(1);

    await act(async () => {
      first.resolve(40);
      await Promise.resolve();
    });
    expect(onCommit).toHaveBeenNthCalledWith(2, 30);

    view.rerender(
      <RangeHarness value={40} onCommit={onCommit} onError={onError} />,
    );
    expect(range).toHaveValue("30");

    await act(async () => {
      second.reject(new Error("newest failed"));
      await Promise.resolve();
    });

    expect(range).toHaveValue("40");
    expect(onError).toHaveBeenCalledTimes(1);
  });

  it("suppresses an older rejection while preserving and starting a newer queued commit", async () => {
    const first = deferred<number>();
    const second = deferred<number>();
    const onError = vi.fn();
    const onCommit = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    render(
      <RangeHarness value={20} onCommit={onCommit} onError={onError} />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 16 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 16 });
    fireEvent.pointerDown(range, { pointerId: 17 });
    fireEvent.input(range, { target: { value: "40" } });
    fireEvent.pointerUp(range, { pointerId: 17 });
    expect(onCommit).toHaveBeenCalledTimes(1);

    await act(async () => {
      first.reject(new Error("older failed"));
      await Promise.resolve();
    });

    expect(onCommit).toHaveBeenNthCalledWith(2, 40);
    expect(range).toHaveValue("40");
    expect(onError).not.toHaveBeenCalled();

    await act(async () => {
      second.resolve(40);
      await Promise.resolve();
    });
    expect(range).toHaveValue("40");
  });

  it("ignores stale prop events until the latest Promise acknowledgement is reflected", async () => {
    const commit = deferred<number>();
    const view = render(
      <RangeHarness value={20} onCommit={() => commit.promise} />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 18 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 18 });
    view.rerender(
      <RangeHarness value={25} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("30");

    await act(async () => {
      commit.resolve(30);
      await Promise.resolve();
    });
    view.rerender(
      <RangeHarness value={26} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("30");

    view.rerender(
      <RangeHarness value={30} onCommit={() => commit.promise} />,
    );
    view.rerender(
      <RangeHarness value={35} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("35");
  });
});
