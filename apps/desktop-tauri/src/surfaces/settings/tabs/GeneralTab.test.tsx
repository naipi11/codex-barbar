import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import GeneralTab from "./GeneralTab";
import { defaultAppSettings } from "../../../hooks/useSettings";

describe("GeneralTab status surfaces", () => {
  afterEach(() => vi.restoreAllMocks());

  it("reflects both opt-in surface settings", () => {
    render(
      <GeneralTab
        settings={{
          ...defaultAppSettings,
          taskbarStatusEnabled: true,
          floatBallEnabled: false,
        }}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: "Show status in taskbar" }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "Show floating status ball" }),
    ).not.toBeChecked();
  });

  it("routes surface changes through the typed bridge", () => {
    const update = vi.fn().mockResolvedValue(defaultAppSettings);
    const setSurfaceEnabled = vi.fn().mockResolvedValue(defaultAppSettings);
    render(
      <GeneralTab
        settings={defaultAppSettings}
        update={update}
        setSurfaceEnabled={setSurfaceEnabled}
      />,
    );

    fireEvent.click(
      screen.getByRole("checkbox", { name: "Show status in taskbar" }),
    );
    fireEvent.click(
      screen.getByRole("checkbox", { name: "Show floating status ball" }),
    );

    expect(setSurfaceEnabled).toHaveBeenNthCalledWith(
      1,
      "taskbarStatus",
      true,
    );
    expect(setSurfaceEnabled).toHaveBeenNthCalledWith(2, "floatBall", false);
    expect(update).not.toHaveBeenCalled();
  });

  it("previews transparency input frames locally and persists once on pointer release", () => {
    const update = vi.fn().mockResolvedValue(defaultAppSettings);
    const setSurfaceEnabled = vi.fn().mockResolvedValue(defaultAppSettings);
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
    const view = render(
      <GeneralTab
        settings={{
          ...defaultAppSettings,
          taskbarStatusEnabled: false,
          floatBallEnabled: false,
          taskbarStatusOpacity: 20,
          floatBallOpacity: 60,
        }}
        update={update}
        setSurfaceEnabled={setSurfaceEnabled}
      />,
    );

    const taskbarTransparency = screen.getByRole("slider", {
      name: "Taskbar status transparency",
    });
    const floatBallTransparency = screen.getByRole("slider", {
      name: "Floating status ball transparency",
    });
    expect(taskbarTransparency).toHaveValue("20");
    expect(floatBallTransparency).toHaveValue("60");
    expect(taskbarTransparency).toBeEnabled();
    expect(floatBallTransparency).toBeEnabled();

    fireEvent.pointerDown(taskbarTransparency, { pointerId: 4 });
    for (let value = 21; value <= 30; value += 1) {
      fireEvent.input(taskbarTransparency, { target: { value: String(value) } });
    }
    expect(update).not.toHaveBeenCalled();
    act(() => frames.splice(0).forEach((callback) => callback(0)));
    expect(taskbarTransparency).toHaveValue("30");

    view.rerender(
      <GeneralTab
        settings={{
          ...defaultAppSettings,
          taskbarStatusOpacity: 25,
          floatBallOpacity: 60,
        }}
        update={update}
        setSurfaceEnabled={setSurfaceEnabled}
      />,
    );
    expect(taskbarTransparency).toHaveValue("30");
    fireEvent.pointerUp(taskbarTransparency, { pointerId: 4 });
    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenLastCalledWith({ taskbarStatusOpacity: 30 });

    fireEvent.pointerDown(floatBallTransparency, { pointerId: 5 });
    fireEvent.input(floatBallTransparency, { target: { value: "5" } });
    fireEvent.pointerCancel(floatBallTransparency, { pointerId: 5 });
    expect(floatBallTransparency).toHaveValue("60");
    expect(update).toHaveBeenCalledTimes(1);

    const glow = screen.getByRole("slider", { name: "Floating status ball glow" });
    expect(glow).toHaveValue("20");
    fireEvent.change(glow, { target: { value: "70" } });
    expect(update).toHaveBeenLastCalledWith({ floatBallGlow: 70 });

    fireEvent.click(screen.getByRole("checkbox", { name: "Show status in taskbar" }));
    expect(setSurfaceEnabled).toHaveBeenCalledWith("taskbarStatus", true);
  });

  it("commits keyboard and blur transparency edits once per interaction", () => {
    const update = vi.fn().mockResolvedValue(defaultAppSettings);
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
    render(
      <GeneralTab
        settings={defaultAppSettings}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const taskbar = screen.getByRole("slider", { name: "Taskbar status transparency" });
    fireEvent.keyDown(taskbar, { key: "ArrowRight" });
    fireEvent.input(taskbar, { target: { value: "21" } });
    act(() => frames.splice(0).forEach((callback) => callback(0)));
    fireEvent.keyUp(taskbar, { key: "ArrowRight" });
    fireEvent.blur(taskbar);
    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenLastCalledWith({ taskbarStatusOpacity: 21 });

    const floatBall = screen.getByRole("slider", { name: "Floating status ball transparency" });
    fireEvent.input(floatBall, { target: { value: "40" } });
    fireEvent.blur(floatBall);
    expect(update).toHaveBeenCalledTimes(2);
    expect(update).toHaveBeenLastCalledWith({ floatBallOpacity: 40 });
  });

  it("shows a localized inline save error and clears it after a later successful commit", async () => {
    const update = vi
      .fn()
      .mockRejectedValueOnce(new Error("raw persistence detail"))
      .mockResolvedValue(defaultAppSettings);
    render(
      <GeneralTab
        settings={{ ...defaultAppSettings, taskbarStatusOpacity: 20 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const range = screen.getByRole("slider", {
      name: "Taskbar status transparency",
    });

    fireEvent.pointerDown(range, { pointerId: 15 });
    fireEvent.input(range, { target: { value: "30" } });
    fireEvent.pointerUp(range, { pointerId: 15 });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Transparency could not be saved. Try again.",
    );
    expect(range).toHaveValue("20");
    expect(screen.queryByText("raw persistence detail")).not.toBeInTheDocument();

    fireEvent.pointerDown(range, { pointerId: 16 });
    fireEvent.input(range, { target: { value: "35" } });
    fireEvent.pointerUp(range, { pointerId: 16 });

    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });
});

  it("lists the five built-in skins plus a custom editor", () => {
    render(
      <GeneralTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );

    const theme = screen.getByLabelText("Theme");
    expect(theme).toHaveValue("system");
    fireEvent.change(theme, { target: { value: "custom" } });
    expect(screen.getByRole("group", { name: "Custom skin" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apply custom skin" }));
  });
