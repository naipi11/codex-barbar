import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import GeneralTab from "./GeneralTab";
import { defaultAppSettings } from "../../../hooks/useSettings";

function pending<T>(): Promise<T> {
  return new Promise(() => undefined);
}

describe("GeneralTab status surfaces", () => {
  afterEach(() => vi.restoreAllMocks());

  it("keeps floating-ball controls and removes the taskbar status controls", () => {
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
      screen.getByRole("checkbox", { name: "Show floating status ball" }),
    ).not.toBeChecked();
    expect(screen.queryByRole("heading", { name: "Taskbar status" })).not.toBeInTheDocument();
    expect(screen.queryByRole("slider", { name: "Taskbar status transparency" })).not.toBeInTheDocument();
  });

  it("routes the floating-ball surface change through the typed bridge", () => {
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
      screen.getByRole("checkbox", { name: "Show floating status ball" }),
    );

    expect(setSurfaceEnabled).toHaveBeenCalledWith("floatBall", false);
    expect(update).not.toHaveBeenCalled();
  });

  it("previews floating-ball transparency locally and persists once on pointer release", () => {
    const update = vi.fn(() => pending<typeof defaultAppSettings>());
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

    const floatBallTransparency = screen.getByRole("slider", {
      name: "Floating status ball transparency",
    });
    expect(floatBallTransparency).toHaveValue("60");
    expect(floatBallTransparency).toBeEnabled();

    fireEvent.pointerDown(floatBallTransparency, { pointerId: 5 });
    fireEvent.input(floatBallTransparency, { target: { value: "5" } });
    fireEvent.pointerCancel(floatBallTransparency, { pointerId: 5 });
    expect(floatBallTransparency).toHaveValue("60");
    expect(update).not.toHaveBeenCalled();

    const glow = screen.getByRole("slider", { name: "Floating status ball glow" });
    expect(glow).toHaveValue("20");
    fireEvent.change(glow, { target: { value: "70" } });
    expect(update).toHaveBeenLastCalledWith({ floatBallGlow: 70 });
  });

  it("commits keyboard and blur transparency edits once per interaction", () => {
    const update = vi.fn(() => pending<typeof defaultAppSettings>());
    render(
      <GeneralTab
        settings={defaultAppSettings}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const floatBall = screen.getByRole("slider", { name: "Floating status ball transparency" });
    fireEvent.input(floatBall, { target: { value: "40" } });
    fireEvent.blur(floatBall);
    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenLastCalledWith({ floatBallOpacity: 40 });
  });

  it("shows a localized inline float-ball save error and clears it after a later successful commit", async () => {
    const update = vi
      .fn()
      .mockRejectedValueOnce(new Error("raw persistence detail"))
      .mockResolvedValue({ ...defaultAppSettings, floatBallOpacity: 35 });
    render(
      <GeneralTab
        settings={{ ...defaultAppSettings, floatBallOpacity: 20 }}
        update={update}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    const range = screen.getByRole("slider", {
      name: "Floating status ball transparency",
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
    expect(range).toHaveValue("35");
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
