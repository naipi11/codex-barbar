import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AppSettingsDto } from "../../../types/bridge";
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
  taskbarStatusOpacity: 20,
  floatBallOpacity: 20,
  floatBallGlow: 20,
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
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Send test notification" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Windows could not send the test notification. Check notification settings and try again.",
    );
  });
});
