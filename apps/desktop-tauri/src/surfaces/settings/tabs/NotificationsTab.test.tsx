import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  AppSettingsDto,
  NotificationCapabilityDto,
} from "../../../types/bridge";
import { settingsCopy } from "../settingsCopy";
import NotificationsTab from "./NotificationsTab";

const settings: AppSettingsDto = {
  autostartEnabled: false,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  theme: "system",
  language: "en-US",
  codexExecutableOverride: null,
  taskbarStatusEnabled: false,
  floatBallEnabled: false,
  taskbarTransparencyPercent: 20,
  floatBallTransparencyPercent: 20,
  floatBallGlowPercent: 20,
  taskbarPresentation: {
    showTaskbarIcon: true,
    showTaskbarAccount: true,
    showWeeklyLabel: true,
    showWeeklyPercent: true,
    showResetDate: true,
    density: "compact",
    hideStatusSurfacesInFullscreen: true,
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
  panel: {
    density: "compact",
    showResetTime: true,
    showFreshness: true,
    showAccountStatus: true,
    actions: {
      order: ["refresh", "open_usage", "settings", "dismiss", "quit"],
      hidden: [],
    },
  },
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
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((next, fail) => {
    resolve = next;
    reject = fail;
  });
  return { promise, reject, resolve };
}

describe("NotificationsTab", () => {
  it("patches only the notification master field", () => {
    const update = vi.fn().mockResolvedValue(settings);
    render(
      <NotificationsTab
        settings={settings}
        update={update}
        copy={settingsCopy("en-US")}
      />,
    );

    const master = screen.getByRole("checkbox", { name: /enable notifications/i });
    expect(master).not.toBeChecked();
    fireEvent.click(master);
    expect(update).toHaveBeenCalledWith({ notifications: { enabled: true } });
  });

  it("disables event switches with the master off and exposes saved thresholds", () => {
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
      />,
    );

    for (const name of [
      /warning band/i,
      /danger band/i,
      /weekly allowance resets/i,
      /refresh fails/i,
      /new release/i,
    ]) {
      expect(screen.getByRole("checkbox", { name })).toBeDisabled();
    }
    expect(screen.getByRole("spinbutton", { name: /warning remaining/i })).toHaveValue(66);
    expect(screen.getByRole("spinbutton", { name: /danger remaining/i })).toHaveValue(33);
  });

  it("does not expose reset-credit notifications as an operational switch", () => {
    render(
      <NotificationsTab
        settings={{
          ...settings,
          notifications: { ...settings.notifications, enabled: true },
        }}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
      />,
    );

    expect(
      screen.queryByRole("checkbox", { name: /reset credits increase/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/reset-credit notifications are not available yet/i)).toBeInTheDocument();
  });

  it("shows rejected threshold patches inline and retains saved values", async () => {
    const update = vi.fn().mockRejectedValue("SETTINGS_NOTIFICATION_THRESHOLDS_INVALID");
    render(
      <NotificationsTab
        settings={settings}
        update={update}
        copy={settingsCopy("en-US")}
      />,
    );
    const warning = screen.getByRole("spinbutton", { name: /warning remaining/i });
    fireEvent.change(warning, { target: { value: "20" } });

    expect(await screen.findByRole("alert")).toHaveTextContent(/danger.*lower.*warning/i);
    expect(warning).toHaveValue(66);
  });

  it("maps non-threshold save failures to the generic save diagnostic", async () => {
    const update = vi.fn().mockRejectedValue("SETTINGS_SAVE_FAILED");
    render(
      <NotificationsTab
        settings={settings}
        update={update}
        copy={settingsCopy("en-US")}
      />,
    );
    fireEvent.change(screen.getByRole("spinbutton", { name: /warning remaining/i }), {
      target: { value: "20" },
    });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Notification settings could not be saved. Try again.",
    );
    expect(screen.getByRole("spinbutton", { name: /warning remaining/i })).toHaveValue(66);
  });

  it("localizes every control in English and Simplified Chinese", () => {
    const { unmount } = render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
      />,
    );
    expect(screen.getByRole("heading", { name: "Notifications" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send test notification" })).toBeInTheDocument();
    unmount();

    render(
      <NotificationsTab
        settings={{ ...settings, language: "zh-CN" }}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("zh-CN")}
      />,
    );
    expect(screen.getByRole("heading", { name: "通知" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "启用通知" })).toBeInTheDocument();
    expect(
      screen.getByText("重置额度通知暂不可用，后续用量历史功能将启用此选项。"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "发送测试通知" })).toBeInTheDocument();
  });

  it("maps a failed test action to a localized inline diagnostic", async () => {
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        sendTest={vi.fn().mockRejectedValue("NOTIFICATION_TEST_FAILED")}
        getCapability={vi.fn().mockResolvedValue({
          status: "available",
          canOpenSettings: true,
        })}
      />,
    );
    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    await waitFor(() => expect(sendButton).toBeEnabled());
    fireEvent.click(sendButton);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Windows could not send the test notification. Check notification settings and try again.",
    );
  });

  it("does not send or show sent when clicked while capability is loading", () => {
    const capability = deferred<NotificationCapabilityDto>();
    const sendTest = vi.fn().mockResolvedValue(undefined);
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        sendTest={sendTest}
        getCapability={vi.fn().mockReturnValue(capability.promise)}
      />,
    );

    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    expect(sendButton).toBeDisabled();
    fireEvent.click(sendButton);

    expect(sendTest).not.toHaveBeenCalled();
    expect(screen.queryByText("Test notification sent.")).not.toBeInTheDocument();
  });

  it("shows app-disabled recovery and never reports a suppressed test as sent", async () => {
    const openNotificationSettings = vi.fn().mockResolvedValue(undefined);
    const sendTest = vi.fn().mockRejectedValue("NOTIFICATION_PERMISSION_DISABLED");
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        sendTest={sendTest}
        getCapability={vi.fn().mockResolvedValue({
          status: "appDisabled",
          canOpenSettings: true,
        })}
        openNotificationSettings={openNotificationSettings}
      />,
    );

    expect(
      await screen.findByText(/notifications for codex-barbar are turned off in windows/i),
    ).toBeInTheDocument();
    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    expect(sendButton).toBeDisabled();
    fireEvent.click(sendButton);

    expect(sendTest).not.toHaveBeenCalled();
    expect(screen.queryByText("Test notification sent.")).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Open Windows notification settings" }),
    );
    expect(openNotificationSettings).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Open Windows notification settings" }),
      ).toBeEnabled(),
    );
  });

  it("refreshes capability after focus returns from Windows notification settings", async () => {
    const getCapability = vi
      .fn()
      .mockResolvedValueOnce({ status: "appDisabled", canOpenSettings: true })
      .mockResolvedValueOnce({ status: "available", canOpenSettings: true });
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        getCapability={getCapability}
      />,
    );

    expect(
      await screen.findByText(/notifications for codex-barbar are turned off in windows/i),
    ).toBeInTheDocument();

    act(() => window.dispatchEvent(new Event("focus")));

    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(
        screen.queryByText(/notifications for codex-barbar are turned off in windows/i),
      ).not.toBeInTheDocument(),
    );
  });

  it("disables a previously available test action while a focus refresh is pending", async () => {
    const focusCapability = deferred<NotificationCapabilityDto>();
    const getCapability = vi
      .fn()
      .mockResolvedValueOnce({ status: "available", canOpenSettings: true })
      .mockReturnValueOnce(focusCapability.promise);
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        getCapability={getCapability}
      />,
    );

    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    await waitFor(() => expect(sendButton).toBeEnabled());

    act(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(2));
    expect(sendButton).toBeDisabled();

    await act(async () => {
      focusCapability.resolve({ status: "available", canOpenSettings: true });
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
  });

  it("keeps the test action disabled while a disabled-code refresh is pending", async () => {
    const refreshedCapability = deferred<NotificationCapabilityDto>();
    const getCapability = vi
      .fn()
      .mockResolvedValueOnce({ status: "available", canOpenSettings: true })
      .mockReturnValueOnce(refreshedCapability.promise);
    const sendTest = vi.fn().mockRejectedValue("NOTIFICATION_PERMISSION_DISABLED");
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        getCapability={getCapability}
        sendTest={sendTest}
      />,
    );

    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    await waitFor(() => expect(sendButton).toBeEnabled());
    fireEvent.click(sendButton);

    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(2));
    expect(sendTest).toHaveBeenCalledTimes(1);
    expect(sendButton).toBeDisabled();
    expect(screen.queryByText("Test notification sent.")).not.toBeInTheDocument();
  });

  it("does not let stale fulfillment or rejection clear loading for a newer request", async () => {
    const staleFulfillment = deferred<NotificationCapabilityDto>();
    const staleRejection = deferred<NotificationCapabilityDto>();
    const newerFocusCapability = deferred<NotificationCapabilityDto>();
    const getCapability = vi
      .fn()
      .mockResolvedValueOnce({ status: "available", canOpenSettings: true })
      .mockReturnValueOnce(staleFulfillment.promise)
      .mockReturnValueOnce(staleRejection.promise)
      .mockReturnValueOnce(newerFocusCapability.promise);
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        getCapability={getCapability}
      />,
    );

    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    await waitFor(() => expect(sendButton).toBeEnabled());

    act(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(2));
    act(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(3));

    await act(async () => {
      staleFulfillment.resolve({ status: "available", canOpenSettings: true });
    });
    expect(sendButton).toBeDisabled();

    act(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(4));
    await act(async () => {
      staleRejection.reject(new Error("stale capability failure"));
    });
    expect(sendButton).toBeDisabled();
    expect(
      screen.queryByText(/availability could not be checked/i),
    ).not.toBeInTheDocument();

    await act(async () => {
      newerFocusCapability.resolve({ status: "available", canOpenSettings: true });
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
  });

  it("keeps the newer focus capability when the older mount request resolves last", async () => {
    const mountCapability = deferred<NotificationCapabilityDto>();
    const focusCapability = deferred<NotificationCapabilityDto>();
    const getCapability = vi
      .fn()
      .mockReturnValueOnce(mountCapability.promise)
      .mockReturnValueOnce(focusCapability.promise);
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        getCapability={getCapability}
      />,
    );

    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(1));
    act(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => expect(getCapability).toHaveBeenCalledTimes(2));

    await act(async () => {
      focusCapability.resolve({ status: "available", canOpenSettings: true });
    });
    const sendButton = screen.getByRole("button", { name: "Send test notification" });
    await waitFor(() => expect(sendButton).toBeEnabled());

    await act(async () => {
      mountCapability.resolve({ status: "appDisabled", canOpenSettings: true });
    });

    expect(sendButton).toBeEnabled();
    expect(
      screen.queryByText(/notifications for codex-barbar are turned off in windows/i),
    ).not.toBeInTheDocument();
  });

  it("localizes the app-disabled recovery in Simplified Chinese", async () => {
    render(
      <NotificationsTab
        settings={{ ...settings, language: "zh-CN" }}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("zh-CN")}
        getCapability={vi.fn().mockResolvedValue({
          status: "appDisabled",
          canOpenSettings: true,
        })}
      />,
    );

    expect(
      await screen.findByText(/windows 已关闭 codex-barbar 的通知/i),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "打开 Windows 通知设置" }),
    ).toBeInTheDocument();
  });

  it.each([
    ["globalDisabled", /windows notifications are turned off/i, true],
    ["unsupported", /availability could not be checked/i, false],
  ] as const)(
    "distinguishes the %s capability state",
    async (status, message, canOpenSettings) => {
      render(
        <NotificationsTab
          settings={settings}
          update={vi.fn().mockResolvedValue(settings)}
          copy={settingsCopy("en-US")}
          getCapability={vi.fn().mockResolvedValue({ status, canOpenSettings })}
        />,
      );

      expect(await screen.findByText(message)).toBeInTheDocument();
      const recovery = screen.queryByRole("button", {
        name: "Open Windows notification settings",
      });
      if (canOpenSettings) {
        expect(recovery).toBeInTheDocument();
      } else {
        expect(recovery).not.toBeInTheDocument();
      }
      expect(
        screen.getByRole("button", { name: "Send test notification" }),
      ).toBeDisabled();
    },
  );

  it("shows a localized error when Windows notification settings cannot open", async () => {
    render(
      <NotificationsTab
        settings={settings}
        update={vi.fn().mockResolvedValue(settings)}
        copy={settingsCopy("en-US")}
        getCapability={vi.fn().mockResolvedValue({
          status: "appDisabled",
          canOpenSettings: true,
        })}
        openNotificationSettings={vi.fn().mockRejectedValue("OPEN_FAILED")}
      />,
    );

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Open Windows notification settings",
      }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Windows notification settings could not be opened.",
    );
  });
});
