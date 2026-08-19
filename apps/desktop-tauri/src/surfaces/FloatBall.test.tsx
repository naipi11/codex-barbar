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

  it("spins a blossom instead of showing a numeric quota", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "Ming Zhao";
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_float_ball_motion") return { thinking: false, fast: false };
      return bootstrap;
    });
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /(打开完整面板|Open panel).*Ming Zhao/i }, { timeout: 5000 });
    expect(ball).toHaveAttribute("data-status", "ready");
    expect(ball.querySelector(".float-ball__blossom")).not.toBeNull();
    expect(screen.queryByText("42")).toBeNull();
    expect(screen.queryByTestId("float-ball-ring-progress")).toBeNull();
    expect(floatBallCss).toContain("float-ball-spin");
    expect(floatBallCss).toContain("rotate(360deg)");
    expect(floatBallCss).toContain("width: 40px");
    expect(floatBallCss).toContain("height: 40px");
    expect(floatBallCss).toMatch(/\.float-ball__spin \{[\s\S]*inset: 0;/);
    expect(floatBallCss).toContain("opacity: calc(0.22 + var(--surface-bg-alpha) * 0.78)");
    expect(floatBallCss).toContain("opacity: calc(0.18 + var(--float-glow) * 0.82)");
  });

  it("renders a circular percent indicator with an identity title", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "Ming Zhao";
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    const ball = await screen.findByRole("button", {
      name: /(打开完整面板|Open panel).*Ming Zhao/i,
    }, { timeout: 5000 });
    expect(ball).toHaveAttribute("data-status", "ready");
    expect(ball).toHaveAttribute("title", expect.stringContaining("Ming Zhao"));
  });

  it("announces the used percentage in used display mode", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.displayMode = "used";
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    const ball = await screen.findByRole("button", {
      name: /(打开完整面板|Open panel).*58%/,
    }, { timeout: 5000 });
    expect(ball).toHaveAttribute("aria-label", expect.stringContaining("58%"));
  });

  it("keeps weekly quota details in the accessible name without painting digits", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);

    render(<FloatBall />);

    const ball = await screen.findByRole("button", {
      name: /(打开完整面板|Open panel).*98%/,
    }, { timeout: 5000 });
    expect(screen.queryByText("98")).toBeNull();
    expect(screen.queryByText(/5H/)).not.toBeInTheDocument();
    expect(ball).toHaveAccessibleName(expect.stringContaining("98%"));
  });

  it("does not open the panel after a pointer drag", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板|Open panel/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(ball, { pointerId: 7, clientX: 30, clientY: 30 });
    await waitFor(() => expect(ball).toHaveAttribute("data-dragging", "true"));
    fireEvent.pointerUp(ball, { pointerId: 7, clientX: 30, clientY: 30 });

    expect(invokeMock).not.toHaveBeenCalledWith("open_tray_panel");
  });

  it("keeps a pointer drag active when compatibility mouse events follow it", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板|Open panel/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.mouseDown(ball, { clientX: 10, clientY: 10 });
    fireEvent.pointerMove(ball, { pointerId: 7, clientX: 30, clientY: 30 });

    await waitFor(() => expect(ball).toHaveAttribute("data-dragging", "true"));
  });

  it("starts the native window drag after crossing the movement threshold", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板|Open panel/ });

    fireEvent.pointerDown(ball, { pointerId: 7, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(ball, { pointerId: 7, clientX: 30, clientY: 30 });

    await waitFor(() => {
      expect(windowHarness.startDragging).toHaveBeenCalledTimes(1);
    });
  });

  it("opens the panel for a click without movement", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<FloatBall />);
    const ball = await screen.findByRole("button", { name: /打开完整面板|Open panel/ });

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
    expect(screen.queryByTestId("float-ball-ring-progress")).toBeNull();
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
      expect(screen.getByRole("button", { name: /打开完整面板|Open panel/ })).toHaveAttribute("data-band", band),
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

    expect(screen.getByRole("button", { name: /打开完整面板|Open panel/ })).toHaveAttribute("data-band", "medium");
    expect(screen.getByRole("button", { name: /打开完整面板|Open panel/ })).toHaveAttribute("data-band", "medium");
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

  it("maps glow brightness onto the blossom without changing spin size", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.floatBallGlow = 80;
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_float_ball_motion") return { thinking: false, fast: true };
      return bootstrap;
    });
    render(<FloatBall />);
    const shell = await screen.findByTestId("float-ball-shell");
    expect(shell.style.getPropertyValue("--float-glow")).toBe("1");
    await waitFor(() => expect(shell).toHaveAttribute("data-motion", "fast"));
  });

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
