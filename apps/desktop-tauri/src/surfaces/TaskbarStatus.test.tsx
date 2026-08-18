import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import {
  bootstrapWithTwoProfiles,
  readyTwoWindowFixture,
  staleOfflineFixture,
  weeklyOnlyUsage,
} from "../test/profileUsageFixtures";
import { events } from "../lib/tauri";
import TaskbarStatus from "./TaskbarStatus";

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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("TaskbarStatus", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    eventHarness.listeners.clear();
    window.history.replaceState({}, "", "/");
  });

  it("renders visible weekly content without an in-page measurement replica", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "ProofUser@example.com";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    let resolveBootstrap!: (value: typeof bootstrap) => void;
    const bootstrapPromise = new Promise<typeof bootstrap>((resolve) => {
      resolveBootstrap = resolve;
    });
    invokeMock.mockResolvedValue(bootstrapPromise);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    resolveBootstrap(bootstrap);
    const main = await within(visible).findByRole("button", { name: /ProofUser@example\.com/ });

    expect(within(visible).getByText("ProofU")).toBeVisible();
    expect(within(visible).getByText("周 98%")).toBeVisible();
    expect(within(visible).getByText("8/20")).toBeVisible();
    expect(within(visible).getByRole("button", { name: "关闭任务栏状态" })).toBeVisible();
    expect(within(visible).queryByText(/5H/)).not.toBeInTheDocument();
    expect(main).toHaveAttribute("title", "ProofUser@example.com");
    expect(within(visible).queryByText("ProofUser@example.com")).not.toBeInTheDocument();

    expect(screen.queryByTestId("taskbar-status-measurement")).toBeNull();
    expect(screen.getAllByRole("button")).toHaveLength(2);
    expect(
      invokeMock.mock.calls.some(([command]) => command === "set_taskbar_status_width"),
    ).toBe(false);
  });

  it("ignores the retired external-measurement query on the visible route", async () => {
    window.history.replaceState({}, "", "/?measurement=external");
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "ProofUser@example.com";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);

    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    expect(await within(visible).findByText("周 98%")).toBeVisible();
    expect(within(visible).getByText("ProofU")).toBeVisible();
    expect(within(visible).getByText("8/20")).toBeVisible();
    expect(screen.queryByTestId("taskbar-status-measurement")).toBeNull();
  });

  it("renders every real metric in backend order without duplicating an urgent metric", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.usageByProfile.personal!.additionalWindows = [{
      limitId: "spark", label: "Spark quota", usedPercent: 12, remainingPercent: 88,
      windowDurationMinutes: 1_440, resetsAt: "2026-08-20T00:00:00Z", reachedType: null,
    }];
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    expect(await within(visible).findByText("5H 42%")).toBeInTheDocument();
    expect(within(visible).getByText("周 61%")).toBeInTheDocument();
    expect(within(visible).getByText("Spark 88%")).toBeInTheDocument();
    expect(within(visible).getAllByText("周 61%")).toHaveLength(1);
  });

  it("contains long real metrics in the only overflow track while reserving reset and final close", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.usageByProfile.personal!.additionalWindows = Array.from(
      { length: 6 },
      (_, index) => ({
        limitId: `long-${index}`,
        label: `Legitimate long quota ${index + 1}`,
        usedPercent: index + 10,
        remainingPercent: 90 - index,
        windowDurationMinutes: 1_440 + index,
        resetsAt: `2026-08-${20 + index}T00:00:00Z`,
        reachedType: null,
      }),
    );
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    await within(visible).findByText("Legiti 90%");
    const track = within(visible).getByTestId("taskbar-status-quota-track");
    const reset = within(visible).getByTestId("taskbar-status-reset");
    const close = within(visible).getByRole("button", { name: /关闭任务栏状态/ });
    expect(track.querySelectorAll('[data-testid="taskbar-status-metric"]')).toHaveLength(8);
    expect(within(visible).getByText("Legiti 90%")).toBeInTheDocument();
    expect(track).toContainElement(within(visible).getByText("Legiti 90%"));
    expect(track).not.toContainElement(reset);
    expect(visible.lastElementChild).toBe(close);
    expect(close.previousElementSibling).toBe(screen.getByRole("button", { name: /打开完整面板/ }));
  });

  it("uses stable unique metric keys when same-label real windows lack limit ids", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.usageByProfile.personal!.additionalWindows = [
      {
        limitId: "", label: "Burst", usedPercent: 20, remainingPercent: 80,
        windowDurationMinutes: 1_440, resetsAt: "2026-08-20T00:00:00Z", reachedType: null,
      },
      {
        limitId: "", label: "Burst", usedPercent: 30, remainingPercent: 70,
        windowDurationMinutes: 2_880, resetsAt: "2026-08-21T00:00:00Z", reachedType: null,
      },
    ];
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    await waitFor(() => expect(within(visible).getAllByText(/Burst (80|70)%/)).toHaveLength(2));
    expect(within(visible).getAllByText(/Burst (80|70)%/)).toHaveLength(2);
    expect(consoleError.mock.calls.flat().join(" ")).not.toContain("same key");
    consoleError.mockRestore();
  });

  it.each([[100,"high"],[67,"high"],[66,"medium"],[34,"medium"],[33,"low"],[0,"low"]] as const)(
    "renders %s remaining with the %s band", async (remaining, band) => {
      const bootstrap = bootstrapWithTwoProfiles();
      bootstrap.usageByProfile.personal!.primary!.remainingPercent = remaining;
      bootstrap.usageByProfile.personal!.primary!.usedPercent = 100 - remaining;
      invokeMock.mockResolvedValue(bootstrap);
      render(<TaskbarStatus />);
      const visible = await screen.findByTestId("taskbar-status-visible");
      expect(await within(visible).findByTestId("taskbar-status-metric")).toHaveAttribute("data-band", band);
    },
  );

  it("announces cached data and its update time while rendering colored metric bands", async () => {
    const bootstrap = staleOfflineFixture();
    bootstrap.usageByProfile.personal!.fetchedAt = new Date().toISOString();
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const main = await screen.findByRole("button", { name: /缓存.*0分钟前/ });
    const visible = screen.getByTestId("taskbar-status-visible");
    expect(within(visible).getByTestId("taskbar-status-metric")).toHaveAttribute("data-band", "medium");
    expect(main).toHaveAccessibleName(expect.stringContaining("缓存"));
    expect(main).toHaveAccessibleName(expect.stringContaining("0分钟前"));
  });

  it.each([[0, "0"], [20, "0.2"], [80, "0.8"]])(
    "attaches taskbar opacity %s to the rendered root as alpha %s",
    async (opacity, expectedAlpha) => {
      const bootstrap = readyTwoWindowFixture();
      bootstrap.settings.taskbarStatusOpacity = opacity;
      invokeMock.mockResolvedValue(bootstrap);
      render(<TaskbarStatus />);

      const visible = await screen.findByTestId("taskbar-status-visible");
      await within(visible).findByText("5H 42%");
      expect(visible.style.getPropertyValue("--surface-bg-alpha")).toBe(expectedAlpha);
      expect(visible.parentElement?.style.getPropertyValue("--surface-bg-alpha")).toBe("");
      expect(visible.style.opacity).toBe("");
    },
  );

  it("updates the rendered root alpha from settings-changed without requesting a width", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.settings.taskbarStatusOpacity = 20;
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    await within(visible).findByText("5H 42%");
    await waitFor(() =>
      expect(eventHarness.listeners.get(events.settingsChanged)?.size).toBeGreaterThan(0),
    );
    expect(visible.style.getPropertyValue("--surface-bg-alpha")).toBe("0.2");

    act(() => {
      eventHarness.emit(events.settingsChanged, {
        ...bootstrap.settings,
        taskbarStatusOpacity: 80,
      });
    });

    await waitFor(() =>
      expect(visible.style.getPropertyValue("--surface-bg-alpha")).toBe("0.8"),
    );
    expect(
      invokeMock.mock.calls.some(([command]) => command === "set_taskbar_status_width"),
    ).toBe(false);
  });

  it("opens the tray panel only from the main button", async () => {
    invokeMock.mockResolvedValue(bootstrapWithTwoProfiles());
    render(<TaskbarStatus />);
    fireEvent.click(await screen.findByRole("button", { name: /打开完整面板/ }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("open_tray_panel"));
  });

  it("keeps the close action as the final sibling with no flexible spacer", async () => {
    const bootstrap = readyTwoWindowFixture();
    invokeMock.mockImplementation(async (command: string) => command === "get_bootstrap_state" ? bootstrap : undefined);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    const close = within(visible).getByRole("button", { name: "关闭任务栏状态" });
    expect(visible.querySelector("[data-flex-spacer]")).toBeNull();
    expect(visible.lastElementChild).toBe(close);
    expect(close.previousElementSibling).toBe(screen.getByRole("button", { name: /打开完整面板/ }));
    fireEvent.click(close);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", { surface: "taskbarStatus", enabled: false }));
    expect(invokeMock).not.toHaveBeenCalledWith("open_tray_panel");
  });

  it("keeps a successful close out of the retry error state", async () => {
    const bootstrap = readyTwoWindowFixture();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_bootstrap_state") return bootstrap;
      if (command === "set_status_surface_enabled") {
        return { ...bootstrap.settings, taskbarStatusEnabled: false };
      }
      return undefined;
    });
    render(<TaskbarStatus />);

    const close = await screen.findByRole("button", { name: "关闭任务栏状态" });
    fireEvent.click(close);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", {
        surface: "taskbarStatus",
        enabled: false,
      }),
    );
    expect(close).not.toHaveAttribute("data-error");
    expect(screen.getByRole("status")).toHaveTextContent("");
  });

  it("restores shared close feedback when the taskbar window is recreated", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.statusSurfaceFeedback.taskbarStatusCloseFailed = true;
    invokeMock.mockResolvedValue(bootstrap);

    const first = render(<TaskbarStatus />);
    const visible = await screen.findByTestId("taskbar-status-visible");
    const close = within(visible).getByRole("button", {
      name: "关闭任务栏状态",
    });

    expect(close).toHaveAttribute("data-error", "true");
    expect(close).toHaveAttribute("title", "关闭失败，点击重试");
    expect(screen.getByRole("status")).toHaveTextContent("关闭失败，点击重试");
    expect(within(visible).getByText("5H 42%")).toBeInTheDocument();
    expect(within(visible).getByText("周 61%")).toBeInTheDocument();

    first.unmount();
    render(<TaskbarStatus />);

    const rebuilt = await screen.findByTestId("taskbar-status-visible");
    const rebuiltClose = within(rebuilt).getByRole("button", {
      name: "关闭任务栏状态",
    });
    expect(rebuiltClose).toHaveAttribute("data-error", "true");
    expect(screen.getByRole("status")).toHaveTextContent("关闭失败，点击重试");
  });

  it("keeps shared close failure feedback out of geometry and resets only the retry target", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.statusSurfaceFeedback = {
      taskbarStatusCloseFailed: true,
      floatBallCloseFailed: true,
    };
    const retry = deferred<void>();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_bootstrap_state") return bootstrap;
      if (command === "set_status_surface_enabled") {
        return retry.promise;
      }
      return undefined;
    });
    render(<TaskbarStatus />);

    const close = await screen.findByRole("button", { name: "关闭任务栏状态" });
    const closeStatus = screen.getByRole("status");
    expect(closeStatus).toHaveTextContent("关闭失败，点击重试");
    expect(closeStatus).toHaveAttribute("aria-live", "polite");
    const visible = screen.getByTestId("taskbar-status-visible");
    expect(within(visible).getByText("5H 42%")).toBeInTheDocument();
    expect(within(visible).getByText("周 61%")).toBeInTheDocument();
    expect(visible).not.toHaveTextContent("关闭失败，请重试");
    expect(close).toHaveAttribute("data-error", "true");
    expect(close).toHaveAttribute("title", "关闭失败，点击重试");
    expect(close).toBeEnabled();

    fireEvent.click(close);
    await waitFor(() => expect(close).not.toHaveAttribute("data-error"));
    expect(close).toHaveAttribute("title", "关闭任务栏状态");

    act(() => {
      eventHarness.emit(events.statusSurfaceFeedbackChanged, {
        surface: "floatBall",
        closeFailed: false,
      });
    });
    expect(close).not.toHaveAttribute("data-error");

    retry.reject(new Error("STATUS_SURFACE_WINDOW_CLOSE_FAILED"));
    await waitFor(() => expect(close).toHaveAttribute("data-error", "true"));
    expect(closeStatus).toHaveTextContent("关闭失败，点击重试");
  });
});
