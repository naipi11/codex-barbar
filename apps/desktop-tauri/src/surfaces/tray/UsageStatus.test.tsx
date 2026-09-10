import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ProfileUsageStateDto } from "../../types/bridge";
import UsageStatus from "./UsageStatus";
import { trayCopy } from "./copy";

const protocolDiagnosticsState: ProfileUsageStateDto = {
  profileId: "profile-1",
  primary: null,
  secondary: null,
  additionalWindows: [],
  fetchedAt: null,
  currentError: {
    kind: "protocolMismatch",
    userMessageKey: "protocolMismatch",
    action: "exportDiagnostics",
    retryAfter: null,
  },
  freshness: "missing",
  refreshStatus: "idle",
  manualCooldownUntil: null,
  protocolAnomaly: true,
};

describe("UsageStatus", () => {
  it("exposes diagnostics export as an actionable recovery button", () => {
    const onExportDiagnostics = vi.fn();

    render(
      <UsageStatus
        state={protocolDiagnosticsState}
        isSwitching={false}
        copy={trayCopy("en-US")}
        locale="en-US"
        onRefresh={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenUsage={vi.fn()}
        onExportDiagnostics={onExportDiagnostics}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Export diagnostics" }));
    expect(onExportDiagnostics).toHaveBeenCalledOnce();
  });
});
