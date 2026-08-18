import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { invokeMock } from "../../../test/setup";
import AboutTab from "./AboutTab";
import { settingsCopy } from "../settingsCopy";

describe("AboutTab", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("maps update-check failures to Chinese friendly copy", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "check_for_updates") throw new Error("RAW_UPDATE_ERROR");
      return undefined;
    });
    render(<AboutTab copy={settingsCopy("zh-CN")} />);
    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
    expect(await screen.findByText("暂时无法检查更新。", { exact: false })).toBeInTheDocument();
    expect(screen.queryByText(/RAW_UPDATE_ERROR/)).not.toBeInTheDocument();
  });

  it("renders a Chinese available-update result", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "check_for_updates") {
        return { status: "available", currentVersion: "1.0.0", latestVersion: "v2.0.0" };
      }
      return undefined;
    });
    render(<AboutTab copy={settingsCopy("zh-CN")} />);
    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
    expect(await screen.findByText("有可用更新：v2.0.0")).toBeInTheDocument();
  });

  it.each([
    ["current", "当前已是最新版本。"],
    ["releaseFeedUnavailable", "暂时无法获取发布信息。"],
  ] as const)("renders Chinese %s update result", async (status, expected) => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "check_for_updates") return { status, currentVersion: "1.0.0" };
      return undefined;
    });
    render(<AboutTab copy={settingsCopy("zh-CN")} />);
    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
    expect(await screen.findByText(expected)).toBeInTheDocument();
  });

  it("checks manually and shows a newer public release", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "check_for_updates") {
        return {
          status: "available",
          currentVersion: "1.0.0",
          latestVersion: "v0.1.1",
        };
      }
      return undefined;
    });

    render(<AboutTab />);
    fireEvent.click(screen.getByRole("button", { name: /check for updates/i }));

    expect(await screen.findByText(/v0\.1\.1/)).toBeInTheDocument();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("check_for_updates");
    });
  });

  it("opens the fixed releases page without a download control", async () => {
    render(<AboutTab />);
    fireEvent.click(screen.getByRole("button", { name: /open releases/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_release_page");
    });
    expect(
      screen.queryByRole("button", { name: /download/i }),
    ).not.toBeInTheDocument();
  });

  it("reports an unavailable release feed", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "check_for_updates") {
        return {
          status: "releaseFeedUnavailable",
          currentVersion: "1.0.0",
        };
      }
      return undefined;
    });

    render(<AboutTab />);
    fireEvent.click(screen.getByRole("button", { name: /check for updates/i }));

    expect(
      await screen.findByText(/release feed is unavailable/i),
    ).toBeInTheDocument();
  });
});
