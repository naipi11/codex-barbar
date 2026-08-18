import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ManagedLoginDialog from "./ManagedLoginDialog";
import type { ManagedLoginStateDto } from "../../../types/bridge";
import { settingsCopy } from "../settingsCopy";

function loginState(
  overrides: Partial<ManagedLoginStateDto> = {},
): ManagedLoginStateDto {
  return {
    operationId: "op-1",
    profileId: "managed",
    stage: "awaitingUser",
    verificationUrl: null,
    userCode: null,
    errorKind: null,
    ...overrides,
  };
}

describe("ManagedLoginDialog", () => {
  it("shows the exact device verification URL and code", () => {
    render(
      <ManagedLoginDialog
        open
        state={loginState({
          verificationUrl: "https://auth.example/device",
          userCode: "ABCD-1234",
        })}
        onStart={() => {}}
        onCancel={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("https://auth.example/device")).toBeInTheDocument();
    expect(screen.getByText("ABCD-1234")).toBeInTheDocument();
  });

  it("offers retry with device code only after a failed browser attempt", () => {
    const onStart = vi.fn();
    render(
      <ManagedLoginDialog
        open
        state={loginState({ stage: "failed", errorKind: "offlineOrTimeout" })}
        onStart={onStart}
        onCancel={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /retry with device code/i }));
    expect(onStart).toHaveBeenCalledWith("deviceCode");
  });

  it("does not offer retry while the operation is still running", () => {
    render(
      <ManagedLoginDialog
        open
        state={loginState({ stage: "starting" })}
        onStart={() => {}}
        onCancel={() => {}}
        onClose={() => {}}
      />,
    );
    expect(
      screen.queryByRole("button", { name: /retry with device code/i }),
    ).not.toBeInTheDocument();
  });

  it("renders Chinese failed and successful managed-login states", () => {
    const { rerender } = render(
      <ManagedLoginDialog
        open
        state={loginState({ stage: "failed", errorKind: "offlineOrTimeout" })}
        copy={settingsCopy("zh-CN")}
        onStart={() => {}}
        onCancel={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("登录失败。请使用设备代码重试。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "使用设备代码重试" })).toBeInTheDocument();
    rerender(
      <ManagedLoginDialog
        open
        state={loginState({ stage: "succeeded" })}
        copy={settingsCopy("zh-CN")}
        onStart={() => {}}
        onCancel={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("登录成功。")).toBeInTheDocument();
  });
});
