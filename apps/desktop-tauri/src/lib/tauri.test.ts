import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));

import { commands, getPlatformCapabilities } from "./tauri";

describe("platform capability bridge", () => {
  it("invokes the typed platform capability command", async () => {
    mocks.invoke.mockResolvedValue({ platform: "linux", taskbarStatus: false });
    await getPlatformCapabilities();
    expect(commands.getPlatformCapabilities).toBe("get_platform_capabilities");
    expect(mocks.invoke).toHaveBeenCalledWith("get_platform_capabilities");
  });
});
