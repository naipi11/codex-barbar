import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { currentCliProfile, managedProfile } from "../../../test/profileUsageFixtures";
import AccountsTab from "./AccountsTab";

import { clearProfileAvatar, saveProfileAvatar } from "../../../lib/tauri";

vi.mock("../../../lib/tauri", () => ({
  clearProfileAvatar: vi.fn(),
  saveProfileAvatar: vi.fn(),
}));

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
            presentationName: "ming",
          }),
        ]}
      />,
    );

    expect(screen.getByRole("button", { name: /ming.*selected/i })).toBeInTheDocument();
    expect(screen.queryByText("Ming Zhao")).not.toBeInTheDocument();
    expect(screen.queryByText("ming@example.com")).not.toBeInTheDocument();
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
            presentationName: "work-user",
          }),
        ]}
      />,
    );

    expect(screen.getByRole("button", { name: /work-user/ })).toBeInTheDocument();
    expect(screen.getByText("work-user")).toBeInTheDocument();
    expect(screen.queryByText("work@example.com")).not.toBeInTheDocument();
  });

  it("saves a selected PNG avatar and can restore the default", async () => {
    vi.mocked(saveProfileAvatar).mockResolvedValue({
      profiles: [],
      selectedProfileId: "work",
    });
    vi.mocked(clearProfileAvatar).mockResolvedValue({
      profiles: [],
      selectedProfileId: "work",
    });

    render(
      <AccountsTab
        {...baseProps}
        profiles={[managedProfile({ id: "work", presentationName: "work-user" })]}
        selectedProfileId="work"
      />,
    );

    const file = new File([new Uint8Array([137, 80, 78, 71])], "avatar.png", {
      type: "image/png",
    });
    fireEvent.change(screen.getByLabelText("Profile avatar"), {
      target: { files: [file] },
    });

    await waitFor(() =>
      expect(saveProfileAvatar).toHaveBeenCalledWith(
        "work",
        expect.stringMatching(/^data:image\/png;base64,/),
      ),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("Avatar saved");

    fireEvent.click(screen.getByRole("button", { name: "Restore default avatar" }));
    await waitFor(() => expect(clearProfileAvatar).toHaveBeenCalledWith("work"));
    expect(await screen.findByRole("status")).toHaveTextContent("Avatar restored");
  });
});
