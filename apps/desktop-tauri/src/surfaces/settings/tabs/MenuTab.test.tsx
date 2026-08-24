import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { defaultAppSettings } from "../../../hooks/useSettings";
import { invokeMock } from "../../../test/setup";
import { settingsCopy } from "../settingsCopy";
import MenuTab from "./MenuTab";

describe("MenuTab", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(defaultAppSettings);
  });

  it("owns panel presentation and actions without exposing a native-tray editor", () => {
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);

    expect(screen.getByRole("heading", { name: "Panel" })).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Tray menu" })).not.toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Panel layout" })).toBeInTheDocument();
    const actions = screen.getByRole("group", { name: "Quick actions" });
    const refresh = within(actions).getByRole("checkbox", { name: "Refresh" });
    expect(refresh).toBeChecked();
    expect(refresh).toBeDisabled();
    expect(within(actions).getAllByRole("checkbox")[0]).toBe(refresh);
    expect(screen.getByText(/refresh always stays first/i)).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("patches density and optional detail lines through update_settings", async () => {
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Panel density" }), {
      target: { value: "standard" },
    });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_settings", {
        patch: { panel: { density: "standard" } },
      }),
    );

    fireEvent.click(screen.getByRole("checkbox", { name: "Show data freshness" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_settings", {
        patch: { panel: { showFreshness: false } },
      }),
    );
  });

  it("hides an eligible action while keeping Refresh fixed", async () => {
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);
    const actions = screen.getByRole("group", { name: "Quick actions" });

    fireEvent.click(within(actions).getByRole("checkbox", { name: "Settings" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_settings", {
        patch: { panel: { actions: { hidden: ["settings"] } } },
      }),
    );
    expect(within(actions).getByRole("checkbox", { name: "Refresh" })).toBeDisabled();
  });

  it("reorders eligible actions without moving Refresh", async () => {
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);
    const actions = screen.getByRole("group", { name: "Quick actions" });

    fireEvent.click(
      within(actions).getByRole("button", { name: "Move down Usage & Spend" }),
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_settings", {
        patch: {
          panel: {
            actions: {
              order: ["refresh", "settings", "open_usage", "dismiss", "quit"],
            },
          },
        },
      }),
    );
  });

  it("restores the documented panel layout", async () => {
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);

    fireEvent.click(screen.getByRole("button", { name: "Restore panel layout" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_settings", {
        patch: {
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
        },
      }),
    );
  });

  it("shows a localized save failure without leaking the raw error", async () => {
    invokeMock.mockRejectedValue(new Error("raw persistence detail"));
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("en-US")} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Panel density" }), {
      target: { value: "standard" },
    });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Panel settings could not be saved. Try again.",
    );
    expect(screen.queryByText("raw persistence detail")).not.toBeInTheDocument();
  });

  it("localizes the panel controls in Simplified Chinese", () => {
    render(<MenuTab settings={defaultAppSettings} copy={settingsCopy("zh-CN")} />);

    expect(screen.getByRole("heading", { name: "面板" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "面板布局" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "快捷操作" })).toBeInTheDocument();
    expect(screen.getByText(/刷新始终位于首位/)).toBeInTheDocument();
    expect(screen.queryByText("托盘菜单")).not.toBeInTheDocument();
  });
});
