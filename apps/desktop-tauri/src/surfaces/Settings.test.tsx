import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Settings from "./Settings";
import AccountsTab from "./settings/tabs/AccountsTab";
import { invokeMock } from "../test/setup";

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

const defaultSettings = {
  autostartEnabled: false,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  theme: "system",
  language: "system",
  codexExecutableOverride: null,
  taskbarStatusEnabled: false,
  floatBallEnabled: false,
  taskbarTransparencyPercent: 20,
  floatBallTransparencyPercent: 20,
  floatBallGlowPercent: 20,
  notifications: {
    enabled: false,
    playSound: true,
    warningEnabled: true,
    dangerEnabled: true,
    weeklyResetEnabled: true,
    resetCreditIncreaseEnabled: true,
    refreshFailureEnabled: true,
    updateAvailableEnabled: true,
    warningRemainingPercent: 66,
    dangerRemainingPercent: 33,
  },

  menu: {
    nativeTray: {
      order: [
        "open_panel",
        "refresh",
        "accounts",
        "open_usage",
        "settings",
        "about",
        "quit",
      ],
      hidden: [],
    },
    trayPanel: {
      order: ["refresh", "open_usage", "settings", "dismiss", "quit"],
      hidden: [],
    },
  },

  taskbarPresentation: {
    showTaskbarIcon: true,
    showTaskbarAccount: true,
    showWeeklyLabel: true,
    showWeeklyPercent: true,
    showResetDate: true,
    density: "compact",
    hideStatusSurfacesInFullscreen: true,
  },
};

function renderSettings(language: "system" | "zh-CN" | "en-US" = "system") {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_bootstrap_state") {
      return {
        productName: "codex-barbar",
        version: "9.8.7",
        settings: { ...defaultSettings, language },
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
    }
    if (command === "get_settings_snapshot") {
      return { ...defaultSettings, language };
    }
    return undefined;
  });
  return render(<Settings />);
}

describe("Settings surface", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("mentions the codex-barbar identity and build information", async () => {
    renderSettings();
    expect(
      await screen.findByRole("heading", { name: "codex-barbar Settings" }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "About" }));
    expect(await screen.findByText("Version 9.8.7")).toBeInTheDocument();
  });

  it("uses Chinese sidebar labels and changes the visible pane", async () => {
    renderSettings("zh-CN");

    for (const label of [
      "通用",
      "账户",
      "通知",
      "任务栏与悬浮球",
      "面板",
      "用量与费用",
      "高级",
      "关于",
    ]) {
      expect(await screen.findByRole("button", { name: label })).toBeInTheDocument();
    }

    fireEvent.click(screen.getByRole("button", { name: "通知" }));
    expect(
      screen.getByRole("heading", { name: "通知" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "启用通知" })).toBeInTheDocument();
  });

  it("renders the concrete Taskbar & Float Ball pane instead of a placeholder", async () => {
    renderSettings("en-US");
    fireEvent.click(await screen.findByRole("button", { name: "Taskbar & Float Ball" }));

    expect(screen.getByRole("heading", { name: "Taskbar & Float Ball" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Taskbar status" })).toBeInTheDocument();
    expect(screen.queryByText("reserved for a later release", { exact: false })).not.toBeInTheDocument();
  });

  it("retains English sidebar labels for en-US", async () => {
    renderSettings("en-US");

    expect(await screen.findByRole("button", { name: "General" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Accounts" })).toBeInTheDocument();
  });

  it("moves sidebar focus and the selected pane with arrow keys", async () => {
    renderSettings("en-US");
    const general = await screen.findByRole("button", { name: "General" });
    general.focus();
    fireEvent.keyDown(general, { key: "ArrowDown" });
    expect(screen.getByRole("button", { name: "Accounts" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "Accounts" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.keyDown(screen.getByRole("button", { name: "Accounts" }), {
      key: "ArrowUp",
    });
    expect(general).toHaveFocus();
    expect(general).toHaveAttribute("aria-pressed", "true");
  });

  it("renders sidebar items as native buttons with click selection", async () => {
    renderSettings("en-US");
    const accounts = await screen.findByRole("button", { name: "Accounts" });
    expect(accounts.tagName).toBe("BUTTON");
    fireEvent.click(accounts);
    expect(accounts).toHaveAttribute("aria-pressed", "true");
  });

  it("closes through both the visible control and Escape outside native fields", async () => {
    renderSettings("en-US");
    await screen.findByRole("heading", { name: "codex-barbar Settings" });
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(invokeMock).toHaveBeenCalledWith("close_settings_window");

    invokeMock.mockClear();
    fireEvent.keyDown(screen.getByRole("main"), { key: "Escape" });
    expect(invokeMock).toHaveBeenCalledWith("close_settings_window");

    invokeMock.mockClear();
    fireEvent.keyDown(screen.getByRole("combobox", { name: "Language" }), {
      key: "Escape",
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("localizes reachable account, advanced, and about content in Chinese", async () => {
    renderSettings("zh-CN");
    fireEvent.click(await screen.findByRole("button", { name: "账户" }));
    expect(screen.getByRole("heading", { name: "账户" })).toBeInTheDocument();
    expect(screen.getByLabelText("新账户名称")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "添加账户" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Accounts" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "高级" }));
    expect(screen.getByLabelText("Codex 可执行文件路径")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "验证并保存" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Validate and save" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关于" }));
    expect(screen.getByText("适用于 Codex 用量的 Windows 11 托盘伴侣。", { exact: false })).toBeInTheDocument();
    expect(screen.getByText("当前版本 9.8.7")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check for updates" })).not.toBeInTheDocument();
  });

  it("localizes the managed-login dialog in Chinese", async () => {
    renderSettings("zh-CN");
    fireEvent.click(await screen.findByRole("button", { name: "账户" }));
    fireEvent.click(screen.getByRole("button", { name: "添加账户" }));
    expect(screen.getByRole("heading", { name: "账户登录" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "浏览器登录" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Browser login" })).not.toBeInTheDocument();
  });

  it("renders every tab with a real heading and no placeholder", async () => {
    renderSettings("en-US");
    const tabs: Array<[string, string]> = [
      ["General", "General"],
      ["Accounts", "Accounts"],
      ["Notifications", "Notifications"],
      ["Taskbar & Float Ball", "Taskbar & Float Ball"],
      ["Panel", "Panel"],
      ["Usage & spend", "Usage & Spend"],
      ["Advanced", "Advanced"],
      ["About", "About"],
    ];
    for (const [buttonLabel, heading] of tabs) {
      fireEvent.click(await screen.findByRole("button", { name: buttonLabel }));
      expect(await screen.findByRole("heading", { name: heading })).toBeInTheDocument();
      expect(
        screen.queryByText(/reserved for a later release/i),
      ).not.toBeInTheDocument();
    }
  });
  it("never offers remove or re-login for Current CLI", () => {
+


    render(
      <AccountsTab
        profiles={[
          {
            id: "personal",
            kind: "currentCli",
            label: "Personal",
            email: null,
            accountDisplayName: null,
            accountEmail: null,
            accountStatus: "unavailable",
            accountUpdatedAt: null,
            planType: "plus",
            presentationName: "账号信息不可用",
            avatarKind: "default",
            avatarAssetUri: null,
            authMode: "chatGpt",
            removable: false,
            lastSuccessAt: null,
          },
        ]}
        selectedProfileId="personal"
        loginState={null}
        onSelect={() => {}}
        onRename={async () => {}}
        onRemove={async () => {}}
        onStartLogin={() => {}}
        onCancelLogin={() => {}}
      />,
    );
    expect(
      screen.queryByRole("button", { name: /remove/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /re-login/i }),
    ).not.toBeInTheDocument();
  });
});
