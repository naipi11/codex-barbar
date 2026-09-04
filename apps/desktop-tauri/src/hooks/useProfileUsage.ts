import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  events,
  refreshSelectedProfile,
  selectProfile as invokeSelectProfile,
} from "../lib/tauri";
import type {
  AccountsSnapshotDto,
  BootstrapDto,
  ManagedLoginStateDto,
  ProfileSummaryDto,
  ProfileUsageStateDto,
  RefreshStateChangedDto,
  RefreshStatus,
  SelectedProfileChangedDto,
} from "../types/bridge";
import { parseProfileUsageState } from "../types/bridge";

export interface UseProfileUsageResult {
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  state: ProfileUsageStateDto;
  refresh(): Promise<void>;
  selectProfile(profileId: string): Promise<void>;
  isSwitching: boolean;
  loginState: ManagedLoginStateDto | null;
}

type EventEnvelope<T> = { payload: T };
type Unlisten = () => void | Promise<void>;

const REFRESH_STATUSES: readonly RefreshStatus[] = [
  "idle",
  "refreshing",
  "cooldown",
  "backoff",
  "blocked",
];

const LOGIN_STAGES: readonly ManagedLoginStateDto["stage"][] = [
  "starting",
  "awaitingUser",
  "succeeded",
  "failed",
  "cancelled",
];

function missingState(profileId: string): ProfileUsageStateDto {
  return {
    profileId,
    primary: null,
    secondary: null,
    additionalWindows: [],
    fetchedAt: null,
    currentError: null,
    freshness: "missing",
    refreshStatus: "idle",
    manualCooldownUntil: null,
    protocolAnomaly: false,
  };
}

function cacheFromBootstrap(
  bootstrap: BootstrapDto,
): Record<string, ProfileUsageStateDto> {
  const cache: Record<string, ProfileUsageStateDto> = {};
  for (const [profileId, value] of Object.entries(bootstrap.usageByProfile)) {
    try {
      const parsed = parseProfileUsageState(value);
      cache[profileId] = parsed;
    } catch {
      // A malformed event/cache must not replace a valid state with a
      // frontend-generated approximation.  The selected profile falls back to
      // an explicit missing state instead.
    }
  }
  return cache;
}

function bootstrapKey(bootstrap: BootstrapDto): string {
  // The hook is often called with an inline fixture/object expression.  A
  // content key avoids resetting local state merely because the caller made a
  // new object with the same bootstrap payload.  Settings events update the
  // surrounding bootstrap object without changing the usage snapshot; they
  // must not overwrite fresher event-driven usage held in cacheRef.
  return JSON.stringify({
    profiles: bootstrap.profiles,
    selectedProfileId: bootstrap.selectedProfileId,
    usageByProfile: bootstrap.usageByProfile,
  });
}

function isAccountsSnapshot(value: unknown): value is AccountsSnapshotDto {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<AccountsSnapshotDto>;
  return (
    Array.isArray(candidate.profiles) &&
    typeof candidate.selectedProfileId === "string"
  );
}

function isRefreshState(value: unknown): value is RefreshStateChangedDto {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<RefreshStateChangedDto>;
  return (
    typeof candidate.profileId === "string" &&
    typeof candidate.status === "string" &&
    REFRESH_STATUSES.includes(candidate.status as RefreshStatus)
  );
}

function profileIdFromSelection(value: unknown): string | null {
  if (typeof value === "string" && value.length > 0) return value;
  if (!value || typeof value !== "object") return null;
  const candidate = value as Partial<SelectedProfileChangedDto>;
  return typeof candidate.profileId === "string" && candidate.profileId.length > 0
    ? candidate.profileId
    : null;
}

function isLoginState(value: unknown): value is ManagedLoginStateDto {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<ManagedLoginStateDto>;
  return (
    typeof candidate.operationId === "string" &&
    typeof candidate.profileId === "string" &&
    typeof candidate.stage === "string" &&
    LOGIN_STAGES.includes(candidate.stage as ManagedLoginStateDto["stage"]) &&
    (candidate.verificationUrl === null ||
      typeof candidate.verificationUrl === "string") &&
    (candidate.userCode === null || typeof candidate.userCode === "string") &&
    (candidate.errorKind === null || typeof candidate.errorKind === "string") &&
    typeof candidate.runtimeCleanupFailed === "boolean"
  );
}

function cooldownUntilFrom(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const candidate = value as Record<string, unknown>;
  const direct =
    candidate.manualCooldownUntil ??
    candidate.cooldownUntil ??
    candidate.retryAfter;
  if (typeof direct === "string" && direct.length > 0) return direct;
  if (typeof direct === "number" && Number.isFinite(direct)) {
    // A numeric retry-after is interpreted as seconds from now.  The shell
    // normally sends an RFC3339 timestamp, but accepting this representation
    // keeps the bridge tolerant of older command implementations.
    return new Date(Date.now() + direct * 1000).toISOString();
  }
  return null;
}

function isCooldownActive(value: string | null): boolean {
  if (!value) return false;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) && timestamp > Date.now();
}

function withRefreshStatus(
  state: ProfileUsageStateDto,
  refreshStatus: RefreshStatus,
  manualCooldownUntil = state.manualCooldownUntil,
): ProfileUsageStateDto {
  return {
    ...state,
    refreshStatus,
    manualCooldownUntil,
  };
}

export function useProfileUsage(bootstrap: BootstrapDto): UseProfileUsageResult {
  const initialCache = cacheFromBootstrap(bootstrap);
  const initialSelectedProfileId =
    bootstrap.selectedProfileId || bootstrap.profiles[0]?.id || "";

  const cacheRef = useRef<Record<string, ProfileUsageStateDto>>(initialCache);
  const selectedRef = useRef(initialSelectedProfileId);
  const profilesRef = useRef(bootstrap.profiles);
  const switchingRef = useRef(false);
  const bootstrapKeyRef = useRef(bootstrapKey(bootstrap));

  const [profiles, setProfiles] = useState<ProfileSummaryDto[]>(
    bootstrap.profiles,
  );
  const [selectedProfileId, setSelectedProfileId] = useState(
    initialSelectedProfileId,
  );
  const [state, setState] = useState<ProfileUsageStateDto>(
    initialCache[initialSelectedProfileId] ??
      missingState(initialSelectedProfileId),
  );
  const [isSwitching, setIsSwitching] = useState(false);
  const [loginState, setLoginState] = useState<ManagedLoginStateDto | null>(
    null,
  );

  const updateSelectedState = useCallback((profileId: string) => {
    const next = cacheRef.current[profileId] ?? missingState(profileId);
    selectedRef.current = profileId;
    setSelectedProfileId(profileId);
    setState(next);
  }, []);

  const updateCachedState = useCallback(
    (candidate: unknown) => {
      let next: ProfileUsageStateDto;
      try {
        next = parseProfileUsageState(candidate);
      } catch {
        return;
      }
      cacheRef.current[next.profileId] = next;
      if (next.profileId === selectedRef.current) {
        setState(next);
        if (switchingRef.current) {
          switchingRef.current = false;
          setIsSwitching(false);
        }
      }
    },
    [],
  );

  useEffect(() => {
    const nextKey = bootstrapKey(bootstrap);
    if (bootstrapKeyRef.current === nextKey) return;
    bootstrapKeyRef.current = nextKey;

    const nextCache = cacheFromBootstrap(bootstrap);
    cacheRef.current = nextCache;
    profilesRef.current = bootstrap.profiles;
    setProfiles(bootstrap.profiles);

    const nextSelected =
      bootstrap.selectedProfileId || bootstrap.profiles[0]?.id || "";
    switchingRef.current = false;
    setIsSwitching(false);
    updateSelectedState(nextSelected);
  }, [bootstrap, updateSelectedState]);

  useEffect(() => {
    let active = true;
    const unlisteners: Unlisten[] = [];

    const usageHandler = (event: EventEnvelope<unknown>) => {
      updateCachedState(event.payload);
    };

    const refreshHandler = (event: EventEnvelope<unknown>) => {
      if (!isRefreshState(event.payload)) return;
      const { profileId, status } = event.payload;
      const current = cacheRef.current[profileId] ?? missingState(profileId);
      const next = withRefreshStatus(current, status);
      cacheRef.current[profileId] = next;
      if (profileId === selectedRef.current) {
        setState(next);
        if (switchingRef.current) {
          switchingRef.current = false;
          setIsSwitching(false);
        }
      }
    };

    const accountsHandler = (event: EventEnvelope<unknown>) => {
      if (!isAccountsSnapshot(event.payload)) return;
      const snapshot = event.payload;
      profilesRef.current = snapshot.profiles;
      setProfiles(snapshot.profiles);
      if (
        !snapshot.profiles.some(
          (profile) => profile.id === selectedRef.current,
        )
      ) {
        updateSelectedState(snapshot.selectedProfileId);
        switchingRef.current = false;
        setIsSwitching(false);
      }
    };

    const loginHandler = (event: EventEnvelope<unknown>) => {
      if (event.payload === null) {
        setLoginState(null);
      } else if (isLoginState(event.payload)) {
        setLoginState(event.payload);
      }
    };

    const selectedHandler = (event: EventEnvelope<unknown>) => {
      const profileId = profileIdFromSelection(event.payload);
      if (!profileId) return;
      updateSelectedState(profileId);
      switchingRef.current = false;
      setIsSwitching(false);
    };

    const registrations: Array<
      [string, (event: EventEnvelope<unknown>) => void]
    > = [
      [events.profileUsageStateChanged, usageHandler],
      [events.refreshStateChanged, refreshHandler],
      [events.accountsUpdated, accountsHandler],
      [events.accountLoginUpdated, loginHandler],
      [events.selectedProfileChanged, selectedHandler],
      [events.settingsChanged, () => {}],
      [events.localeChanged, () => {}],
      [events.updateStateChanged, () => {}],
    ];

    const register = async () => {
      await Promise.all(
        registrations.map(async ([eventName, handler]) => {
          try {
            const unlisten = await listen<unknown>(eventName, handler);
            if (!active) {
              await unlisten();
            } else {
              unlisteners.push(unlisten);
            }
          } catch {
            // A failed event subscription must not discard the bootstrap
            // cache.  Other subscriptions continue independently.
          }
        }),
      );
    };
    void register();

    return () => {
      active = false;
      const pending = unlisteners.splice(0).map((unlisten) => {
        try {
          return Promise.resolve(unlisten()).catch(() => {});
        } catch {
          return Promise.resolve();
        }
      });
      void Promise.all(pending);
    };
  }, [updateCachedState, updateSelectedState]);

  const selectProfile = useCallback(
    async (profileId: string) => {
      const previousProfileId = selectedRef.current;
      const previousProfiles = profilesRef.current;
      try {
        const snapshot = await invokeSelectProfile(profileId);
        if (!isAccountsSnapshot(snapshot)) {
          throw new Error("invalid profile selection response");
        }

        profilesRef.current = snapshot.profiles;
        setProfiles(snapshot.profiles);
        const nextProfileId = snapshot.selectedProfileId || profileId;
        switchingRef.current = true;
        setIsSwitching(true);
        updateSelectedState(nextProfileId);
      } catch (error) {
        profilesRef.current = previousProfiles;
        setProfiles(previousProfiles);
        updateSelectedState(previousProfileId);
        switchingRef.current = false;
        setIsSwitching(false);
        throw error;
      }
    },
    [updateSelectedState],
  );

  const refresh = useCallback(async () => {
    const profileId = selectedRef.current;
    const current = cacheRef.current[profileId] ?? missingState(profileId);
    if (
      current.refreshStatus === "cooldown" ||
      isCooldownActive(current.manualCooldownUntil)
    ) {
      const next = withRefreshStatus(
        current,
        "cooldown",
        current.manualCooldownUntil,
      );
      cacheRef.current[profileId] = next;
      setState(next);
      return;
    }

    const refreshing = withRefreshStatus(current, "refreshing", null);
    cacheRef.current[profileId] = refreshing;
    setState(refreshing);

    try {
      const response = (await refreshSelectedProfile()) as unknown;
      const cooldownUntil = cooldownUntilFrom(response);
      if (cooldownUntil) {
        const cooldown = withRefreshStatus(
          cacheRef.current[profileId] ?? refreshing,
          "cooldown",
          cooldownUntil,
        );
        cacheRef.current[profileId] = cooldown;
        if (selectedRef.current === profileId) setState(cooldown);
      }
    } catch (error) {
      const cooldownUntil = cooldownUntilFrom(error);
      if (cooldownUntil) {
        const cooldown = withRefreshStatus(
          cacheRef.current[profileId] ?? refreshing,
          "cooldown",
          cooldownUntil,
        );
        cacheRef.current[profileId] = cooldown;
        if (selectedRef.current === profileId) setState(cooldown);
      }
      // Refresh errors are also published by the backend as usage/error
      // events.  Keep the cached state visible and let that event carry the
      // user-facing error; do not synthesize or overwrite it here.
    }
  }, []);

  return {
    profiles,
    selectedProfileId,
    state,
    refresh,
    selectProfile,
    isSwitching,
    loginState,
  };
}
