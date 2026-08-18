import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invokeMock } from "../test/setup";
import {
  currentCliProfile,
  bootstrapWithTwoProfiles,
  personalLateState,
  profileUsageFixture,
} from "../test/profileUsageFixtures";
import { events } from "../lib/tauri";
import type {
  AccountsSnapshotDto,
  AppErrorDto,
  ManagedLoginStateDto,
  ProfileUsageStateDto,
} from "../types/bridge";

type EventCallback<T> = (event: { payload: T }) => void;

const eventHarness = vi.hoisted(() => {
  const listeners = new Map<string, Set<EventCallback<unknown>>>();
  return {
    listeners,
    failEvent: null as string | null,
    unlistenCalls: [] as string[],
    emit<T>(eventName: string, payload: T) {
      for (const callback of listeners.get(eventName) ?? []) {
        callback({ payload });
      }
    },
    listen<T>(eventName: string, callback: EventCallback<T>) {
      if (eventHarness.failEvent === eventName) {
        eventHarness.failEvent = null;
        return Promise.reject(new Error(`failed to listen: ${eventName}`));
      }
      const callbacks = eventHarness.listeners.get(eventName) ?? new Set();
      callbacks.add(callback as EventCallback<unknown>);
      eventHarness.listeners.set(eventName, callbacks);
      return Promise.resolve(() => {
        callbacks.delete(callback as EventCallback<unknown>);
        if (callbacks.size === 0) {
          eventHarness.listeners.delete(eventName);
        }
        eventHarness.unlistenCalls.push(eventName);
      });
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: <T,>(eventName: string, callback: EventCallback<T>) =>
    eventHarness.listen(eventName, callback),
}));

import { useProfileUsage } from "./useProfileUsage";

describe("useProfileUsage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    eventHarness.listeners.clear();
    eventHarness.failEvent = null;
    eventHarness.unlistenCalls.length = 0;
  });

  it("switches to target cache before its background refresh", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "select_profile") {
        return {
          profiles: bootstrap.profiles,
          selectedProfileId: "work",
        };
      }
      return undefined;
    });

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    await act(async () => {
      await result.current.selectProfile("work");
    });

    expect(result.current.state.profileId).toBe("work");
    expect(result.current.state.primary?.remainingPercent).toBe(61);
    expect(result.current.isSwitching).toBe(true);
  });

  it("ignores a late usage event for the previous profile", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockResolvedValue({
      profiles: bootstrap.profiles,
      selectedProfileId: "work",
    });

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    await act(async () => {
      await result.current.selectProfile("work");
    });

    act(() => {
      eventHarness.emit(events.profileUsageStateChanged, personalLateState());
    });

    expect(result.current.state.profileId).toBe("work");
    expect(result.current.state.primary?.remainingPercent).toBe(61);
  });

  it("keeps a stale cached snapshot beside the current offline error", () => {
    const bootstrap = bootstrapWithTwoProfiles();
    const staleError: AppErrorDto = {
      kind: "offlineOrTimeout",
      userMessageKey: "errors.offlineOrTimeout",
      action: "retry",
      retryAfter: null,
    };
    bootstrap.usageByProfile.personal = profileUsageFixture("personal", 42, {
      freshness: "stale",
      currentError: staleError,
    });

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    expect(result.current.state.primary?.remainingPercent).toBe(42);
    expect(result.current.state.currentError?.kind).toBe("offlineOrTimeout");
    expect(result.current.state.freshness).toBe("stale");
  });

  it("creates an explicit missing state when a selected profile has no cache", () => {
    const bootstrap = bootstrapWithTwoProfiles();
    delete bootstrap.usageByProfile.personal;

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    expect(result.current.state.profileId).toBe("personal");
    expect(result.current.state.freshness).toBe("missing");
    expect(result.current.state.primary).toBeNull();
  });

  it("keeps an API-key no-quota error without inventing usage", () => {
    const bootstrap = bootstrapWithTwoProfiles();
    bootstrap.usageByProfile.personal = profileUsageFixture("personal", 0, {
      primary: null,
      currentError: {
        kind: "apiKeyNoQuota",
        userMessageKey: "errors.apiKeyNoQuota",
        action: "explainApiBilling",
        retryAfter: null,
      },
      freshness: "missing",
    });

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    expect(result.current.state.primary).toBeNull();
    expect(result.current.state.currentError?.kind).toBe("apiKeyNoQuota");
  });

  it("records a cooldown response and does not send a second refresh", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    const cooldownUntil = "2099-01-01T00:00:00.000Z";
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "refresh_selected_profile") {
        throw { manualCooldownUntil: cooldownUntil };
      }
      return undefined;
    });

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    await act(async () => {
      await result.current.refresh();
      await result.current.refresh();
    });

    expect(result.current.state.refreshStatus).toBe("cooldown");
    expect(result.current.state.manualCooldownUntil).toBe(cooldownUntil);
    expect(
      invokeMock.mock.calls.filter(
        ([command]) => command === "refresh_selected_profile",
      ),
    ).toHaveLength(1);
  });

  it("rolls selection back when the shell rejects the target profile", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    invokeMock.mockRejectedValue(new Error("selection failed"));

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    await act(async () => {
      await expect(result.current.selectProfile("work")).rejects.toThrow(
        "selection failed",
      );
    });

    expect(result.current.selectedProfileId).toBe("personal");
    expect(result.current.state.profileId).toBe("personal");
    expect(result.current.state.primary?.remainingPercent).toBe(42);
    expect(result.current.isSwitching).toBe(false);
  });

  it("preserves bootstrap cache when one event listener fails", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    eventHarness.failEvent = events.localeChanged;

    const { result } = renderHook(() => useProfileUsage(bootstrap));

    await waitFor(() => {
      expect(eventHarness.listeners.size).toBe(7);
    });
    expect(result.current.state.primary?.remainingPercent).toBe(42);
    expect(result.current.state.freshness).toBe("fresh");
  });

  it("reconciles account, selected-profile, refresh, and login events", async () => {
    const bootstrap = bootstrapWithTwoProfiles();
    const updatedAccounts: AccountsSnapshotDto = {
      profiles: [
        currentCliProfile({ label: "Personal (renamed)" }),
        bootstrap.profiles[1]!,
      ],
      selectedProfileId: "personal",
    };
    const loginState: ManagedLoginStateDto = {
      operationId: "operation-1",
      profileId: "work",
      stage: "awaitingUser",
      verificationUrl: "https://example.test/device",
      userCode: "ABCD",
      errorKind: null,
    };

    const { result } = renderHook(() => useProfileUsage(bootstrap));
    await waitFor(() => expect(eventHarness.listeners.size).toBe(8));

    act(() => {
      eventHarness.emit(events.accountsUpdated, updatedAccounts);
      eventHarness.emit(events.accountLoginUpdated, loginState);
      eventHarness.emit(events.refreshStateChanged, {
        profileId: "personal",
        status: "refreshing",
      });
    });

    expect(result.current.profiles[0]?.label).toBe("Personal (renamed)");
    expect(result.current.loginState).toEqual(loginState);
    expect(result.current.state.refreshStatus).toBe("refreshing");

    act(() => {
      eventHarness.emit(events.selectedProfileChanged, { profileId: "work" });
    });
    expect(result.current.selectedProfileId).toBe("work");
    expect(result.current.state.primary?.remainingPercent).toBe(61);

    act(() => {
      eventHarness.emit(events.accountsUpdated, updatedAccounts);
    });
    expect(result.current.selectedProfileId).toBe("work");
  });

  it("calls every registered unlisten callback on unmount", async () => {
    const { unmount } = renderHook(() =>
      useProfileUsage(bootstrapWithTwoProfiles()),
    );
    await waitFor(() => expect(eventHarness.listeners.size).toBe(8));

    unmount();

    await waitFor(() => {
      expect(eventHarness.unlistenCalls).toHaveLength(8);
    });
    expect(eventHarness.listeners.size).toBe(0);
  });
});
