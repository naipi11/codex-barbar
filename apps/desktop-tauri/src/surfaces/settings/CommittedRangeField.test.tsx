import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CommittedRangeField } from "./CommittedRangeField";

describe("CommittedRangeField", () => {
  it("commits only the final drag value", async () => {
    const onCommit = vi.fn(async (value: number) => value);
    render(
      <CommittedRangeField
        id="glow"
        label="Glow"
        value={0}
        min={0}
        max={100}
        tickValues={[0, 25, 50, 75, 100]}
        valueText={(value) => String(value)}
        onCommit={onCommit}
      />,
    );
    const input = screen.getByLabelText("Glow");

    fireEvent.input(input, { target: { value: "20" } });
    fireEvent.input(input, { target: { value: "70" } });

    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.pointerUp(input);
    await waitFor(() => expect(onCommit).toHaveBeenCalledWith(70));
  });
});
