import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { invokeMock } from "../../../test/setup";
import { defaultSettings } from "../../../test/profileUsageFixtures";
import AdvancedTab from "./AdvancedTab";
import { settingsCopy } from "../settingsCopy";

describe("AdvancedTab", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("maps validation and export failures to Chinese friendly copy", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "validate_codex_executable") throw new Error("RAW_VALIDATION_ERROR");
      if (command === "export_diagnostics") throw new Error("RAW_EXPORT_ERROR");
      return undefined;
    });
    render(<AdvancedTab settings={defaultSettings} copy={settingsCopy("zh-CN")} />);
    fireEvent.click(screen.getByRole("button", { name: "验证并保存" }));
    expect(await screen.findByText("无法验证 Codex 可执行文件。", { exact: false })).toBeInTheDocument();
    expect(screen.queryByText(/RAW_VALIDATION_ERROR/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "导出诊断信息" }));
    expect(await screen.findByText("无法导出诊断信息。", { exact: false })).toBeInTheDocument();
    expect(screen.queryByText(/RAW_EXPORT_ERROR/)).not.toBeInTheDocument();
  });

  it("renders Chinese validation and export successes", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "validate_codex_executable") {
        return { status: "compatible", version: "1.2.3" };
      }
      if (command === "export_diagnostics") return { path: "C:\\诊断\\report.json" };
      return undefined;
    });
    render(<AdvancedTab settings={defaultSettings} copy={settingsCopy("zh-CN")} />);
    fireEvent.click(screen.getByRole("button", { name: "验证并保存" }));
    expect(await screen.findByText("兼容 (1.2.3)。")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "导出诊断信息" }));
    expect(await screen.findByText("诊断信息已导出到 C:\\诊断\\report.json")).toBeInTheDocument();
  });

  it("exports redacted diagnostics to a fixed-location path", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "export_diagnostics") {
        return {
          path: "%LOCALAPPDATA%\\codex-barbar\\diagnostics\\codex-barbar-diagnostics-test.json",
        };
      }
      return undefined;
    });

    render(<AdvancedTab settings={defaultSettings} />);
    fireEvent.click(
      screen.getByRole("button", { name: /export diagnostics/i }),
    );

    expect(
      await screen.findByText(/codex-barbar-diagnostics-test\.json/),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_diagnostics");
    });
  });
});
