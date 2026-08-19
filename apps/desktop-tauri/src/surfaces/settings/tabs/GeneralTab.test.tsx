import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GeneralTab from "./GeneralTab";
import { defaultAppSettings } from "../../../hooks/useSettings";

describe("GeneralTab status surfaces", () => {
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

  it("updates taskbar and floating status opacity independently", () => {
    const update = vi.fn().mockResolvedValue(defaultAppSettings);
    const setSurfaceEnabled = vi.fn().mockResolvedValue(defaultAppSettings);
    render(
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

    const taskbarOpacity = screen.getByRole("slider", {
      name: "Taskbar status opacity",
    });
    const floatBallOpacity = screen.getByRole("slider", {
      name: "Floating status ball opacity",
    });
    expect(taskbarOpacity).toHaveValue("20");
    expect(floatBallOpacity).toHaveValue("60");
    expect(taskbarOpacity).toBeEnabled();
    expect(floatBallOpacity).toBeEnabled();

    fireEvent.change(taskbarOpacity, { target: { value: "35" } });
    expect(update).toHaveBeenLastCalledWith({ taskbarStatusOpacity: 35 });
    fireEvent.change(floatBallOpacity, { target: { value: "5" } });
    expect(update).toHaveBeenLastCalledWith({ floatBallOpacity: 5 });
    const glow = screen.getByRole("slider", { name: "Floating status ball glow" });
    expect(glow).toHaveValue("20");
    fireEvent.change(glow, { target: { value: "70" } });
    expect(update).toHaveBeenLastCalledWith({ floatBallGlow: 70 });

    fireEvent.click(screen.getByRole("checkbox", { name: "Show status in taskbar" }));
    expect(setSurfaceEnabled).toHaveBeenCalledWith("taskbarStatus", true);
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
