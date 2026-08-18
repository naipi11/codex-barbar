import { render } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import { useTaskbarStatusWidth } from "./useTaskbarStatusWidth";

class ResizeObserverStub {
  static instances: ResizeObserverStub[] = [];
  readonly disconnect = vi.fn();
  private readonly callback: ResizeObserverCallback;

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    ResizeObserverStub.instances.push(this);
  }

  observe = vi.fn();
  unobserve = vi.fn();

  report(width: number) {
    this.callback(
      [{ borderBoxSize: [{ inlineSize: width }] } as unknown as ResizeObserverEntry],
      this as unknown as ResizeObserver,
    );
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function ReplicaSubject() {
  const replicaRef = useRef<HTMLDivElement>(null);
  useTaskbarStatusWidth(replicaRef);
  return (
    <>
      <div data-testid="visible" />
      <div ref={replicaRef} data-testid="replica" />
    </>
  );
}

describe("useTaskbarStatusWidth", () => {
  let replicaWidth = 247.4;

  beforeEach(() => {
    replicaWidth = 247.4;
    ResizeObserverStub.instances = [];
    vi.stubGlobal("ResizeObserver", ResizeObserverStub);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      return { width: this.dataset.testid === "replica" ? replicaWidth : 168 } as DOMRect;
    });
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (this: HTMLElement) {
      return this.dataset.testid === "replica" ? replicaWidth : 168;
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("measures only the unconstrained replica before the first observer callback", async () => {
    invokeMock.mockResolvedValue(undefined);
    const view = render(<ReplicaSubject />);

    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith("set_taskbar_status_width", { width: 247 }),
    );
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(ResizeObserverStub.instances[0]!.observe).toHaveBeenCalledWith(
      view.getByTestId("replica"),
    );
  });

  it("submits observer growth from 247 to 281", async () => {
    invokeMock.mockResolvedValue(undefined);
    render(<ReplicaSubject />);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    replicaWidth = 281;
    ResizeObserverStub.instances[0]!.report(281);

    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith("set_taskbar_status_width", { width: 281 }),
    );
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("submits a genuine replica shrink from 281 to 226", async () => {
    replicaWidth = 281;
    invokeMock.mockResolvedValue(undefined);
    render(<ReplicaSubject />);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    replicaWidth = 226;
    ResizeObserverStub.instances[0]!.report(226);

    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith("set_taskbar_status_width", { width: 226 }),
    );
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("deduplicates a repeated 226 observation", async () => {
    replicaWidth = 226;
    invokeMock.mockResolvedValue(undefined);
    render(<ReplicaSubject />);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    ResizeObserverStub.instances[0]!.report(226);
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("retries a rejected 226 width only after a future observation", async () => {
    replicaWidth = 226;
    const first = deferred<void>();
    invokeMock.mockImplementationOnce(() => first.promise).mockResolvedValueOnce(undefined);
    render(<ReplicaSubject />);

    first.reject(new Error("resize unavailable"));
    await Promise.resolve();
    await Promise.resolve();
    expect(invokeMock).toHaveBeenCalledTimes(1);

    ResizeObserverStub.instances[0]!.report(226);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(invokeMock).toHaveBeenLastCalledWith("set_taskbar_status_width", { width: 226 });
  });

  it("serializes one request in flight and drains only the latest queued width", async () => {
    const first = deferred<void>();
    invokeMock.mockImplementationOnce(() => first.promise).mockResolvedValueOnce(undefined);
    render(<ReplicaSubject />);

    replicaWidth = 281;
    ResizeObserverStub.instances[0]!.report(281);
    replicaWidth = 226;
    ResizeObserverStub.instances[0]!.report(226);
    expect(invokeMock).toHaveBeenCalledTimes(1);

    first.resolve();
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(invokeMock).toHaveBeenLastCalledWith("set_taskbar_status_width", { width: 226 });
  });

  it("does not dispatch a queued width after unmounting with a request pending", async () => {
    const first = deferred<void>();
    invokeMock.mockImplementation(() => first.promise);
    const view = render(<ReplicaSubject />);
    replicaWidth = 281;
    ResizeObserverStub.instances[0]!.report(281);

    view.unmount();
    first.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(ResizeObserverStub.instances[0]!.disconnect).toHaveBeenCalledOnce();
  });

  it("preserves the native fallback when ResizeObserver is missing without logging", async () => {
    vi.stubGlobal("ResizeObserver", undefined);
    const logs = [
      vi.spyOn(console, "log").mockImplementation(() => {}),
      vi.spyOn(console, "warn").mockImplementation(() => {}),
      vi.spyOn(console, "error").mockImplementation(() => {}),
    ];

    render(<ReplicaSubject />);
    await Promise.resolve();

    expect(invokeMock).not.toHaveBeenCalled();
    logs.forEach((log) => expect(log).not.toHaveBeenCalled());
  });

  it.each([0, Number.NaN, Number.POSITIVE_INFINITY])(
    "ignores invalid replica measurement %s without logging",
    async (width) => {
      replicaWidth = width;
      const logs = [
        vi.spyOn(console, "log").mockImplementation(() => {}),
        vi.spyOn(console, "warn").mockImplementation(() => {}),
        vi.spyOn(console, "error").mockImplementation(() => {}),
      ];

      render(<ReplicaSubject />);
      ResizeObserverStub.instances[0]!.report(width);
      await Promise.resolve();

      expect(invokeMock).not.toHaveBeenCalled();
      logs.forEach((log) => expect(log).not.toHaveBeenCalled());
    },
  );
});
