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
  onCommit(value: number): void | Promise<unknown>;
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

function deferred<T = void>() {
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
    const onCommit = vi.fn();
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
    const onCommit = vi.fn();
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

  it("commits keyboard edits on key release and unbound input edits on blur", () => {
    const animationFrames = installAnimationFrameHarness();
    const onCommit = vi.fn();
    const view = render(<RangeHarness value={20} onCommit={onCommit} />);
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.keyDown(range, { key: "ArrowRight" });
    fireEvent.input(range, { target: { value: "21" } });
    animationFrames.flush();
    fireEvent.keyUp(range, { key: "ArrowRight" });
    fireEvent.blur(range);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenLastCalledWith(21);

    view.rerender(<RangeHarness value={21} onCommit={onCommit} />);
    fireEvent.input(range, { target: { value: "35" } });
    fireEvent.blur(range);
    expect(onCommit).toHaveBeenCalledTimes(2);
    expect(onCommit).toHaveBeenLastCalledWith(35);
  });

  it("uses the last committed draft as the baseline before settings echo it back", () => {
    const animationFrames = installAnimationFrameHarness();
    const onCommit = vi.fn();
    render(<RangeHarness value={20} onCommit={onCommit} />);
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 9 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 9 });
    expect(onCommit).toHaveBeenLastCalledWith(30);

    fireEvent.pointerDown(range, { pointerId: 10 });
    fireEvent.input(range, { target: { value: "20" } });
    animationFrames.flush();
    fireEvent.pointerUp(range, { pointerId: 10 });

    expect(onCommit).toHaveBeenCalledTimes(2);
    expect(onCommit).toHaveBeenLastCalledWith(20);
  });

  it("rolls the latest rejected commit back, reports a sanitized error, and consumes rejection", async () => {
    const animationFrames = installAnimationFrameHarness();
    const commit = deferred();
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

  it("keeps the newest interaction when commit promises settle out of order", async () => {
    const animationFrames = installAnimationFrameHarness();
    const first = deferred();
    const second = deferred();
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
    expect(onCommit).toHaveBeenNthCalledWith(2, 40);

    await act(async () => {
      second.resolve();
      await Promise.resolve();
    });
    view.rerender(
      <RangeHarness
        value={30}
        onCommit={onCommit}
        onError={onError}
        onSuccess={onSuccess}
      />,
    );
    expect(range).toHaveValue("40");

    await act(async () => {
      first.reject(new Error("older failure"));
      await Promise.resolve();
    });
    expect(range).toHaveValue("40");
    expect(onError).not.toHaveBeenCalled();
    expect(onSuccess).toHaveBeenCalledTimes(1);
  });

  it("protects a released pending draft from stale external events until its echo confirms", async () => {
    const animationFrames = installAnimationFrameHarness();
    const commit = deferred();
    const view = render(
      <RangeHarness value={20} onCommit={() => commit.promise} />,
    );
    const range = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(range, { pointerId: 14 });
    fireEvent.input(range, { target: { value: "30" } });
    animationFrames.flush();
    fireEvent.pointerUp(range, { pointerId: 14 });
    view.rerender(
      <RangeHarness value={25} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("30");

    await act(async () => {
      commit.resolve();
      await Promise.resolve();
    });
    view.rerender(
      <RangeHarness value={30} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("30");

    view.rerender(
      <RangeHarness value={35} onCommit={() => commit.promise} />,
    );
    expect(range).toHaveValue("35");
  });
});
