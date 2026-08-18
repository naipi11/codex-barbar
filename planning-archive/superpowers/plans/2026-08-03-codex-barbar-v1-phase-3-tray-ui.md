# codex-barbar V1 Phase 3 Tray UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the complete Codex tray product experience: cache-first profile-aware flyout, dynamic icon and native menu, Managed-account settings, safe path/update actions, Chinese/English localization, theme/DPI behavior, and keyboard/screen-reader accessibility.

**Architecture:** Rust remains the source of truth for profiles, freshness, errors, settings, fixed external actions, and tray pixels. A narrow Tauri bridge serializes redacted bootstrap/state DTOs; one React hook merges bootstrap cache with typed events and rejects stale-profile events. The main hidden Tauri window acts as the left-click tray flyout, while Settings is the only detached window; no PopOut, FloatBar, arbitrary URL/path, or frontend network path returns.

**Tech Stack:** React 18, TypeScript 5.6, Testing Library/Vitest 3, Tauri 2 event/invoke APIs, shared Rust tray renderer, Fluent locale files, existing `open`/reqwest crates behind fixed Rust actions, real Windows CUA Driver.

## Global Constraints

- Supported platform is native Windows 11 23H2 or newer on x64; Windows 10, Windows on ARM, WSL, macOS, and Linux builds are outside V1.
- The shipping product supports Codex only. Claude, Gemini, every other provider, browser-cookie import, generic API-key/token accounts, cost charts, sessions, workspaces, PTY, FloatBar, PopOut, and usage notifications stay outside the release surface.
- Account and quota network access goes only through the official but experimental `codex app-server` stdio JSONL process; `experimentalApi` remains `false`, and private `/wham/*` calls are removed without a release fallback.
- `CurrentCli` is read-only: it may call `initialize`, send `initialized`, call `account/read`, and call `account/rateLimits/read`; it has no login, logout, delete, switch, or configuration-write method.
- Every `Managed` profile uses an isolated `CODEX_HOME`, forces `cli_auth_credentials_store = "file"`, clears authentication override variables in the child only, and keeps idle credentials in strict DPAPI Current User ciphertext.
- DPAPI failure is fatal for the vault operation. There is no Local Machine scope fallback and no plaintext fallback. Vault replacement is temporary-file, flush, then atomic replace.
- The React WebView receives no OAuth token, refresh token, raw `auth.json`, arbitrary filesystem capability, arbitrary shell/process capability, or arbitrary network capability.
- Startup does not check for, download, or apply updates. A user-initiated action may check a public GitHub Release or open the fixed Releases page; no PAT is embedded or requested.
- Default refresh interval is 5 minutes; panel-open refresh threshold is 60 seconds; manual cooldown is 15 seconds; transient backoff is 30 seconds, 2 minutes, 5 minutes, then 15 minutes with ±20% jitter.
- Product defaults are remaining-quota display, system theme, system language, autostart off, no telemetry, and `%LOCALAPPDATA%\codex-barbar` storage for installed and portable builds.
- Toolchain is pnpm 10.18.1, Node 20, Rust stable edition 2024, and target `x86_64-pc-windows-msvc`. Use pnpm only.
- Do not introduce a new third-party crate, package, build tool, hosted service, or telemetry endpoint without explicit user confirmation. Additional features on the already-present `windows` crate must be limited to the Win32 APIs named in the approved design.
- Preserve the complete Win-CodexBar Git history, MIT license, author attribution, `win-upstream`, `mac-reference`, and tag `upstream/win-codexbar-2026-08-03`.
- Every UI/tray/settings change requires a fresh desktop build, termination of the old single instance, and real Windows CUA evidence as described by `AGENTS.md`.
- Do not push, publish a GitHub Release, open a pull request, buy a signing certificate, or contact either upstream repository unless the user explicitly authorizes that external action.

---

## File Responsibility Map

| Path | Responsibility after Phase 3 |
|---|---|
| `apps/desktop-tauri/src/types/bridge.ts` | Exact redacted Rust serialization contract |
| `apps/desktop-tauri/src/lib/tauri.ts` | Final V1 invoke/event names only |
| `apps/desktop-tauri/src/hooks/useProfileUsage.ts` | Cache-first profile selection, refresh, and event reconciliation |
| `apps/desktop-tauri/src/surfaces/TrayPanel.tsx` | Ordered account/quota/status/action flyout |
| `apps/desktop-tauri/src/surfaces/tray/*` | Small accessible flyout components |
| `rust/src/tray/render.rs` | Deterministic normal/warning/danger/stale/API/unavailable icon pixels |
| `apps/desktop-tauri/src-tauri/src/tray_bridge.rs` | Tray click behavior, tooltip, state-driven icon/menu rebuild |
| `apps/desktop-tauri/src-tauri/src/tray_menu.rs` | Fixed native menu and checked profile submenu |
| `apps/desktop-tauri/src/surfaces/Settings.tsx` | General/Accounts/Advanced/About surface |
| `rust/src/storage/settings_repository.rs` | SQLite-backed V1 settings and validation |
| `rust/src/update_check.rs` | User-initiated public-release metadata check only |
| `rust/src/locale/{en-US,zh-CN}.ftl` | Complete English/Simplified Chinese copy |
| `apps/desktop-tauri/src-tauri/src/proof_harness.rs` | Credential-free fixed UI proof states |

## Test Support Contract

- `apps/desktop-tauri/src/test/profileUsageFixtures.ts` exports every `currentCliProfile`, managed profile, quota window, error state, event payload, deferred invoke, and `renderTray` helper used in this phase. Values are synthetic and contain no email/token.
- The existing `src/test/setup.ts` owns `invokeMock`, event-listener cleanup, match-media/theme stubs, and deterministic time; each test resets it in `afterEach`.
- Rust tray/settings/update fixtures stay in their owning module's `#[cfg(test)]` block. HTTP update tests bind a loopback mock server supplied directly to the checker and assert that no authorization header is sent; production URLs remain compile-time constants.
- CUA proof data comes only from the fixed `ProofScenario` enum. No proof helper calls `AccountProfileService`, resolves a real Codex executable, or reads the Vault.
- Every helper referenced below is defined in one of these named locations before its first test is compiled and is not production-exported.

## Frozen Bridge DTO

```ts
export type AppErrorKind =
  | "codexNotFound" | "unsupportedCodexVersion" | "notSignedIn"
  | "apiKeyNoQuota" | "authExpired" | "offlineOrTimeout" | "rateLimited"
  | "protocolMismatch" | "vaultFailure" | "storageFailure";

export type RecoveryAction =
  | "selectCodexExecutable" | "installTestedCodex" | "signIn"
  | "reloginManagedProfile" | "retry" | "waitAndRetry"
  | "explainApiBilling" | "exportDiagnostics";

export interface AppErrorDto {
  kind: AppErrorKind;
  userMessageKey: string;
  action: RecoveryAction;
  retryAfter: string | null;
}

export interface AppSettingsDto {
  autostartEnabled: boolean;
  refreshIntervalSeconds: 0 | 60 | 300 | 900 | 1800;
  displayMode: "remaining" | "used";
  theme: "system" | "light" | "dark";
  language: "system" | "zh-CN" | "en-US";
  codexExecutableOverride: string | null;
}

export interface ProfileSummaryDto {
  id: string;
  kind: "currentCli" | "managed";
  label: string;
  email: string | null;
  planType: string | null;
  authMode: "unknown" | "chatGpt" | "apiKey";
  removable: boolean;
  lastSuccessAt: string | null;
}

export interface UsageWindowDto {
  limitId: string;
  label: string | null;
  usedPercent: number;
  remainingPercent: number;
  windowDurationMinutes: number | null;
  resetsAt: string | null;
  reachedType: string | null;
}

export interface CodexCompatibilityDto {
  status: "notChecked" | "compatible" | "notFound" | "unsupported";
  installation: "nativeExe" | "verifiedNpmLayout" | null;
  executablePath: string | null;
  version: string | null;
  capabilities: {
    accountRead: boolean;
    rateLimitsRead: boolean;
    managedLogin: boolean;
  };
}

export interface ManagedLoginStateDto {
  operationId: string;
  profileId: string;
  stage: "starting" | "awaitingUser" | "succeeded" | "failed" | "cancelled";
  verificationUrl: string | null;
  userCode: string | null;
  errorKind: AppErrorKind | null;
}

export interface BootstrapDto {
  productName: "codex-barbar";
  version: string;
  settings: AppSettingsDto;
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  usageByProfile: Record<string, ProfileUsageStateDto>;
  codex: CodexCompatibilityDto;
}

export interface ProfileUsageStateDto {
  profileId: string;
  primary: UsageWindowDto | null;
  secondary: UsageWindowDto | null;
  additionalWindows: UsageWindowDto[];
  fetchedAt: string | null;
  currentError: AppErrorDto | null;
  freshness: "fresh" | "stale" | "missing";
  refreshStatus: "idle" | "refreshing" | "cooldown" | "backoff" | "blocked";
  manualCooldownUntil: string | null;
  protocolAnomaly: boolean;
}
```

`primary`/`secondary`/`fetchedAt` always describe the last successful snapshot. `currentError` describes only the latest attempt; both may be present simultaneously.

### Task 1: Freeze the V1 Tauri bridge and final window routing

**Files:**
- Replace: `apps/desktop-tauri/src/types/bridge.ts`
- Replace: `apps/desktop-tauri/src/types/bridge.test.ts`
- Replace: `apps/desktop-tauri/src/lib/tauri.ts`
- Replace: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/app.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/accounts.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/events.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Replace: `apps/desktop-tauri/src/App.tsx`
- Replace: `apps/desktop-tauri/src/App.test.tsx`
- Test: Rust inline bridge tests plus the two frontend tests

**Interfaces:**
- Consumes: Phase-2 bootstrap, profiles, settings, `ProfileUsageState`, login status, and event stream.
- Produces: frozen DTOs, final command-name exports, final event-name constants, and `main`/`settings` routing only.

- [ ] **Step 1: Write failing cache+error and startup-call tests**

```ts
it("keeps last success and current error as separate fields", () => {
  const state = parseProfileUsageState(profileUsageFixture({
    primaryRemaining: 42,
    errorKind: "offlineOrTimeout",
  }));
  expect(state.primary?.remainingPercent).toBe(42);
  expect(state.currentError?.kind).toBe("offlineOrTimeout");
});

it("bootstraps once without checking for updates", async () => {
  render(<App />);
  await screen.findByRole("heading", { name: "codex-barbar" });
  expect(invokeMock).toHaveBeenCalledWith("get_bootstrap_state");
  expect(invokeMock).not.toHaveBeenCalledWith("check_for_updates", expect.anything());
});
```

- [ ] **Step 2: Run tests and verify DTO/legacy-route failure**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/types/bridge.test.ts src/App.test.tsx
```

Expected: FAIL until the Phase-0 placeholder DTO is expanded and old routes are absent.

- [ ] **Step 3: Implement exact command and event constants**

The frontend exports only:

```ts
export const commands = {
  getBootstrapState: "get_bootstrap_state",
  getSettingsSnapshot: "get_settings_snapshot",
  updateSettings: "update_settings",
  getLocaleStrings: "get_locale_strings",
  selectProfile: "select_profile",
  refreshSelectedProfile: "refresh_selected_profile",
  startManagedLogin: "start_managed_login",
  cancelManagedLogin: "cancel_managed_login",
  renameManagedProfile: "rename_managed_profile",
  removeManagedProfile: "remove_managed_profile",
  validateCodexExecutable: "validate_codex_executable",
  checkForUpdates: "check_for_updates",
  openReleasePage: "open_release_page",
  openCodexUsagePage: "open_codex_usage_page",
  openSettingsWindow: "open_settings_window",
  closeSettingsWindow: "close_settings_window",
  dismissTrayPanel: "dismiss_tray_panel",
  setFlyoutSize: "set_flyout_size",
  getCurrentSurfaceState: "get_current_surface_state",
  quitApp: "quit_app",
} as const;
```

Events are `profile-usage-state-changed`, `refresh-state-changed`, `accounts-updated`, `account-login-updated`, `selected-profile-changed`, `settings-changed`, `locale-changed`, and `update-state-changed`. Route Tauri label `main` to `TrayPanel`, `settings` to `Settings`, and every other label to `null`. Register matching Rust commands only; Phase 4 adds the two diagnostics commands after their trusted backend exists.

- [ ] **Step 4: Run bridge, Tauri, and frontend regressions**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge::tests -- --nocapture
pnpm --dir apps/desktop-tauri exec vitest run src/types/bridge.test.ts src/App.test.tsx
pnpm --dir apps/desktop-tauri run build
```

Expected: all pass; `App.tsx` contains no PopOut, FloatBar, provider, or startup-update import.

- [ ] **Step 5: Commit**

```powershell
git add apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src-tauri/src/commands/app.rs apps/desktop-tauri/src-tauri/src/commands/accounts.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src-tauri/src/events.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src/App.tsx apps/desktop-tauri/src/App.test.tsx
git commit -m "Define the V1 desktop bridge"
```

### Task 2: Add the profile-aware React state hook

**Files:**
- Create: `apps/desktop-tauri/src/hooks/useProfileUsage.ts`
- Create: `apps/desktop-tauri/src/hooks/useProfileUsage.test.tsx`
- Create: `apps/desktop-tauri/src/test/profileUsageFixtures.ts`
- Delete after replacement: `apps/desktop-tauri/src/hooks/useProviders.ts`
- Delete after replacement: `apps/desktop-tauri/src/hooks/useProviders.test.tsx`
- Delete after replacement: `apps/desktop-tauri/src/hooks/useTrayPanelController.ts`

**Interfaces:**
- Consumes: `BootstrapDto`, typed invoke functions, eight event names.
- Produces: `UseProfileUsageResult` for tray/settings consumers.

- [ ] **Step 1: Write failing cache-first and late-event tests**

```tsx
it("switches to target cache before its background refresh", async () => {
  const { result } = renderHook(() => useProfileUsage(bootstrapWithTwoProfiles()));
  await act(() => result.current.selectProfile("work"));
  expect(result.current.state.profileId).toBe("work");
  expect(result.current.state.primary?.remainingPercent).toBe(61);
  expect(result.current.isSwitching).toBe(true);
});

it("ignores a late usage event for the previous profile", async () => {
  const { result } = renderHook(() => useProfileUsage(bootstrapWithTwoProfiles()));
  await act(() => result.current.selectProfile("work"));
  act(() => emitUsageState(personalLateState()));
  expect(result.current.state.profileId).toBe("work");
});
```

- [ ] **Step 2: Run the focused hook test and verify missing hook**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/hooks/useProfileUsage.test.tsx
```

Expected: FAIL because the hook does not exist.

- [ ] **Step 3: Implement event reconciliation and cleanup**

```ts
export interface UseProfileUsageResult {
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  state: ProfileUsageStateDto;
  refresh(): Promise<void>;
  selectProfile(profileId: string): Promise<void>;
  isSwitching: boolean;
  loginState: ManagedLoginStateDto | null;
}
```

Initialize from bootstrap synchronously. On select response, set selected ID and cached target state, mark switching, then wait for that profile's usage/refresh event. Reject usage events whose profile ID differs from current selected ID; accept account events for the list. Preserve existing cache if listener registration fails. Cooldown response updates `manualCooldownUntil` and sends no second invoke. Store every `unlisten` callback and await/call all on unmount.

- [ ] **Step 4: Run the full hook behavior matrix**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/hooks/useProfileUsage.test.tsx
```

Expected: bootstrap cache, target-cache switch, late event, stale+error, missing, API no quota, cooldown, selection rollback, listener failure, and unmount cleanup pass.

- [ ] **Step 5: Commit**

```powershell
git add -A apps/desktop-tauri/src/hooks apps/desktop-tauri/src/test/profileUsageFixtures.ts
git commit -m "Add profile-aware usage state"
```

### Task 3: Build the accessible left-click tray dashboard

**Files:**
- Replace: `apps/desktop-tauri/src/surfaces/TrayPanel.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx`
- Create: `apps/desktop-tauri/src/surfaces/tray/ProfileSelector.tsx`
- Create: `apps/desktop-tauri/src/surfaces/tray/QuotaCard.tsx`
- Create: `apps/desktop-tauri/src/surfaces/tray/UsageStatus.tsx`
- Create: `apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx`
- Create: `apps/desktop-tauri/src/surfaces/tray/TrayPanel.css`
- Modify: `apps/desktop-tauri/src/hooks/useFormattedResetTime.ts`
- Modify: `apps/desktop-tauri/src/hooks/useFormattedResetTime.test.tsx`
- Modify: `apps/desktop-tauri/src/lib/relativeTime.ts`
- Modify: `apps/desktop-tauri/src/lib/relativeTime.test.ts`

**Interfaces:**
- Consumes: `UseProfileUsageResult`, settings display mode, localization function.
- Produces: ordered profile/primary/secondary/status/action flyout with semantic labels.

- [ ] **Step 1: Write failing ordered-state and accessibility tests**

```tsx
it("renders account, short window, long window, state, and actions in order", () => {
  renderTray(readyTwoWindowFixture());
  const regions = screen.getAllByRole("region").map((node) => node.getAttribute("aria-label"));
  expect(regions).toEqual(["Account", "5-hour quota", "Weekly quota", "Data status", "Actions"]);
});

it("keeps cached quota visible beside an offline error", () => {
  renderTray(staleOfflineFixture());
  expect(screen.getByRole("progressbar", { name: /5-hour.*42% remaining/i })).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("Offline");
  expect(screen.getByText(/last updated/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run focused UI tests and verify placeholder failure**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/TrayPanel.test.tsx src/hooks/useFormattedResetTime.test.tsx src/lib/relativeTime.test.ts
```

Expected: FAIL while the Phase-0 placeholder remains.

- [ ] **Step 3: Implement exact rendering and keyboard behavior**

`QuotaCard` uses `role="progressbar"`, `aria-valuemin=0`, `aria-valuemax=100`, current displayed value for `aria-valuenow`, and an accessible name containing window label, percentage, and reset countdown. The default is remaining; used mode changes text/progress value only. 300 minutes is “5 hours”, 10,080 is “Weekly”; unknown duration is formatted. `formatResetTime(resetsAt, now, locale, timeZone)` receives an explicit IANA `timeZone` in tests and production supplies `Intl.DateTimeFormat().resolvedOptions().timeZone`; include `Asia/Shanghai` and `America/Los_Angeles` fixtures around a UTC-day boundary. Past reset is “Awaiting refresh”, never negative. Errors map to one fixed action: path settings, tested-version guidance, login/re-login, retry, diagnostics, or API billing explanation. Escape invokes `dismiss_tray_panel`; focus starts at profile selector or refresh button; native Tab/Enter/Space behavior is retained.

- [ ] **Step 4: Run all flyout data/keyboard tests**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/TrayPanel.test.tsx src/hooks/useFormattedResetTime.test.tsx src/lib/relativeTime.test.ts
```

Expected: ready, refreshing, stale, cache+error, missing, API, two-window, unknown-window, expired reset, Shanghai/Los Angeles date boundaries, anomaly, long label/email, English/Chinese, Tab, Enter, Space, Escape, and accessible-name cases pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/desktop-tauri/src/surfaces/TrayPanel.tsx apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx apps/desktop-tauri/src/surfaces/tray apps/desktop-tauri/src/hooks/useFormattedResetTime.ts apps/desktop-tauri/src/hooks/useFormattedResetTime.test.tsx apps/desktop-tauri/src/lib/relativeTime.ts apps/desktop-tauri/src/lib/relativeTime.test.ts
git commit -m "Build the Codex tray dashboard"
```

### Task 4: Render dynamic tray state and the fixed native right-click menu

**Files:**
- Replace: `rust/src/tray/icon.rs`
- Replace: `rust/src/tray/render.rs`
- Modify: `rust/src/tray/mod.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/tray_menu.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/events.rs`
- Test: inline tray tests in both Rust crates

**Interfaces:**
- Consumes: selected `ProfileUsageState`, account list, settings language.
- Produces: `TrayVisualState`, RGBA icon, secret-free tooltip, native menu, left/right click dispatch.

- [ ] **Step 1: Write failing visual boundary/menu-order tests**

```rust
#[test]
fn remaining_thresholds_use_exact_v1_levels() {
    assert_eq!(TrayVisualState::from_remaining(51, false).level(), Some(TrayLevel::Normal));
    assert_eq!(TrayVisualState::from_remaining(50, false).level(), Some(TrayLevel::Warning));
    assert_eq!(TrayVisualState::from_remaining(50.4, false).level(), Some(TrayLevel::Normal));
    assert_eq!(TrayVisualState::from_remaining(21, false).level(), Some(TrayLevel::Warning));
    assert_eq!(TrayVisualState::from_remaining(20, false).level(), Some(TrayLevel::Danger));
}

#[test]
fn native_menu_order_is_fixed() {
    assert_eq!(menu_item_ids(), ["open_panel", "refresh", "accounts", "open_usage", "settings", "about", "quit"]);
}
```

- [ ] **Step 2: Run tests and verify legacy merged-provider failure**

```powershell
cargo test --manifest-path rust/Cargo.toml remaining_thresholds_use_exact_v1_levels
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml native_menu_order_is_fixed
```

Expected: FAIL until legacy provider tray behavior is replaced.

- [ ] **Step 3: Implement icon/tooltip/menu rules**

```rust
pub enum TrayVisualState {
    Remaining { percent: u8, level: TrayLevel },
    Stale { percent: u8 },
    Api,
    Unavailable,
}

pub const NORMAL_RGBA: [u8; 4] = [59, 130, 246, 255];
pub const WARNING_RGBA: [u8; 4] = [245, 158, 11, 255];
pub const DANGER_RGBA: [u8; 4] = [239, 68, 68, 255];
pub const STALE_RGBA: [u8; 4] = [156, 163, 175, 255];
```

Choose the lowest finite normalized `f64` remaining value of all available selected-profile windows. Compute the color threshold from that unrounded value (`> 50`, `> 20`, otherwise danger), then display `remaining.round().clamp(0.0, 100.0) as u8`; this keeps `50.4` in the normal color even though the digits render `50`. Stale renders gray digits; API renders `API`; missing/auth failure renders `!`. Tooltip includes profile label, minimum remaining, primary/secondary, updated time, and state; never email or diagnostic detail. Profile submenu uses checked items and no delete/logout. Left click toggles main flyout; right click opens only the native menu. `open_usage` calls fixed Rust action for `https://chatgpt.com/codex/settings/usage`.

- [ ] **Step 4: Run tray renderer/bridge suites**

```powershell
cargo test --manifest-path rust/Cargo.toml tray:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tray_ -- --nocapture
```

Expected: thresholds, minimum window, stale/API/unavailable pixel differences, menu order, checked profile, tooltip secrecy, event rebuild, and left/right dispatch pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/tray apps/desktop-tauri/src-tauri/src/tray_menu.rs apps/desktop-tauri/src-tauri/src/tray_bridge.rs apps/desktop-tauri/src-tauri/src/events.rs
git commit -m "Render Codex tray status"
```

### Task 5: Implement SQLite settings, account management, and safe Codex path validation

**Files:**
- Create: `rust/src/storage/settings_repository.rs`
- Create: `rust/src/platform/mod.rs`
- Create: `rust/src/platform/windows/mod.rs`
- Create: `rust/src/platform/windows/autostart.rs`
- Modify: `rust/src/lib.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/accounts.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Replace: `apps/desktop-tauri/src/surfaces/Settings.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/Settings.test.ts`
- Replace: `apps/desktop-tauri/src/surfaces/settings/settingsTabs.ts`
- Replace: `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/AccountsTab.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/accounts/ManagedLoginDialog.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/accounts/ManagedLoginDialog.test.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/settings/tabs/AdvancedTab.tsx`
- Modify: `apps/desktop-tauri/src/hooks/useSettings.ts`

**Interfaces:**
- Consumes: Phase-2 profile service, Phase-1 resolver, SQLite `app_settings`.
- Produces: `AppSettings`, `SettingsPatch`, get/update commands, validate command, autostart adapter, and General/Accounts/Advanced tabs.

- [ ] **Step 1: Write failing settings-default and CurrentCli UI tests**

```rust
#[test]
fn v1_settings_defaults_are_exact() {
    let settings = AppSettings::default();
    assert_eq!(settings.refresh_interval_seconds, 300);
    assert_eq!(settings.display_mode, DisplayMode::Remaining);
    assert_eq!(settings.theme, ThemePreference::System);
    assert_eq!(settings.language, LanguagePreference::System);
    assert!(!settings.start_at_login);
}
```

```tsx
it("never offers remove or re-login for Current CLI", () => {
  render(<AccountsTab profiles={[currentCliProfile()]} />);
  expect(screen.queryByRole("button", { name: /remove/i })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /re-login/i })).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests and verify legacy settings failure**

```powershell
cargo test --manifest-path rust/Cargo.toml v1_settings_defaults_are_exact
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/Settings.test.ts src/surfaces/settings/accounts/ManagedLoginDialog.test.tsx
```

Expected: FAIL while legacy provider settings remain.

- [ ] **Step 3: Implement exact settings and account flows**

Only tab IDs `general`, `providers`, `advanced`, `about` remain; `providers` is displayed as Accounts. Refresh choices are 0/60/300/900/1800 seconds. Account actions are add (browser/device code), select, rename, atomic re-login, and confirm remove; no logout/API-key/cookie/token input. The login dialog renders all `ManagedLoginStateDto` stages, shows the exact device verification URL/code, restores focus to its trigger after close, and on a failed browser attempt offers “Retry with device code” only after the backend reports the previous operation stopped. Advanced accepts an absolute path, calls `validate_codex_executable`, and saves only a successful normalized result; no generic file/path IPC. Autostart derives the normalized executable from `std::env::current_exe()`, requires the filename `codex-barbar.exe`, and writes only HKCU `Software\Microsoft\Windows\CurrentVersion\Run`, value `codex-barbar`, with exactly one quoted argument followed by ` --background`. The quoting test asserts the exact sample `"C:\Program Files\codex-barbar\codex-barbar.exe" --background`; relative paths and wrong filenames are rejected.

- [ ] **Step 4: Run settings, account, resolver, and frontend suites**

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml platform::windows::autostart::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::settings::tests -- --nocapture
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/Settings.test.ts src/surfaces/settings/accounts/ManagedLoginDialog.test.tsx
```

Expected: defaults/allowed values, transactional update, invalid patch rollback, autostart quoting, validated path, CurrentCli restrictions, Managed add/cancel/re-login/rename/remove pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/storage/settings_repository.rs rust/src/platform rust/src/lib.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src-tauri/src/commands/accounts.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/Settings.test.ts apps/desktop-tauri/src/surfaces/settings apps/desktop-tauri/src/hooks/useSettings.ts
git commit -m "Add Codex account settings"
```

### Task 6: Add manual-only update checking and fixed external actions

**Files:**
- Create: `rust/src/update_check.rs`
- Modify: `rust/src/lib.rs`
- Create: `apps/desktop-tauri/src-tauri/src/commands/update.rs`
- Create: `apps/desktop-tauri/src-tauri/src/commands/fixed_actions.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Replace: `apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.test.tsx`
- Delete: `apps/desktop-tauri/src/hooks/useUpdateState.ts`
- Delete: `apps/desktop-tauri/src/components/UpdateBanner.tsx`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/updater.rs`
- Delete: `rust/src/updater.rs`
- Test: inline Rust tests and About Vitest

**Interfaces:**
- Consumes: fixed GitHub API/releases URLs, current app version, explicit user click.
- Produces: `ManualUpdateResult`, `check_for_updates`, `open_release_page`, and `open_codex_usage_page`; no download/apply state.

- [ ] **Step 1: Write failing private-repo and no-download tests**

```rust
#[tokio::test]
async fn private_release_feed_degrades_without_credentials() {
    let result = checker(mock_server_404()).check().await.unwrap();
    assert_eq!(result, ManualUpdateResult::ReleaseFeedUnavailable);
    assert_eq!(mock_server_404().authorization_headers(), Vec::<String>::new());
}

#[test]
fn update_module_exports_no_download_or_apply_function() {
    let source = include_str!("update_check.rs");
    assert!(!source.contains("download_update"));
    assert!(!source.contains("apply_update"));
}
```

- [ ] **Step 2: Run tests and verify legacy updater failure**

```powershell
cargo test --manifest-path rust/Cargo.toml private_release_feed_degrades_without_credentials
cargo test --manifest-path rust/Cargo.toml update_module_exports_no_download_or_apply_function
```

Expected: FAIL until the old updater is removed/replaced.

- [ ] **Step 3: Implement fixed, user-triggered behavior**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ManualUpdateResult {
    Current { current_version: String },
    Available { current_version: String, latest_version: String },
    ReleaseFeedUnavailable { current_version: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PrereleaseChannel { Alpha, Beta, Rc }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prerelease { channel: PrereleaseChannel, number: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReleaseVersion {
    major: u32,
    minor: u32,
    patch: u32,
    prerelease: Option<Prerelease>,
}
```

`check_for_updates` performs one anonymous GET to `https://api.github.com/repos/naipi11/codex-barbar/releases/latest` only when invoked. Parse tags with an internal dependency-free `ReleaseVersion { major, minor, patch, prerelease }`; accepted forms are `vMAJOR.MINOR.PATCH` and `vMAJOR.MINOR.PATCH-(alpha|beta|rc).N`, compared by numeric core then `alpha < beta < rc < final` and numeric `N`. Reject every other tag as `ReleaseFeedUnavailable`; do not add a semver package. A valid public response returns current/available only after `html_url` parses as HTTPS host `github.com` and exact repository path prefix `/naipi11/codex-barbar/releases/`; 401/403/404 returns `ReleaseFeedUnavailable`; it never reads a PAT. `open_release_page` opens only `https://github.com/naipi11/codex-barbar/releases`; `open_codex_usage_page` opens only `https://chatgpt.com/codex/settings/usage`. About shows version/MIT/upstreams, a manual button, and “Open Releases”; no automatic, beta-channel, download, install, or quit-install control.

- [ ] **Step 4: Run update/action/About tests and boundary scan**

```powershell
cargo test --manifest-path rust/Cargo.toml update_check::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::update::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::fixed_actions::tests -- --nocapture
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/settings/tabs/AboutTab.test.tsx
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
```

Expected: public newer/current, private/unavailable, invalid URL, fixed actions, manual-only UI, and no legacy update command pass.

- [ ] **Step 5: Commit**

```powershell
git add -A rust/src apps/desktop-tauri/src apps/desktop-tauri/src-tauri/src/commands
git commit -m "Replace automatic updates with manual checks"
```

### Task 7: Complete English/Chinese copy, theme, reduced motion, and semantics

**Files:**
- Replace: `rust/src/locale/en-US.ftl`
- Replace: `rust/src/locale/zh-CN.ftl`
- Modify: `rust/src/locale.rs`
- Modify: `rust/src/locale/tests.rs`
- Modify: `rust/src/settings/types.rs`
- Modify: `apps/desktop-tauri/src/i18n/keys.ts`
- Modify: `apps/desktop-tauri/src/i18n/keys.test.ts`
- Modify: `apps/desktop-tauri/src/i18n/LocaleProvider.tsx`
- Modify: `apps/desktop-tauri/scripts/check-locale-drift.mjs`
- Modify: `apps/desktop-tauri/src/hooks/useTheme.ts`
- Modify: `apps/desktop-tauri/src/hooks/useTheme.test.ts`
- Modify: `apps/desktop-tauri/src/styles.css`
- Modify: `rust/Cargo.toml`
- Test: locale/theme/frontend semantic tests

**Interfaces:**
- Consumes: language/theme settings and Windows system locale/theme.
- Produces: identical `en-US`/`zh-CN` key sets and explicit light/dark CSS palettes.

- [ ] **Step 1: Add failing locale parity and semantic tests**

```ts
it("requires the complete V1 locale key set", () => {
  expect(localeKeys).toEqual(expect.arrayContaining([
    "app.name", "usage.fiveHours", "usage.weekly", "usage.remaining", "usage.used",
    "usage.awaitingRefresh", "status.updated", "status.cached", "status.refreshing",
    "error.codexNotFound", "error.unsupportedCodexVersion", "error.notSignedIn",
    "error.apiKeyNoQuota", "error.authExpired", "error.offlineOrTimeout",
    "error.rateLimited", "error.protocolMismatch", "error.vaultFailure", "error.storageFailure",
    "action.refresh", "action.openUsage", "action.settings", "action.exportDiagnostics",
    "settings.general", "settings.accounts", "settings.advanced", "settings.about",
    "accounts.add", "accounts.rename", "accounts.relogin", "accounts.remove", "accounts.currentCli",
  ]));
});
```

- [ ] **Step 2: Run locale/theme tests and verify missing-key failures**

```powershell
pnpm --dir apps/desktop-tauri run check-locale
pnpm --dir apps/desktop-tauri exec vitest run src/i18n/keys.test.ts src/hooks/useTheme.test.ts src/surfaces/TrayPanel.test.tsx
```

Expected: FAIL until both locale files and new keys are complete.

- [ ] **Step 3: Implement locale/system/theme rules**

Keep only System, 简体中文, and English choices. Add existing `windows` crate feature `Win32_Globalization`; map `GetUserDefaultLocaleName` values beginning `zh-CN` or `zh-Hans` to Simplified Chinese and everything else to English. Keep the detached settings WebView2 Tauri theme pinned Dark per repository invariant, while React applies explicit `data-theme="light|dark"` CSS based on preference/system signal so one WebView does not flip another. Add `@media (prefers-reduced-motion: reduce)` that disables nonessential transitions. Color is never the only status carrier.

- [ ] **Step 4: Run parity, theme, semantic, and build checks**

```powershell
cargo test --manifest-path rust/Cargo.toml locale::tests -- --nocapture
pnpm --dir apps/desktop-tauri run check-locale
pnpm --dir apps/desktop-tauri exec vitest run src/i18n/keys.test.ts src/hooks/useTheme.test.ts src/surfaces/TrayPanel.test.tsx src/surfaces/Settings.test.ts
pnpm --dir apps/desktop-tauri run build
```

Expected: exact locale parity, system fallback, explicit theme, reduced-motion CSS, accessible roles/names, and production build pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/Cargo.toml Cargo.lock rust/src/locale rust/src/locale.rs rust/src/settings/types.rs apps/desktop-tauri/src/i18n apps/desktop-tauri/scripts/check-locale-drift.mjs apps/desktop-tauri/src/hooks/useTheme.ts apps/desktop-tauri/src/hooks/useTheme.test.ts apps/desktop-tauri/src/styles.css
git commit -m "Localize and theme the V1 interface"
```

### Task 8: Prove flyout positioning, DPI, keyboard, and native tray interaction

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/shell/flyout_window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/settings_window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/position.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/window_positioner.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Rewrite: `docs/WINDOWS_PROOF.md`
- Create: `docs/verification/windows/${ExecutionDate}/cua-observations.md`
- Create: selected redacted screenshots under `docs/verification/windows/${ExecutionDate}/screenshots/`

**Interfaces:**
- Consumes: fixed proof-state enum and existing monitor/taskbar geometry primitives.
- Produces: credential-free reproducible proof routes and recorded Windows observations.

For this task, set `$ExecutionDate = Get-Date -Format 'yyyy-MM-dd'` before creating the evidence directory and use that same value for every observation and screenshot path.

- [ ] **Step 1: Write failing proof-mode whitelist and work-area tests**

```rust
#[test]
fn proof_modes_are_fixed_and_secret_free() {
    assert_eq!(ProofScenario::ALL_NAMES, [
        "trayPanel:ready", "trayPanel:stale", "trayPanel:error", "trayPanel:api",
        "trayPanel:profiles", "settings:general", "settings:providers",
        "settings:advanced", "settings:about",
    ]);
}

#[test]
fn flyout_rect_is_clamped_inside_scaled_work_area() {
    let rect = place_flyout(tray_bottom_right_at_200_percent());
    assert!(tray_bottom_right_at_200_percent().work_area.contains(rect));
}
```

- [ ] **Step 2: Run geometry/proof tests and verify legacy scenario failure**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_modes_are_fixed_and_secret_free
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml flyout_rect_is_clamped_inside_scaled_work_area
```

Expected: FAIL until legacy proof modes and dimensions are replaced.

- [ ] **Step 3: Implement fixed proof states and work-area clamping**

Use a 400×520 logical-pixel target; clamp physical bounds to current monitor work area with an 8-pixel inset and make the inner panel scroll rather than escape the work area. Account for taskbar at all four edges and monitor scale from 1.0 through 2.0. Respect Windows animations-off and reduced motion. Proof fixtures contain labels `Current CLI`, `Work`, `Personal`, no email, and synthetic percentages/times/errors only.

- [ ] **Step 4: Build fresh, close the old instance, and drive CUA**

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
Get-Process -Name codex-barbar -ErrorAction SilentlyContinue | Stop-Process -Force
$env:CODEXBAR_PROOF_MODE = 'trayPanel:ready'
$DesktopExe = @(
    '.\target\debug\codex-barbar.exe',
    '.\target\x86_64-pc-windows-msvc\debug\codex-barbar.exe'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $DesktopExe) { throw 'Fresh codex-barbar.exe was not found' }
Start-Process -FilePath $DesktopExe -WindowStyle Hidden
$cua = Join-Path $env:LOCALAPPDATA 'Programs\Cua\cua-driver\bin\cua-driver.exe'
& $cua call list_windows '{}'
```

Use returned window IDs for `get_window_state`, clicks, Tab/Enter/Space/Escape, and screenshots. Capture ready 100%, stale 200%, error, profile switch, General, Accounts, Advanced, and About. Separately verify left click toggles the flyout, right click opens native menu without opening flyout, keyboard-only completion, long Chinese/English text, four taskbar edges, two monitors, 100/150/200% DPI, animations off, and theme auto without cross-WebView contamination.

- [ ] **Step 5: Run Phase-3 automated verification and record CUA evidence**

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
git diff --check
```

Expected: automated commands exit `0`; CUA observation file maps every manual matrix row to pass/fail and screenshot path.

- [ ] **Step 6: Commit**

```powershell
git add apps/desktop-tauri/src-tauri/src/shell apps/desktop-tauri/src-tauri/src/window_positioner.rs apps/desktop-tauri/src-tauri/src/proof_harness.rs apps/desktop-tauri/src-tauri/src/main.rs docs/WINDOWS_PROOF.md docs/verification/windows
git commit -m "Prove Windows tray interactions"
```

## Phase 3 Exit Gate

- Main flyout order, values, cache/error separation, countdowns, and fixed recovery actions match the approved Spec.
- Dynamic icon uses minimum remaining with exact thresholds; tooltip is textual and secret-free; native menu order and checked profile submenu are fixed.
- CurrentCli is visibly immutable; Managed add/cancel/select/rename/re-login/remove works without token/key/cookie inputs.
- Settings values and defaults are exact; Codex path is resolver-validated; update action is manual-only and fixed URL.
- English/Simplified Chinese keys are complete; theme/reduced-motion/keyboard/screen-reader semantics pass.
- Fresh-build CUA covers left/right click, focus/Escape, taskbar edges, multi-monitor, 100–200% DPI, animations off, and WebView2 theme behavior.
