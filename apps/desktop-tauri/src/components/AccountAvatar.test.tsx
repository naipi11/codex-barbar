import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { currentCliProfile } from "../test/profileUsageFixtures";
import AccountAvatar from "./AccountAvatar";

describe("AccountAvatar", () => {
  it("renders the product mark for the default identity", () => {
    const { container } = render(
      <AccountAvatar identity={currentCliProfile()} size={28} decorative />,
    );

    expect(container.querySelector("svg")).not.toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.firstElementChild).toHaveAttribute("data-avatar-kind", "default");
  });

  it.each(["official", "manual"] as const)(
    "renders the generated local URI for a %s avatar",
    (avatarKind) => {
      render(
        <AccountAvatar
          identity={currentCliProfile({
            presentationName: "stack",
            avatarKind,
            avatarAssetUri: "account-avatar://profile/a?rev=1",
          })}
          size={32}
          decorative={false}
        />,
      );

      const image = screen.getByRole("img", { name: "stack" });
      expect(image).toHaveAttribute("src", "account-avatar://profile/a?rev=1");
      expect(image.closest("[data-avatar-kind]")).toHaveAttribute(
        "data-avatar-kind",
        avatarKind,
      );
    },
  );

  it("falls back to the product mark when the local asset cannot render", () => {
    const { container } = render(
      <AccountAvatar
        identity={currentCliProfile({
          presentationName: "stack",
          avatarKind: "official",
          avatarAssetUri: "account-avatar://profile/a?rev=1",
        })}
        size={24}
        decorative
      />,
    );

    fireEvent.error(container.querySelector("img")!);

    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("svg")).not.toBeNull();
    expect(container.firstElementChild).toHaveAttribute("data-avatar-kind", "default");
  });
});
