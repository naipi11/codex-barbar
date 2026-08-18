import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { invokeMock } from "./test/setup";

const webviewWindowMocks = vi.hoisted(() => ({
  label: "main",
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ label: webviewWindowMocks.label }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

import App from "./App";

const bootstrapFixture = {
  productName: "codex-barbar",
  version: "1.0.0",
  settings: {
    autostartEnabled: false,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    theme: "system",
    language: "system",
    codexExecutableOverride: null,
    taskbarStatusEnabled: false,
    floatBallEnabled: false,
    taskbarStatusOpacity: 20,
    floatBallOpacity: 20,
  },
  profiles: [],
  selectedProfileId: "",
  usageByProfile: {},
  statusSurfaceFeedback: {
    taskbarStatusCloseFailed: false,
    floatBallCloseFailed: false,
  },
  codex: {
    status: "notChecked",
    installation: null,
    executablePath: null,
    version: null,
    capabilities: {
      accountRead: false,
      rateLimitsRead: false,
      managedLogin: false,
    },
  },
};

describe("App", () => {
  it("bootstraps once without checking for updates", async () => {
    webviewWindowMocks.label = "main";
    invokeMock.mockResolvedValue(bootstrapFixture);

    render(<App />);

    await waitFor(() => {
      expect(
        screen.getByRole("region", { name: "Account" }),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByRole("heading", { name: "codex-barbar" }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_bootstrap_state");
    });
    expect(
      invokeMock.mock.calls.filter(([name]) => name === "get_bootstrap_state"),
    ).toHaveLength(1);
    expect(invokeMock).not.toHaveBeenCalledWith(
      "check_for_updates",
      expect.anything(),
    );
  });

  it("routes only the settings window to Settings", async () => {
    webviewWindowMocks.label = "settings";
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_bootstrap_state") return bootstrapFixture;
      if (command === "get_settings_snapshot") {
        return bootstrapFixture.settings;
      }
      return undefined;
    });
    render(<App />);
    expect(
      await screen.findByRole("heading", { name: "codex-barbar Settings" }),
    ).toBeInTheDocument();
  });

  it("routes auxiliary status windows to their compact surfaces", async () => {
    invokeMock.mockResolvedValue(bootstrapFixture);

    webviewWindowMocks.label = "taskbar-status";
    const taskbar = render(<App />);
    expect(await taskbar.findByTestId("taskbar-status-content")).toBeInTheDocument();
    expect(
      await taskbar.findByRole("button", { name: /打开完整面板/ }),
    ).toBeInTheDocument();
    taskbar.unmount();

    webviewWindowMocks.label = "float-ball";
    const floatBall = render(<App />);
    expect(
      await floatBall.findByRole("button", { name: /unavailable/i }),
    ).toBeInTheDocument();
  });

  it("routes the hidden taskbar measurement window to measurement-only content", async () => {
    webviewWindowMocks.label = "taskbar-status-measure";
    invokeMock.mockResolvedValue(bootstrapFixture);

    render(<App />);

    expect(await screen.findByTestId("taskbar-status-measurement")).toBeInTheDocument();
    expect(screen.queryByTestId("taskbar-status-visible")).toBeNull();
  });

  it("does not render the legacy flyout or unknown window labels", () => {
    webviewWindowMocks.label = "flyout";
    const { container } = render(<App />);
    expect(container.firstChild).toBeNull();

    webviewWindowMocks.label = "floatbar";
    const second = render(<App />);
    expect(second.container.firstChild).toBeNull();
  });
});
