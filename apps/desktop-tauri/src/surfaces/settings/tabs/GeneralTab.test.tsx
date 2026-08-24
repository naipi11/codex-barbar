import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { defaultAppSettings } from "../../../hooks/useSettings";
import GeneralTab from "./GeneralTab";

describe("GeneralTab", () => {
  it("keeps startup, refresh, theme, and language while moving all float controls out", () => {
    render(
      <GeneralTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );

    expect(screen.getByRole("checkbox", { name: "Start at login" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Refresh interval" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Theme" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Language" })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: /floating status ball/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("slider", { name: /floating status ball/i })).not.toBeInTheDocument();
  });

  it("commits custom skin radius on the interaction boundary", async () => {
    render(
      <GeneralTab
        settings={defaultAppSettings}
        update={vi.fn().mockResolvedValue(defaultAppSettings)}
        setSurfaceEnabled={vi.fn().mockResolvedValue(defaultAppSettings)}
      />,
    );
    fireEvent.change(screen.getByLabelText("Theme"), { target: { value: "custom" } });
    const radius = screen.getByRole("slider", { name: "Corner radius" });
    fireEvent.input(radius, { target: { value: "20" } });
    fireEvent.input(radius, { target: { value: "24" } });
    fireEvent.pointerUp(radius);
    await waitFor(() => expect(radius).toHaveValue("24"));
  });
});
