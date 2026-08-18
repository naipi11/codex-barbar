import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
// @ts-ignore Vitest executes tests in Node; the browser build does not include test modules.
import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import {
  bootstrapWithTwoProfiles,
  staleOfflineFixture,
  weeklyOnlyUsage,
} from "../test/profileUsageFixtures";
type EventCallback = (event: { payload: unknown }) => void;

const eventHarness = vi.hoisted(() => {
  const listeners = new Map<string, Set<EventCallback>>();
  return {
    listeners,
    listen(eventName: string, callback: EventCallback) {
      const callbacks = listeners.get(eventName) ?? new Set<EventCallback>();
      callbacks.add(callback);
      listeners.set(eventName, callbacks);
      return Promise.resolve(() => callbacks.delete(callback));
    },
    emit(eventName: string, payload: unknown) {
      for (const callback of listeners.get(eventName) ?? []) {
        callback({ payload });
      }
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, callback: EventCallback) =>
    eventHarness.listen(eventName, callback),
}));

const floatBallCss = readFileSync("src/surfaces/FloatBall.css", "utf8");

const windowHarness = vi.hoisted(() => ({
  startDragging: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowHarness,
}));

import FloatBall from "./FloatBall";

describe("FloatBall", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    windowHarness.startDragging.mockReset();
    windowHarness.startDragging.mockResolvedValue(undefined);
    eventHarness.listeners.clear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("derives every float-ball surface-fill alpha directly from the opacity variable", () => {
    const ruleFor = (selector: string) => {
      const start = floatBallCss.indexOf(selector);
      expect(start).toBeGreaterThanOrEqual(0);
      return floatBallCss.slice(start, floatBallCss.indexOf("}", start));
    };
    const collapsed = ruleFor(".float-ball--collapsed .float-ball__body");
    const expanded = ruleFor(".float-ball--expanded .float-ball__body");

    for (const [rule, gradient, firstStop, secondStop, hasChrome] of [
      [collapsed, "radial-gradient", "rgb(70 76 102 / var(--surface-fill-alpha))", "rgb(17 19 27 / var(--surface-fill-alpha))", false],
      [expanded, "linear-gradient", "rgb(45 49 65 / var(--surface-fill-alpha))", "rgb(20 22 30 / var(--surface-fill-alpha))", true],
    ] as const) {
      const alphaChannel = (name: string) =>
        new RegExp(`--${name}:\\s*calc\\(var\\(--surface-bg-alpha\\)\\s*\\*\\s*(?:0|(?:0?\\.)?\\d+)\\)`);
      for (const name of ["surface-fill-alpha", "surface-border-alpha", "surface-shadow-alpha", "surface-inset-alpha"]) {
        expect(rule).toMatch(alphaChannel(name));
      }
      expect(rule).toContain(`background: ${gradient}`);
      expect(rule).toContain(firstStop);
      expect(rule).toContain(secondStop);
      if (hasChrome) {
        expect(rule).toContain("rgb(0 0 0 / var(--surface-shadow-alpha))");
        expect(rule).toContain("rgb(255 255 255 / var(--surface-border-alpha))");
        expect(rule).toContain("rgb(255 255 255 / var(--surface-inset-alpha))");
      }
      expect(rule).not.toMatch(/(?:calc\([^)]*(?:var\(--surface-bg-alpha\)\s*\+|\+\s*var\(--surface-bg-alpha\)))|(?:min|max|clamp)\([^)]*var\(--surface-bg-alpha\)/);
      expect(rule).not.toMatch(/(?:rgb|rgba)\([^)]*\/\s*(?:0?\.\d+|[1-9]\d*)\)/);
    }
  });

  it("renders a circular percent indicator with an identity title", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "Ming Zhao";
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    const ball = await screen.findByRole("button", {
      name: /打开完整面板.*Ming Zhao/i,
    }, { timeout: 5000 });
    expect(ball).toHaveAttribute("data-status", "ready");
    expect(ball).toHaveAttribute("title", expect.stringContaining("Ming Zhao"));
    expect(screen.getByText("42")).toBeInTheDocument();
  });

  it("announces the used percentage in used display mode", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.displayMode = "used";
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    const ball = await screen.findByRole("button", {
      name: /打开完整面板.*58% used/,
    }, { timeout: 5000 });
    expect(ball).toHaveAttribute(
      "aria-label",
      expect.stringContaining("58% used"),
    );
    expect(ball).toHaveAttribute("title", expect.stringContaining("58% used"));
    expect(ball).not.toHaveAttribute(
      "aria-label",
      expect.stringContaining("58% remaining"),
    );
    expect(ball).not.toHaveAttribute(
      "title",
      expect.stringContaining("58% remaining"),
    );
  });

  it("uses the urgent real weekly metric in the collapsed ball without fabricating 5H", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    const ball = await screen.findByRole("button", {
      name: /打开完整面板.*98% remaining/,
    }, { timeout: 5000 });
    expect(screen.getByText("98")).toBeInTheDocument();
    expect(screen.getByText("周 剩余")).toBeInTheDocument();
    expect(screen.queryByText(/5H/)).not.toBeInTheDocument();
    expect(ball).toHaveAccessibleName(expect.stringContaining("98% remaining"));
  });

  it("does not open the panel after a pointer drag", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(ball, { pointerId: 7, clientX: 30, clientY: 30 });
    await waitFor(() => expect(ball).toHaveAttribute("data-dragging", "true"));
    fireEvent.pointerUp(ball, { pointerId: 7, clientX: 30, clientY: 30 });

    expect(invokeMock).not.toHaveBeenCalledWith("open_tray_panel");
  });

  it("keeps a pointer drag active when compatibility mouse events follow it", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.mouseDown(ball, { clientX: 10, clientY: 10 });
    fireEvent.pointerMove(ball, { pointerId: 7, clientX: 30, clientY: 30 });

    await waitFor(() => expect(ball).toHaveAttribute("data-dragging", "true"));
  });

  it("starts the native window drag after crossing the movement threshold", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(ball, { pointerId: 7, clientX: 30, clientY: 30 });

    await waitFor(() => {
      expect(windowHarness.startDragging).toHaveBeenCalledTimes(1);
    });
  });

  it("opens the panel for a click without movement", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.pointerUp(ball, { pointerId: 7, clientX: 11, clientY: 10 });

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("open_tray_panel"));
  });

  it("stays collapsed on hover without an expanded card", async () => {
    vi.useFakeTimers();
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);
    render(<FloatBall />);
    await act(async () => {
      await Promise.resolve();
    });

    const shell = screen.getByTestId("float-ball-shell");
    expect(shell).toHaveClass("float-ball--collapsed");
    fireEvent.pointerEnter(shell);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });

    expect(shell).toHaveClass("float-ball--collapsed");
    expect(screen.queryByTestId("float-ball-quotas")).not.toBeInTheDocument();
    expect(screen.getByTestId("float-ball-ring-progress")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("set_float_ball_expanded", {
      expanded: true,
    });
  });


  it.each([
    [99, "high"],
    [66, "medium"],
    [0, "low"],
  ])("renders an urgent %s remaining metric with the %s ring band", async (remaining, band) => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal.primary!.remainingPercent = remaining;
    bootstrap.usageByProfile.personal.primary!.usedPercent = 100 - remaining;
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    await waitFor(() =>
      expect(screen.getByTestId("float-ball-ring-progress")).toHaveAttribute("data-band", band),
    );
  });

  it("colors cached data by remaining percent on the ring", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-13T00:00:00Z"));
    invokeMock.mockResolvedValue(staleOfflineFixture());
    render(<FloatBall />);
    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByTestId("float-ball-ring-progress")).toHaveAttribute("data-band", "medium");
    expect(screen.getByRole("button", { name: /打开完整面板/ })).toHaveAccessibleName(
      expect.stringContaining("\u7f13\u5b58\u6570\u636e\uff0c7\u5929\u524d"),
    );
  });

  it.each([[0, "0"], [20, "0.2"], [80, "0.8"]])(
    "maps float ball opacity %s to background alpha %s without root opacity",
    async (opacity, expectedAlpha) => {
      const bootstrap = bootstrapWithTwoProfiles();
      bootstrap.settings.floatBallOpacity = opacity;
      invokeMock.mockResolvedValue(bootstrap);

      render(<FloatBall />);

      const shell = await screen.findByTestId("float-ball-shell");
      expect(shell.style.getPropertyValue("--surface-bg-alpha")).toBe(expectedAlpha);
      expect(shell.style.opacity).toBe("");
    },
  );

  it("restores close feedback after the float window is recreated", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.statusSurfaceFeedback.floatBallCloseFailed = true;
    invokeMock.mockResolvedValue(bootstrap);

    const first = render(<FloatBall />);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "关闭失败，请重试",
    );
    first.unmount();

    render(<FloatBall />);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "关闭失败，请重试",
    );
  });



});
