import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import {
  bootstrapWithTwoProfiles,
  readyTwoWindowFixture,
  staleOfflineFixture,
} from "../test/profileUsageFixtures";
import { events } from "../lib/tauri";
import TrayPanel from "./TrayPanel";

type EventCallback = (event: { payload: unknown }) => void;

const eventHarness = vi.hoisted(() => {
  const listeners = new Map<string, EventCallback>();
  return {
    listeners,
    emit(eventName: string, payload: unknown) {
      listeners.get(eventName)?.({ payload });
    },
    listen(eventName: string, callback: EventCallback) {
      listeners.set(eventName, callback);
      return Promise.resolve(() => listeners.delete(eventName));
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: (eventName: string, callback: EventCallback) =>
    eventHarness.listen(eventName, callback),
}));

function renderTray(bootstrap = bootstrapWithTwoProfiles()) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_bootstrap_state") return bootstrap;
    if (command === "select_profile") {
      return {
        profiles: bootstrap.profiles,
        selectedProfileId: "work",
      };
    }
    return undefined;
  });
  return render(<TrayPanel />);
}

describe("TrayPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    eventHarness.listeners.clear();
  });

  it("renders account, short window, long window, state, and actions in order", async () => {
    renderTray(readyTwoWindowFixture());

    expect(await screen.findByRole("main")).toHaveClass("tray-panel--macos");
    expect(
      await screen.findByRole("region", { name: /account/i }),
    ).toHaveClass(
      "tray-account--card",
    );
    expect(
      await screen.findByRole("progressbar", { name: /5-hour quota/i }),
    ).toHaveClass("quota-card--ready");

    const regions = (
      await screen.findAllByRole("region")
    ).map((node) => node.getAttribute("aria-label"));

    expect(regions).toEqual([
      "Account",
      "5-hour quota",
      "Weekly quota",
      "Data status",
      "Actions",
    ]);
  });

  it("keeps cached quota visible beside an offline error", async () => {
    renderTray(staleOfflineFixture());

    expect(
      await screen.findByRole("progressbar", {
        name: /5-hour quota.*42% remaining/i,
      }),
    ).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Offline");
    expect(screen.getByText(/last updated/i)).toBeInTheDocument();
  });

  it("switches the displayed value and progress semantics to used mode", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.settings.displayMode = "used";
    renderTray(bootstrap);

    const progress = await screen.findByRole("progressbar", {
      name: /5-hour quota.*58% used/i,
    });
    expect(progress).toHaveAttribute("aria-valuenow", "58");
    expect(screen.getByText("58% used")).toBeInTheDocument();
  });

  it("formats an unknown window duration without dropping the quota", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal.primary!.windowDurationMinutes = 37;
    bootstrap.usageByProfile.personal.primary!.label = null;
    renderTray(bootstrap);

    expect(
      await screen.findByRole("region", { name: /37 minutes quota/i }),
    ).toBeInTheDocument();
  });

  it("marks an expired reset as awaiting refresh without a negative countdown", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal.primary!.resetsAt =
      "2020-01-01T00:00:00Z";
    renderTray(bootstrap);

    expect(await screen.findByText("Awaiting refresh")).toBeInTheDocument();
    expect(screen.queryByText(/-\d/)).toBeNull();
  });

  it("renders missing, refreshing, and protocol-anomaly status text", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = {
      ...bootstrap.usageByProfile.personal,
      primary: null,
      freshness: "missing",
      refreshStatus: "refreshing",
      protocolAnomaly: true,
    };
    renderTray(bootstrap);

    expect(await screen.findByText(/refreshing/i)).toBeInTheDocument();
    expect(screen.getByText(/no usage data/i)).toBeInTheDocument();
    expect(screen.getByText(/protocol/i)).toBeInTheDocument();
  });

  it("maps API billing errors to the fixed usage-page action", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = {
      ...bootstrap.usageByProfile.personal,
      primary: null,
      freshness: "missing",
      currentError: {
        kind: "apiKeyNoQuota",
        userMessageKey: "errors.apiKeyNoQuota",
        action: "explainApiBilling",
        retryAfter: null,
      },
    };
    renderTray(bootstrap);

    expect(
      await screen.findByRole("button", { name: /open usage/i }),
    ).toBeInTheDocument();
  });

  it("focuses the profile selector first and dismisses on Escape", async () => {
    renderTray();
    const selector = await screen.findByRole("button", {
      name: /profile/i,
    });
    expect(selector).toHaveFocus();

    fireEvent.keyDown(selector, { key: "Escape" });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("dismiss_tray_panel");
    });
  });

  it("hides the panel from the macOS-style close button without quitting", async () => {
    renderTray();
    await screen.findByRole("button", { name: /profile/i });

    const close = await screen.findByRole("button", {
      name: /hide panel|隐藏面板/i,
    });
    fireEvent.click(close);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("dismiss_tray_panel");
    });
    expect(invokeMock).not.toHaveBeenCalledWith("quit_app");
  });

  it("keeps native select and button activation available to keyboard users", async () => {
    renderTray();
    const selector = await screen.findByRole("button", {
      name: /profile/i,
    });

    fireEvent.click(selector);
    fireEvent.click(screen.getByRole("option", { name: /Work/i }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("select_profile", {
        profileId: "work",
      });
    });

    const refresh = screen.getByRole("button", { name: /refresh/i });
    refresh.focus();
    fireEvent.keyDown(refresh, { key: "Enter" });
    fireEvent.click(refresh);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "refresh_selected_profile",
      );
    });
  });

  it("shows a switching status while the target cache waits for its refresh event", async () => {
    renderTray();
    const selector = await screen.findByRole("button", {
      name: /profile/i,
    });

    fireEvent.click(selector);
    fireEvent.click(screen.getByRole("option", { name: /Work/i }));

    expect(await screen.findByText(/switching/i)).toBeInTheDocument();
    expect(
      screen.getByRole("progressbar", { name: /61% remaining/i }),
    ).toBeInTheDocument();
  });

  it("supports Simplified Chinese copy without changing semantic order", async () => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.settings.language = "zh-CN";
    renderTray(bootstrap);

    expect(await screen.findAllByText(/剩余/)).not.toHaveLength(0);
    expect(screen.getAllByRole("region")).toHaveLength(5);
  });

  it("renders a long account label without exposing an email", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.profiles[0]!.label = "Personal account with a deliberately long label";
    bootstrap.profiles[0]!.email = null;
    renderTray(bootstrap);
    const selector = await screen.findByRole("button", { name: /profile/i });
    fireEvent.click(selector);

    expect(
      await screen.findByRole("option", {
        name: "Personal account with a deliberately long label",
      }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/@/)).toBeNull();
  });
});
