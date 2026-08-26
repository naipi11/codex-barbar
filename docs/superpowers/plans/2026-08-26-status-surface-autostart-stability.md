# Status Surface and Autostart Stability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the float ball/taskbar state live through settings and Windows shell transitions, and make Windows login startup actually register and honor the background launch option.

**Architecture:** Preserve the Rust source of truth and existing surface controller. Narrow the frontend usage-cache invalidation key to usage data, make shell reconciliation restore/reassert instead of no-op for long-lived shell states, and reconcile HKCU Run at startup with a tested `--background` parser.

**Tech Stack:** Rust 2024, Tauri 2, React 18, TypeScript, Vitest, Win32/WinReg, CUA Driver.

**Spec:** `docs/superpowers/specs/2026-08-26-status-surface-autostart-stability-design.md`

## Global Constraints

- Real full-screen remains the only state allowed to suspend enabled status surfaces.
- Settings changes must not discard a fresher usage snapshot.
- Startup registration stays per-user HKCU Run and uses an absolute quoted `codex-barbar.exe --background` command.
- No private Codex endpoints, cookies, tokens, new dependencies, or raw paths in React/logs.
- UI and Windows behavior require fresh CUA proof after a rebuild; unit tests alone are insufficient.

---

### Task 1: Prove the settings/usage cache regression

**Files:**
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx`
- Modify: `apps/desktop-tauri/src/hooks/useProfileUsage.test.tsx` if a lower-level assertion is clearer
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.test.tsx` for the rendered band regression

**Interfaces:**
- Consumes: existing `events.settingsChanged` and `events.profileUsageStateChanged` harnesses.
- Produces: a failing test that distinguishes a settings-only update from a usage update.

- [ ] **Step 1: Write the failing test**
  Emit a fresh selected-profile usage state with a known green/high band, then emit a settings event changing float-ball opacity/glow. Assert the status surface still reports the same metric, freshness/trust, and band instead of `unknown`.
- [ ] **Step 2: Run the focused test and verify RED**
  Run `pnpm --dir apps/desktop-tauri test -- src/hooks/useStatusSurface.test.tsx src/surfaces/FloatBall.test.tsx`.
  Expected: the new assertion fails because the settings event resets the usage cache.
- [ ] **Step 3: Commit the test-only RED checkpoint**
  `git add apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx apps/desktop-tauri/src/hooks/useProfileUsage.test.tsx apps/desktop-tauri/src/surfaces/FloatBall.test.tsx && git commit -m "Test settings changes preserve live usage state"`

### Task 2: Keep live usage state during presentation updates

**Files:**
- Modify: `apps/desktop-tauri/src/hooks/useProfileUsage.ts`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx` only if the RED test needs fixture cleanup

**Interfaces:**
- Consumes: `BootstrapStateDto` and existing usage event reducers.
- Produces: a usage cache key based only on `profiles`, `selectedProfileId`, and `usageByProfile`.

- [ ] **Step 1: Implement the minimal key narrowing**
  Replace the whole-bootstrap JSON key with a stable object containing only usage-bearing fields. Keep settings changes in the surrounding hook so CSS/presentation state still updates immediately.
- [ ] **Step 2: Run the focused test and verify GREEN**
  Run the same Vitest command from Task 1. Expected: all focused tests pass, including the new settings-after-usage sequence.
- [ ] **Step 3: Commit**
  `git add apps/desktop-tauri/src/hooks/useProfileUsage.ts apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx apps/desktop-tauri/src/surfaces/FloatBall.test.tsx && git commit -m "Preserve usage cache across settings events"`

### Task 3: Prove shell-transient reconciliation behavior

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs` tests
- Modify: `apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs` tests only if a distinct desktop classification is required

**Interfaces:**
- Consumes: `ForegroundClass`, `reduce_surface_phase`, and `ReconcileAction`.
- Produces: failing reducer tests for long-lived shell/desktop state and real fullscreen suspension.

- [ ] **Step 1: Write the failing tests**
  Add cases that hold `ShellTransient` across periodic reconciliation and assert a restore/reassert action, while `RealFullscreen` still asserts `Suspend`.
- [ ] **Step 2: Run the focused Rust test and verify RED**
  Run `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces`.
  Expected: the shell-transient assertion fails against the current `KeepVisible`/no-op path.
- [ ] **Step 3: Commit the test-only RED checkpoint**
  `git add apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs && git commit -m "Test shell transitions restore status surfaces"`

### Task 4: Restore and reassert surfaces without stealing focus

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs` only if desktop and transient states must be separated

**Interfaces:**
- Consumes: existing `restore_after_shell`, `reassert_topmost`, `hide_for_fullscreen`, and saved geometry APIs.
- Produces: idempotent reconciliation that restores/reasserts enabled surfaces during shell/desktop states.

- [ ] **Step 1: Implement the minimal reducer/reconciliation fix**
  Remove the shell-transient no-op. On periodic/shell reconciliation call the existing non-activating restore/reassert path; keep `RealFullscreen` on the suspend path and preserve enabled flags/geometry.
- [ ] **Step 2: Run the focused Rust tests and verify GREEN**
  Run the Task 3 command and then `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces -- --nocapture`.
- [ ] **Step 3: Commit**
  `git add apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs && git commit -m "Reassert status surfaces across Windows shell states"`

### Task 5: Prove startup registration and background parsing

**Files:**
- Modify: `rust/src/platform/windows/autostart.rs` tests
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs` tests or a new small `startup_options.rs` module test
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs` tests if startup reconciliation is exposed there

**Interfaces:**
- Consumes: `autostart::command_for_executable`, current settings repository, and process arguments.
- Produces: failing tests for startup reconciliation and `--background` parsing.

- [ ] **Step 1: Write failing tests**
  Assert `--background` is recognized, normal launch remains foreground, and default-enabled startup calls the registration boundary even without a settings patch. Keep registry access behind a small Windows boundary so non-Windows tests remain deterministic.
- [ ] **Step 2: Run focused tests and verify RED**
  Run `cargo test --manifest-path rust/Cargo.toml autostart` and `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml startup`.
  Expected: background/reconciliation assertions fail before implementation.
- [ ] **Step 3: Commit test-only RED checkpoint**
  `git add rust/src/platform/windows/autostart.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs && git commit -m "Test Windows autostart reconciliation"`

### Task 6: Reconcile HKCU Run and honor `--background`

**Files:**
- Modify: `rust/src/platform/windows/autostart.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs` or create `apps/desktop-tauri/src-tauri/src/startup_options.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs` only to reuse the shared reconciliation boundary

**Interfaces:**
- Consumes: `StartupOptions::from_args` and `autostart::set_enabled`.
- Produces: idempotent startup registration and background launch behavior with no panel activation.

- [ ] **Step 1: Implement minimal startup reconciliation**
  Parse `--background`, load settings during setup, and call the existing registry writer when `start_at_login` is enabled. Keep failures non-fatal but record a diagnostic warning. Reuse the same path for toggle updates.
- [ ] **Step 2: Run focused tests and verify GREEN**
  Run the Task 5 commands. Expected: all autostart/startup tests pass.
- [ ] **Step 3: Commit**
  `git add rust/src/platform/windows/autostart.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/startup_options.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs && git commit -m "Make Windows login autostart reliable"`

### Task 7: Full verification and fresh Windows proof

**Files:**
- Modify: `docs/WINDOWS_ACCEPTANCE.md` if the acceptance wording needs to document the repaired login/background semantics
- Create: `tmp-cua-proof/2026-08-26-status-surface-autostart/PROOF.md` locally only; do not add it to Git

- [ ] **Step 1: Run full automated gates**
  Run `cargo fmt --all -- --check`, both Rust clippy suites, both Rust test suites, `CI=true pnpm --dir apps/desktop-tauri test`, `CI=true pnpm --dir apps/desktop-tauri run build`, boundary/release policy scripts, and `CI=true pnpm --dir apps/desktop-tauri run tauri:build:debug`.
- [ ] **Step 2: Rebuild and launch the fresh debug binary**
  Stop only the test instance, set `CODEXBAR_PROOF_MODE` for the target surface, launch the freshly built binary, and verify the window list before interacting.
- [ ] **Step 3: Run the CUA regression matrix**
  Capture before/after screenshots for opacity/glow changes, taskbar click, Start, Explorer, desktop, and real full-screen. Read HKCU Run after startup and verify `--background` does not open the panel. Record only sanitized evidence.
- [ ] **Step 4: Commit docs/evidence metadata if needed**
  Do not add screenshots or local account data. Commit only acceptance-document changes.
- [ ] **Step 5: Final review**
  Run `git diff --check`, verify no secrets and no untracked release artifacts, then use the finishing/verification skills before any release claim.
