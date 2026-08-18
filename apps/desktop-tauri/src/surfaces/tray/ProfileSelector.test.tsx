import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { currentCliProfile, managedProfile } from "../../test/profileUsageFixtures";
import { trayCopy } from "./copy";
import ProfileSelector from "./ProfileSelector";

describe("ProfileSelector", () => {
  it("shows the concrete current account name without the Current CLI label", () => {
    render(
      <ProfileSelector
        profiles={[
          currentCliProfile({
            label: "Current CLI",
            accountDisplayName: "Ming Zhao",
          }),
          managedProfile(),
        ]}
        selectedProfileId="personal"
        copy={trayCopy("en-US")}
        onSelect={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /profile/i }));
    const option = screen.getByRole("option", { name: /Ming Zhao/i });
    expect(option).toHaveTextContent("Ming Zhao");
    expect(option).not.toHaveTextContent("Current CLI");
  });

  it("uses a signed-out label when the current account has no identity", () => {
    render(
      <ProfileSelector
        profiles={[currentCliProfile({ label: "Current CLI" })]}
        selectedProfileId="personal"
        copy={trayCopy("en-US")}
        onSelect={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /profile/i }));
    const option = screen.getByRole("option");
    expect(option).toHaveTextContent("Signed out");
    expect(option).not.toHaveTextContent("Current CLI");
  });

  it("opens an accessible listbox, selects a profile, and restores trigger focus", () => {
    const onSelect = vi.fn();
    render(
      <ProfileSelector
        profiles={[currentCliProfile(), managedProfile()]}
        selectedProfileId="personal"
        copy={trayCopy("en-US")}
        onSelect={onSelect}
      />,
    );

    const trigger = screen.getByRole("button", { name: /profile/i });
    fireEvent.click(trigger);
    const listbox = screen.getByRole("listbox");
    expect(listbox).toBeVisible();

    fireEvent.click(screen.getByRole("option", { name: /Work/i }));
    expect(onSelect).toHaveBeenCalledWith("work");
    expect(trigger).toHaveFocus();
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});
