import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { currentCliProfile } from "../../test/profileUsageFixtures";
import { trayCopy } from "./copy";
import TrayHeader from "./TrayHeader";

describe("TrayHeader", () => {
  it("shows the account email and hides the panel from the close button", () => {
    const onDismiss = vi.fn();

    render(
      <TrayHeader
        productName="codex-barbar"
        version="1.0.0"
        profile={
          currentCliProfile({
            label: "Current CLI",
            accountDisplayName: null,
            accountEmail: "user@example.com",
            accountStatus: "signedIn",
          })
        }
        copy={trayCopy("en-US")}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByText("user@example.com")).toBeInTheDocument();
    const close = screen.getByRole("button", { name: /hide panel|隐藏面板/i });
    fireEvent.click(close);
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
