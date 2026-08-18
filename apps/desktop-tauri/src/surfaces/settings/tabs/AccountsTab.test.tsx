import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { currentCliProfile, managedProfile } from "../../../test/profileUsageFixtures";
import AccountsTab from "./AccountsTab";

const baseProps = {
  selectedProfileId: "personal",
  loginState: null,
  onSelect: vi.fn(),
  onRename: vi.fn(async () => undefined),
  onRemove: vi.fn(async () => undefined),
  onStartLogin: vi.fn(),
  onCancelLogin: vi.fn(),
};

describe("AccountsTab", () => {
  it("shows the concrete current Codex identity instead of Current CLI", () => {
    render(
      <AccountsTab
        {...baseProps}
        profiles={[
          currentCliProfile({
            label: "Current CLI",
            accountDisplayName: "Ming Zhao",
            accountEmail: "ming@example.com",
          }),
        ]}
      />,
    );

    expect(screen.getByText("Ming Zhao")).toBeInTheDocument();
    expect(screen.queryByText("Current CLI")).toBeNull();
  });

  it("keeps a managed label while exposing its identity separately", () => {
    render(
      <AccountsTab
        {...baseProps}
        profiles={[
          managedProfile({
            accountDisplayName: null,
            accountEmail: "work@example.com",
          }),
        ]}
      />,
    );

    expect(screen.getByRole("button", { name: /Work/ })).toBeInTheDocument();
    expect(screen.getByText("work@example.com")).toBeInTheDocument();
  });
});
