import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../../../hooks/useSettings";
import { settingsCopy } from "../settingsCopy";
import TaskbarTrayTab from "./TaskbarTrayTab";

function pending<T>(): Promise<T> {
  return new Promise(() => undefined);
}

describe("TaskbarTrayTab", () => {
  afterEach(() => vi.restoreAllMocks());

  it("renders compact accessible groups and localizes their controls", () => {
    const { rerender } = render(
      <TaskbarTrayTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
        copy={settingsCopy("en-US")}
      />,
    );

    expect(screen.getByRole("heading", { name: "Taskbar & Tray" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Taskbar status" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Tray icon and tooltip" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Full-screen behavior" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Transparency" })).toHaveAttribute(
      "aria-valuetext",
      "20% transparent",
    );
    expect(screen.getByText("0% is most opaque; 80% is most transparent.")).toBeInTheDocument();
    expect(screen.queryByText(/account name length/i)).not.toBeInTheDocument();

    rerender(
      <TaskbarTrayTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
        copy={settingsCopy("zh-CN")}
      />,
    );

    expect(screen.getByRole("heading", { name: "任务栏与托盘" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "任务栏状态" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "托盘图标与工具提示" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "全屏行为" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "透明度" })).toHaveAttribute(
      "aria-valuetext",
      "20% 透明",
    );
    expect(screen.getByText("0% 最不透明，80% 最透明。")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "显示账户名称" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "密度" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "托盘图标样式" })).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "全屏应用运行时隐藏状态界面" }),
    ).toBeInTheDocument();
  });

  it("routes taskbar visibility and every preference through the correct bridge shape", () => {
    const update = vi.fn().mockResolvedValue(defaultAppSettings);
    const setSurfaceEnabled = vi.fn().mockResolvedValue(defaultAppSettings);
    render(
      <TaskbarTrayTab
        settings={{ ...defaultAppSettings, taskbarStatusEnabled: true }}
        update={update}
        setSurfaceEnabled={setSurfaceEnabled}
      />,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: "Show taskbar status" }));
    expect(setSurfaceEnabled).toHaveBeenCalledWith("taskbarStatus", false);

    const toggles = [
      ["Show product icon", "showTaskbarIcon"],
      ["Show account name", "showTaskbarAccount"],
      ["Show weekly label", "showWeeklyLabel"],
      ["Show remaining percentage", "showWeeklyPercent"],
      ["Show reset date", "showResetDate"],
      ["Account name", "tooltipAccount"],
      ["Weekly remaining", "tooltipWeekly"],
      ["Reset date", "tooltipResetDate"],
      ["Last update time", "tooltipUpdatedAt"],
      ["Hide status surfaces during full-screen apps", "hideStatusSurfacesInFullscreen"],
    ] as const;

    for (const [label, field] of toggles) {
      fireEvent.click(screen.getByRole("checkbox", { name: label }));
      expect(update).toHaveBeenLastCalledWith({ taskbarTray: { [field]: false } });
    }

    fireEvent.change(screen.getByRole("combobox", { name: "Density" }), {
      target: { value: "standard" },
    });
    expect(update).toHaveBeenLastCalledWith({ taskbarTray: { density: "standard" } });

    fireEvent.change(screen.getByRole("combobox", { name: "Tray icon style" }), {
      target: { value: "monochrome" },
    });
    expect(update).toHaveBeenLastCalledWith({
      taskbarTray: { trayIconMode: "monochrome" },
    });
  });

  it("keeps one taskbar element enabled while the overlay is visible", () => {
    render(
      <TaskbarTrayTab
        settings={{
          ...defaultAppSettings,
          taskbarStatusEnabled: true,
          taskbarTray: {
            ...defaultAppSettings.taskbarTray,
            showTaskbarIcon: false,
            showTaskbarAccount: false,
            showWeeklyLabel: false,
            showWeeklyPercent: true,
            showResetDate: false,
          },
        }}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: "Show remaining percentage" }),
    ).toBeDisabled();
    expect(
      screen.getByText("Keep at least one taskbar item visible while taskbar status is on."),
    ).toBeInTheDocument();
  });

  it("allows an empty hidden layout but prevents enabling it", () => {
    render(
      <TaskbarTrayTab
        settings={{
          ...defaultAppSettings,
          taskbarStatusEnabled: false,
          taskbarTray: {
            ...defaultAppSettings.taskbarTray,
            showTaskbarIcon: false,
            showTaskbarAccount: false,
            showWeeklyLabel: false,
            showWeeklyPercent: false,
            showResetDate: false,
          },
        }}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );

    expect(screen.getByRole("checkbox", { name: "Show taskbar status" })).toBeDisabled();
    expect(
      screen.getByRole("checkbox", { name: "Show remaining percentage" }),
    ).toBeEnabled();
  });

  it("previews transparency locally, ignores active-drag echoes, and persists once", () => {
    const update = vi.fn(() => pending<typeof defaultAppSettings>());
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
    const view = render(
      <TaskbarTrayTab
        settings={{ ...defaultAppSettings, taskbarStatusOpacity: 20 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const transparency = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(transparency, { pointerId: 4 });
    for (let value = 21; value <= 30; value += 1) {
      fireEvent.input(transparency, { target: { value: String(value) } });
    }
    expect(update).not.toHaveBeenCalled();
    act(() => frames.splice(0).forEach((callback) => callback(0)));
    expect(transparency).toHaveValue("30");

    view.rerender(
      <TaskbarTrayTab
        settings={{ ...defaultAppSettings, taskbarStatusOpacity: 25 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    expect(transparency).toHaveValue("30");

    fireEvent.pointerUp(transparency, { pointerId: 4 });
    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledWith({ taskbarStatusOpacity: 30 });
  });

  it("commits keyboard and blur transparency edits once per interaction", () => {
    const update = vi.fn(() => pending<typeof defaultAppSettings>());
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
    render(
      <TaskbarTrayTab
        settings={defaultAppSettings}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const transparency = screen.getByRole("slider", { name: "Transparency" });
    transparency.focus();
    expect(transparency).toHaveFocus();

    fireEvent.keyDown(transparency, { key: "ArrowRight" });
    fireEvent.input(transparency, { target: { value: "21" } });
    act(() => frames.splice(0).forEach((callback) => callback(0)));
    fireEvent.keyUp(transparency, { key: "ArrowRight" });
    fireEvent.blur(transparency);

    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledWith({ taskbarStatusOpacity: 21 });
  });

  it("rolls back a rejected transparency save and clears the error after acknowledgement", async () => {
    const update = vi
      .fn()
      .mockRejectedValueOnce(new Error("raw persistence detail"))
      .mockResolvedValue({ ...defaultAppSettings, taskbarStatusOpacity: 35 });
    render(
      <TaskbarTrayTab
        settings={{ ...defaultAppSettings, taskbarStatusOpacity: 20 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const transparency = screen.getByRole("slider", { name: "Transparency" });

    fireEvent.pointerDown(transparency, { pointerId: 15 });
    fireEvent.input(transparency, { target: { value: "30" } });
    fireEvent.pointerUp(transparency, { pointerId: 15 });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Transparency could not be saved. Try again.",
    );
    expect(transparency).toHaveValue("20");
    expect(screen.queryByText("raw persistence detail")).not.toBeInTheDocument();

    fireEvent.pointerDown(transparency, { pointerId: 16 });
    fireEvent.input(transparency, { target: { value: "35" } });
    fireEvent.pointerUp(transparency, { pointerId: 16 });

    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(transparency).toHaveValue("35");
  });
});
