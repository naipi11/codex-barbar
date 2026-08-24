import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { currentCliProfile } from "../../test/profileUsageFixtures";
import { trayCopy } from "./copy";
import TrayHeader from "./TrayHeader";

describe("TrayHeader", () => {
  it("shows the presentation name and local avatar without exposing email", () => {
    const onDismiss = vi.fn();

    const { container } = render(
      <TrayHeader
        productName="codex-barbar"
        version="1.0.0"
        profile={
          currentCliProfile({
            label: "Current CLI",
            accountDisplayName: "unsafe display",
            accountEmail: "user@example.com",
            accountStatus: "signedIn",
            presentationName: "stack",
            avatarKind: "official",
            avatarAssetUri: "account-avatar://profile/a?rev=1",
          })
        }
        copy={trayCopy("en-US")}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByText("stack")).toBeInTheDocument();
    expect(screen.getByText("Signed in")).toBeInTheDocument();
    expect(screen.queryByText("user@example.com")).not.toBeInTheDocument();
    expect(container.querySelector("img")).toHaveAttribute(
      "src",
      "account-avatar://profile/a?rev=1",
    );
    const close = screen.getByRole("button", { name: /hide panel|隐藏面板/i });
    fireEvent.click(close);
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("omits account status when the panel preference hides it", () => {
    render(
      <TrayHeader
        productName="codex-barbar"
        version="1.0.0"
        profile={currentCliProfile({
          presentationName: "stack",
          accountStatus: "signedIn",
        })}
        copy={trayCopy("en-US")}
        showAccountStatus={false}
        onDismiss={() => undefined}
      />,
    );

    expect(screen.queryByText("已登录")).not.toBeInTheDocument();
  });
});
