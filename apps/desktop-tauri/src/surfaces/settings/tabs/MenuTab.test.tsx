import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../../../hooks/useSettings";
import { applyMenuPreferences } from "../../../lib/tauri";
import { settingsCopy } from "../settingsCopy";
import MenuTab, { NATIVE_TRAY_ORDER } from "./MenuTab";

vi.mock("../../../lib/tauri", () => ({
  applyMenuPreferences: vi.fn(),
}));

const apply = vi.mocked(applyMenuPreferences);

describe("MenuTab", () => {
  it("renders both editors, localizes rows, and locks required native items", () => {
    apply.mockResolvedValue(defaultAppSettings);
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);

    expect(screen.getByRole("heading", { name: "Menu" })).toBeInTheDocument();
    const trayMenu = screen.getByRole("group", { name: "Tray menu" });
    const panel = screen.getByRole("group", { name: "Panel quick actions" });
    expect(trayMenu).toBeInTheDocument();
    expect(panel).toBeInTheDocument();
    expect(screen.getByText(/only built-in items can be configured/i)).toBeInTheDocument();

    expect(within(trayMenu).getByRole("checkbox", { name: "Settings" })).toBeChecked();
    expect(within(trayMenu).getByRole("checkbox", { name: "Settings" })).toBeDisabled();
    expect(within(trayMenu).getByRole("checkbox", { name: "Quit" })).toBeDisabled();
    expect(
      within(trayMenu).getByText("Settings and Quit are required and cannot be hidden."),
    ).toBeInTheDocument();
    expect(within(trayMenu).getByRole("checkbox", { name: "About" })).toBeEnabled();
    expect(within(panel).getByRole("checkbox", { name: "Dismiss" })).toBeEnabled();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("reorders native items with keyboard move buttons and emits the exact order", async () => {
    apply.mockResolvedValue(defaultAppSettings);
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);
    const trayMenu = screen.getByRole("group", { name: "Tray menu" });

    fireEvent.click(
      within(trayMenu).getByRole("button", { name: "Move down Refresh" }),
    );

    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith({
        nativeTray: {
          order: [
            "open_panel",
            "accounts",
            "refresh",
            "open_usage",
            "settings",
            "about",
            "quit",
          ],
        },
      }),
    );
  });

  it("hides and re-shows an eligible item through the visibility checkbox", async () => {
    let saved = defaultAppSettings;
    const view = render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);
    apply.mockImplementation(async (patch) => {
      saved = {
        ...saved,
        menu: {
          nativeTray: patch.nativeTray
            ? {
                order: patch.nativeTray.order ?? saved.menu.nativeTray.order,
                hidden: patch.nativeTray.hidden ?? saved.menu.nativeTray.hidden,
              }
            : saved.menu.nativeTray,
          trayPanel: patch.trayPanel
            ? {
                order: patch.trayPanel.order ?? saved.menu.trayPanel.order,
                hidden: patch.trayPanel.hidden ?? saved.menu.trayPanel.hidden,
              }
            : saved.menu.trayPanel,
        },
      };
      view.rerender(<MenuTab settings={saved} copy={settingsCopy("en-US")} />);
      return saved;
    });
    const trayMenu = screen.getByRole("group", { name: "Tray menu" });
    const about = within(trayMenu).getByRole("checkbox", { name: "About" });

    fireEvent.click(about);
    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith({ nativeTray: { hidden: ["about"] } }),
    );

    fireEvent.click(about);
    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith({ nativeTray: { hidden: [] } }),
    );
  });

  it("restores defaults for one surface without touching the other", async () => {
    apply.mockResolvedValue(defaultAppSettings);
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);
    const panel = screen.getByRole("group", { name: "Panel quick actions" });

    fireEvent.click(within(panel).getByRole("button", { name: "Restore defaults" }));

    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith({
        trayPanel: {
          order: ["refresh", "open_usage", "settings", "dismiss", "quit"],
          hidden: [],
        },
      }),
    );
  });

  it("shows a localized save failure without leaking the raw error", async () => {
    apply.mockRejectedValue(new Error("raw persistence detail"));
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);

    fireEvent.click(
      within(screen.getByRole("group", { name: "Tray menu" })).getByRole("button", {
        name: "Restore defaults",
      }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Menu settings could not be saved. Try again.",
    );
    expect(screen.queryByText("raw persistence detail")).not.toBeInTheDocument();
  });

  it("localizes every editor control in Simplified Chinese", () => {
    apply.mockResolvedValue(defaultAppSettings);
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("zh-CN")} />);

    expect(screen.getByRole("heading", { name: "菜单" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "托盘菜单" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "面板快捷操作" })).toBeInTheDocument();
    expect(
      screen.getByText("设置与退出为必选项，无法隐藏。"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("仅可配置内置项，不支持自定义命令、脚本、网址或可执行文件。"),
    ).toBeInTheDocument();
  });

  it("restores the exact default native order exported by the registry", () => {
    expect(NATIVE_TRAY_ORDER).toEqual([
      "open_panel",
      "refresh",
      "accounts",
      "open_usage",
      "settings",
      "about",
      "quit",
    ]);
  });
});
