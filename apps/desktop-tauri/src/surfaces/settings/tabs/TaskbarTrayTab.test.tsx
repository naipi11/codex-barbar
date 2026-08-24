import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../../../hooks/useSettings";
import type { SettingsPatchDto } from "../../../types/bridge";
import { settingsCopy } from "../settingsCopy";
import TaskbarTrayTab from "./TaskbarTrayTab";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("TaskbarTrayTab", () => {
  it("owns taskbar and floating-ball controls without tray preferences", () => {
    const { rerender } = render(
      <TaskbarTrayTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );

    expect(screen.getByRole("heading", { name: "Taskbar & Float Ball" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Taskbar status" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Floating status ball" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Transparency" })).toHaveAttribute("max", "100");
    expect(screen.getByRole("slider", { name: "Floating status ball transparency" })).toHaveAttribute("max", "100");
    expect(screen.getByRole("slider", { name: "Floating status ball glow" })).toHaveAttribute("max", "100");
    expect(screen.queryByRole("combobox", { name: /tray icon/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/tooltip rows/i)).not.toBeInTheDocument();

    rerender(
      <TaskbarTrayTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
        copy={settingsCopy("zh-CN")}
      />,
    );
    expect(screen.getByRole("heading", { name: "任务栏与悬浮球" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "悬浮状态球" })).toBeInTheDocument();
  });

  it("uses canonical patches for taskbar preferences and each committed range", async () => {
    let saved = defaultAppSettings;
    const update = vi.fn(async (patch: SettingsPatchDto) => {
      saved = {
        ...saved,
        ...patch,
        taskbarPresentation: { ...saved.taskbarPresentation, ...patch.taskbarPresentation },
      } as typeof defaultAppSettings;
      return saved;
    });
    const setSurfaceEnabled = vi.fn().mockResolvedValue(defaultAppSettings);
    render(<TaskbarTrayTab settings={saved} update={update} setSurfaceEnabled={setSurfaceEnabled} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Density" }), { target: { value: "standard" } });
    expect(update).toHaveBeenLastCalledWith({ taskbarPresentation: { density: "standard" } });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Density" })).toBeEnabled());

    fireEvent.click(screen.getByRole("checkbox", { name: "Show floating status ball" }));
    expect(setSurfaceEnabled).toHaveBeenCalledWith("floatBall", false);

    const transparency = screen.getByRole("slider", { name: "Transparency" });
    fireEvent.input(transparency, { target: { value: "20" } });
    fireEvent.input(transparency, { target: { value: "70" } });
    expect(update).toHaveBeenCalledTimes(1);
    fireEvent.pointerUp(transparency);
    await waitFor(() => expect(update).toHaveBeenLastCalledWith({ taskbarTransparencyPercent: 70 }));

    const glow = screen.getByRole("slider", { name: "Floating status ball glow" });
    fireEvent.input(glow, { target: { value: "55" } });
    fireEvent.blur(glow);
    await waitFor(() => expect(update).toHaveBeenLastCalledWith({ floatBallGlowPercent: 55 }));
  });

  it("keeps an active thumb stable and restores only its rejected range", async () => {
    const save = deferred<typeof defaultAppSettings>();
    const update = vi.fn(() => save.promise);
    const view = render(
      <TaskbarTrayTab
        settings={{ ...defaultAppSettings, taskbarTransparencyPercent: 20 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const transparency = screen.getByRole("slider", { name: "Transparency" });
    const glow = screen.getByRole("slider", { name: "Floating status ball glow" });

    fireEvent.pointerDown(transparency);
    fireEvent.input(transparency, { target: { value: "70" } });
    await waitFor(() => expect(transparency).toHaveValue("70"));
    view.rerender(
      <TaskbarTrayTab
        settings={{ ...defaultAppSettings, taskbarTransparencyPercent: 25, floatBallGlowPercent: 40 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    expect(transparency).toHaveValue("70");
    expect(glow).toHaveValue("40");

    fireEvent.pointerUp(transparency);
    await act(async () => {
      save.reject(new Error("save failed"));
      await Promise.resolve();
    });
    expect(transparency).toHaveValue("25");
    expect(glow).toHaveValue("40");
    expect(await screen.findByRole("alert")).toHaveTextContent("Transparency could not be saved. Try again.");
  });
});
