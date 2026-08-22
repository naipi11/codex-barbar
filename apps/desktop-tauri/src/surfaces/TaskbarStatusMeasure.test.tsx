import { render, screen, waitFor, within } from "@testing-library/react";
// @ts-ignore Vitest executes tests in Node; the browser build does not include test modules.
import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import {
  bootstrapWithTwoProfiles,
  weeklyOnlyUsage,
} from "../test/profileUsageFixtures";
import TaskbarStatus from "./TaskbarStatus";
import TaskbarStatusMeasure from "./TaskbarStatusMeasure";

const taskbarStatusCss = readFileSync("src/surfaces/TaskbarStatus.css", "utf8");

describe("TaskbarStatusMeasure", () => {
  beforeEach(() => invokeMock.mockReset());

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("keeps helper content intrinsic, capped, and off-screen", () => {
    const start = taskbarStatusCss.indexOf(".taskbar-status--measurement {");
    const rule = taskbarStatusCss.slice(start, taskbarStatusCss.indexOf("}", start));
    expect(rule).toContain("position: fixed");
    expect(rule).toContain("left: -10000px");
    expect(rule).toContain("width: max-content");
    expect(rule).toContain("max-width: 318px");
    expect(rule).not.toContain("display: none");
    expect(rule).not.toContain("content-visibility: hidden");
  });

  it("renders only inert weekly measurement geometry", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "ProofUser";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);

    render(<TaskbarStatusMeasure />);

    const measurement = await screen.findByTestId("taskbar-status-measurement");
    expect(await within(measurement).findByText(/周 98%|Wk 98%/)).toBeInTheDocument();
    expect(within(measurement).getByText("ProofU")).toBeInTheDocument();
    expect(within(measurement).getByText("8/20")).toBeInTheDocument();
    expect(within(measurement).queryByText(/5H/)).toBeNull();
    expect(measurement).toHaveAttribute("aria-hidden", "true");
    expect(measurement).toHaveAttribute("inert");
    expect(screen.queryByTestId("taskbar-status-visible")).toBeNull();
  });

  it("uses the same runtime alpha on the independent measurement root", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.taskbarStatusOpacity = 80;
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatusMeasure />);

    const measurement = await screen.findByTestId("taskbar-status-measurement");
    await waitFor(() =>
      expect(measurement.style.getPropertyValue("--surface-bg-alpha")).toBe("0.8"),
    );
  });

  it("keeps the rendered root alpha from being shadowed by a descendant fallback", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.settings.taskbarStatusOpacity = 80;
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatusMeasure />);

    const measurement = await screen.findByTestId("taskbar-status-measurement");
    await waitFor(() =>
      expect(measurement.style.getPropertyValue("--surface-bg-alpha")).toBe("0.8"),
    );
    for (const descendant of measurement.querySelectorAll<HTMLElement>("*")) {
      expect(descendant.style.getPropertyValue("--surface-bg-alpha")).toBe("");
    }

    const alphaDeclarationSelectors = Array.from(
      taskbarStatusCss.matchAll(/([^{}]+)\{[^{}]*--surface-bg-alpha\s*:[^{}]*\}/g),
      ([, selector]) => selector.trim(),
    );
    expect(alphaDeclarationSelectors).toEqual([".taskbar-status"]);
  });

  it("renders the exact visible geometry sequence without a close column", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.accountDisplayName = "ProofUser";
    bootstrap.usageByProfile.personal = weeklyOnlyUsage();
    invokeMock.mockResolvedValue(bootstrap);

    const visibleView = render(<TaskbarStatus />);
    const visible = await screen.findByTestId("taskbar-status-visible");
    await within(visible).findByText(/周 98%|Wk 98%/);
    const geometry = (root: HTMLElement) =>
      Array.from(
        root.querySelectorAll(
          ".taskbar-status__avatar, .taskbar-status__identity, .taskbar-status__metric, .taskbar-status__reset",
        ),
      ).map((element) => `${element.className}:${element.textContent}`);
    const visibleGeometry = geometry(visible);
    visibleView.unmount();

    const measurementView = render(<TaskbarStatusMeasure />);
    const measurement = await screen.findByTestId("taskbar-status-measurement");
    await within(measurement).findByText(/周 98%|Wk 98%/);
    const measurementGeometry = geometry(measurement);
    expect(measurementGeometry).toEqual(visibleGeometry);
    expect(visibleGeometry).toEqual([
      "taskbar-status__avatar:",
      "taskbar-status__identity:ProofU",
      "taskbar-status__metric:Wk 98%",
      "taskbar-status__reset:8/20",
    ]);
    measurementView.unmount();
  });

  it("owns the only ResizeObserver and submits a 247px measurement exactly once", async () => {
    class ResizeObserverStub {
      static instances: ResizeObserverStub[] = [];
      readonly observe = vi.fn();
      readonly disconnect = vi.fn();

      constructor(_callback: ResizeObserverCallback) {
        ResizeObserverStub.instances.push(this);
      }
    }
    vi.stubGlobal("ResizeObserver", ResizeObserverStub);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width: 279,
    } as DOMRect);
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(247);
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockImplementation(async (command: string) =>
      command === "get_bootstrap_state" ? bootstrap : undefined,
    );

    render(<TaskbarStatusMeasure />);

    await screen.findByTestId("taskbar-status-measurement");
    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_taskbar_status_width", {
        width: 295,
      }),
    );
    expect(ResizeObserverStub.instances).toHaveLength(1);
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "set_taskbar_status_width"),
    ).toHaveLength(1);
  });

  it("disables close-error animation when reduced motion is requested", () => {
    expect(taskbarStatusCss).toMatch(
      /@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?\.taskbar-status__close\[data-error="true"\]\s*\{\s*animation:\s*none;?\s*\}/,
    );
  });
});

