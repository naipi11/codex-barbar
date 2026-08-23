import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCommittedRange } from "./useCommittedRange";

function RangeHarness({ value, onCommit }: { value: number; onCommit(value: number): void }) {
  const range = useCommittedRange({ value, min: 0, max: 80, onCommit });
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
});
