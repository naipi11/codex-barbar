import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { trayCopy } from "./copy";
import TrayActions from "./TrayActions";

function handlers() {
  return {
    onRefresh: vi.fn(),
    onOpenUsage: vi.fn(),
    onOpenSettings: vi.fn(),
    onDismiss: vi.fn(),
    onQuit: vi.fn(),
  };
}

describe("TrayActions", () => {
  it("renders only configured actions in the configured order", () => {
    const callbacks = handlers();
    render(
      <TrayActions
        copy={trayCopy("en-US")}
        order={["open_usage", "refresh", "quit"]}
        {...callbacks}
      />,
    );

    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Usage",
      "Refresh",
      "Quit",
    ]);
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("binds each visible action to its existing callback", () => {
    const callbacks = handlers();
    render(<TrayActions copy={trayCopy("en-US")} {...callbacks} />);

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    fireEvent.click(screen.getByRole("button", { name: "Usage" }));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    fireEvent.click(screen.getByRole("button", { name: "Quit" }));

    expect(callbacks.onRefresh).toHaveBeenCalledTimes(1);
    expect(callbacks.onOpenUsage).toHaveBeenCalledTimes(1);
    expect(callbacks.onOpenSettings).toHaveBeenCalledTimes(1);
    expect(callbacks.onDismiss).toHaveBeenCalledTimes(1);
    expect(callbacks.onQuit).toHaveBeenCalledTimes(1);
  });

  it("ignores unknown ids and falls back to the default order when omitted", () => {
    const callbacks = handlers();
    const { rerender } = render(
      <TrayActions copy={trayCopy("en-US")} order={["bogus", "quit"]} {...callbacks} />,
    );

    expect(screen.getByRole("button", { name: "Quit" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Refresh" })).not.toBeInTheDocument();

    rerender(<TrayActions copy={trayCopy("en-US")} {...callbacks} />);
    expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quit" })).toBeInTheDocument();
  });

  it("keeps the first visible actionable button focused when requested", () => {
    const callbacks = handlers();
    render(
      <TrayActions
        copy={trayCopy("en-US")}
        order={["dismiss", "refresh"]}
        autoFocusRefresh
        {...callbacks}
      />,
    );

    expect(screen.getByRole("button", { name: "Refresh" })).toHaveFocus();
  });
});

