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
    expect(within(visible).getByText(/周 98%|Wk 98%/)).toBeVisible();
    expect(within(visible).getByText("8/20")).toBeVisible();
    expect(within(visible).queryByText(/5H/)).not.toBeInTheDocument();
    expect(main).toHaveAttribute("title", "ProofUser@example.com");
    expect(within(visible).queryByText("ProofUser@example.com")).not.toBeInTheDocument();

    expect(screen.queryByTestId("taskbar-status-measurement")).toBeNull();
    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(
      invokeMock.mock.calls.some(([command]) => command === "set_taskbar_status_width"),
    ).toBe(false);
  });

  it("shows only the universal weekly quota when model-specific limits are present", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = weeklyOnlyUsage({
      limitId: "codex:primary",
      remainingPercent: 99,
      usedPercent: 1,
      resetsAt: "2026-08-27T05:13:00Z",
    });
    bootstrap.usageByProfile.personal.additionalWindows = [
      {
        limitId: "codex-spark:primary",
        label: "GPT-5.3-Codex-Spark",
        usedPercent: 0,
        remainingPercent: 100,
        windowDurationMinutes: 300,
        resetsAt: "2026-08-20T10:46:00Z",
        reachedType: null,
      },
      {
        limitId: "codex-spark:secondary",
        label: "GPT-5.3-Codex-Spark",
        usedPercent: 0,
        remainingPercent: 100,
        windowDurationMinutes: 10_080,
        resetsAt: "2026-08-27T05:46:00Z",
        reachedType: null,
      },
    ];
    invokeMock.mockResolvedValue(bootstrap);

    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    expect(await within(visible).findByText(/周 99%|Wk 99%/)).toBeVisible();
    expect(within(visible).queryByText(/5H/)).not.toBeInTheDocument();
    expect(within(visible).queryByText(/100%/)).not.toBeInTheDocument();
    expect(
      within(visible).getByTestId("taskbar-status-quota-track")
        .querySelectorAll('[data-testid="taskbar-status-metric"]'),
    ).toHaveLength(1);
  });

  it("ignores the retired external-measurement query on the visible route", async () => {
    window.history.replaceState({}, "", "/?measurement=external");
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "ProofUser@example.com";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);

    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    expect(await within(visible).findByText(/周 98%|Wk 98%/)).toBeVisible();
    expect(within(visible).getByText("ProofU")).toBeVisible();
    expect(within(visible).getByText("8/20")).toBeVisible();
    expect(screen.queryByTestId("taskbar-status-measurement")).toBeNull();
  });

  it("renders only the universal weekly metric from a multi-window payload", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.usageByProfile.personal!.additionalWindows = [{
      limitId: "spark", label: "Spark quota", usedPercent: 12, remainingPercent: 88,
      windowDurationMinutes: 1_440, resetsAt: "2026-08-20T00:00:00Z", reachedType: null,
    }];
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    expect(await within(visible).findByText(/周 61%|Wk 61%/)).toBeInTheDocument();
    expect(within(visible).queryByText("5H 42%")).not.toBeInTheDocument();
    expect(within(visible).queryByText("Spark 88%")).not.toBeInTheDocument();
    expect(within(visible).getAllByText(/周 61%|Wk 61%/)).toHaveLength(1);
  });

  it("keeps the universal metric in the quota track while reserving reset", async () => {
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
    await within(visible).findByText(/周 61%|Wk 61%/);
    const track = within(visible).getByTestId("taskbar-status-quota-track");
    const reset = within(visible).getByTestId("taskbar-status-reset");
    expect(track.querySelectorAll('[data-testid="taskbar-status-metric"]')).toHaveLength(1);
    expect(within(visible).queryByText("Legiti 90%")).not.toBeInTheDocument();
    expect(track).toContainElement(within(visible).getByText(/周 61%|Wk 61%/));
    expect(track).not.toContainElement(reset);
    expect(visible.lastElementChild).toBe(screen.getByRole("button", { name: /打开完整面板/ }));
  });

  it("ignores model-specific windows without producing React key warnings", async () => {
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
    await within(visible).findByText(/周 61%|Wk 61%/);
    expect(within(visible).queryByText(/Burst (80|70)%/)).not.toBeInTheDocument();
    expect(consoleError.mock.calls.flat().join(" ")).not.toContain("same key");
    consoleError.mockRestore();
  });

  it.each([[100,"high"],[67,"high"],[66,"medium"],[34,"medium"],[33,"low"],[0,"low"]] as const)(
    "renders %s remaining with the %s band", async (remaining, band) => {
      const bootstrap = bootstrapWithTwoProfiles();
      bootstrap.usageByProfile.personal = weeklyOnlyUsage({
        remainingPercent: remaining,
        usedPercent: 100 - remaining,
      });
      invokeMock.mockResolvedValue(bootstrap);
      render(<TaskbarStatus />);
      const visible = await screen.findByTestId("taskbar-status-visible");
      expect(await within(visible).findByTestId("taskbar-status-metric")).toHaveAttribute("data-band", band);
    },
  );

  it("announces cached data and its update time while rendering colored metric bands", async () => {
    const bootstrap = staleOfflineFixture();
    bootstrap.usageByProfile.personal!.secondary = weeklyOnlyUsage({
      remainingPercent: 42,
      usedPercent: 58,
    }).primary;
    bootstrap.usageByProfile.personal!.fetchedAt = new Date().toISOString();
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const main = await screen.findByRole("button", { name: /缓存.*0分钟前/ });
    const visible = screen.getByTestId("taskbar-status-visible");
    expect(within(visible).getByTestId("taskbar-status-metric")).toHaveAttribute("data-band", "medium");
    expect(main).toHaveAccessibleName(expect.stringContaining("缓存"));
    expect(main).toHaveAccessibleName(expect.stringContaining("0分钟前"));
  });

  it.each([[0, "1"], [20, "0.8"], [80, "0.2"]])(
    "attaches taskbar transparency %s to the rendered root as alpha %s",
    async (opacity, expectedAlpha) => {
      const bootstrap = readyTwoWindowFixture();
      bootstrap.settings.taskbarStatusOpacity = opacity;
      invokeMock.mockResolvedValue(bootstrap);
      render(<TaskbarStatus />);

      const visible = await screen.findByTestId("taskbar-status-visible");
      await within(visible).findByText(/周 61%|Wk 61%/);
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
    await within(visible).findByText(/周 61%|Wk 61%/);
    await waitFor(() =>
      expect(eventHarness.listeners.get(events.settingsChanged)?.size).toBeGreaterThan(0),
    );
    expect(visible.style.getPropertyValue("--surface-bg-alpha")).toBe("0.8");

    act(() => {
      eventHarness.emit(events.settingsChanged, {
        ...bootstrap.settings,
        taskbarStatusOpacity: 80,
      });
    });

    await waitFor(() =>
      expect(visible.style.getPropertyValue("--surface-bg-alpha")).toBe("0.2"),
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
});
