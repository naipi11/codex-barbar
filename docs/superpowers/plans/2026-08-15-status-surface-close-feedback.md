# Durable Status Surface Close Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve taskbar and float-ball close failure feedback across native WebView destruction and rollback recreation, then re-run every blocked Windows and release gate.

**Architecture:** Keep the existing runtime-first transition, but add a process-local feedback latch inside `StatusSurfaceState`. The controller sets the latch before rollback recreates a window; bootstrap restores it for a new WebView, while a frozen event updates a surviving WebView. `useStatusSurface` becomes the sole frontend feedback owner for both surfaces.

**Tech Stack:** Rust 2024, Tauri 2.10, React 18, TypeScript, Vitest 3, WebView2 CDP, Windows Computer Use/DPI-aware Win32, pnpm 10.18.1, NSIS.

## Global Constraints

- The approved source of truth is `docs/superpowers/specs/2026-08-15-status-surface-close-feedback-design.md`.
- Keep the existing runtime-first settings transaction; do not switch to persistence-first or introduce a two-phase hide/finalize lifecycle.
- Feedback is process-local only. Do not add it to `AppSettings`, `SettingsPatch`, `SettingsRepository`, SQLite, or `settings.json`.
- Do not add a Tauri command, capability, permission, dependency, database column, package-manager artifact, or native whole-window opacity.
- Never expose raw Rust, SQLite, Tauri, WebView, path, account, token, cookie, or protocol data through the DTO, event, logs, documents, or screenshots.
- Preserve taskbar measurement-first close, exact 104..318 logical width range, 318 fallback, 40 height, hidden 318x40 helper, exact measurement caller authorization, 2-second reconciliation, and identity-safe deferred cleanup.
- Preserve float-ball `.theme(Some(tauri::Theme::Dark))`, saved collapsed position, drag threshold, expansion dimensions, and monitor behavior.
- Preserve runtime-only mutual exclusion for `taskbar-status:*` and `float-ball:*` proof scenarios.
- Use repository-pinned pnpm 10.18.1 and Node 20-compatible commands. Do not create npm or yarn lockfiles.
- Every production behavior change uses RED -> minimal GREEN -> refactor. Each Task receives a fresh independent spec/code-quality review before the next Task.
- Do not stage or modify the seven protected untracked PNG files under `docs/verification/windows/2026-08-14/screenshots`.
- Any failed or inconclusive required Windows gate stops Task 6, release, installation, and push.
- Passing release/install gates does not authorize a push; pushing remains a separate user action.

---

### Task 1: Add the Native Feedback Latch and Transaction Ordering

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`
- Create: `.superpowers/sdd/2026-08-15-status-surface-close-feedback/task-1-report.md`

**Interfaces:**
- Produces: `StatusSurfaceFeedback`, `StatusSurfaceFeedbackState`, `StatusSurfaceFeedbackState::close_failed`, and `StatusSurfaceFeedbackState::set_close_failed` in `controller.rs`.
- Extends: `SurfaceRuntime::set_close_failed(surface, close_failed)`.
- Stores: `StatusSurfaceState.feedback: StatusSurfaceFeedbackState`.
- Preserves: `transition` return type, stable error codes, manager methods, window lifecycle, and settings store contract.

- [ ] **Step 1: Write RED feedback state and isolation tests**

Add literal state tests in `controller.rs`:

```rust
#[test]
fn feedback_is_process_local_and_surface_isolated() {
    let mut feedback = StatusSurfaceFeedbackState::default();
    assert!(!feedback.close_failed(StatusSurfaceKind::TaskbarStatus));
    assert!(!feedback.close_failed(StatusSurfaceKind::FloatBall));

    feedback.set_close_failed(StatusSurfaceKind::TaskbarStatus, true);
    assert!(feedback.close_failed(StatusSurfaceKind::TaskbarStatus));
    assert!(!feedback.close_failed(StatusSurfaceKind::FloatBall));
}
```

Extend `FakeRuntime` with one ordered action log:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeAction {
    Feedback(StatusSurfaceKind, bool),
    Apply(StatusSurfaceKind, bool),
    Force(StatusSurfaceKind, bool),
}
```

Implement the future trait method in the fake by pushing
`RuntimeAction::Feedback(surface, close_failed)`.

- [ ] **Step 2: Write RED transaction-order tests**

Add tests with hand-derived action sequences:

```rust
#[test]
fn persistence_failure_latches_close_feedback_before_runtime_rollback() {
    let mut runtime = FakeRuntime::enabled();
    let store = FakeStore::with_settings(settings(true, false)).failing_save();

    let error = transition(
        &mut runtime,
        &store,
        StatusSurfaceKind::TaskbarStatus,
        false,
    )
    .unwrap_err();

    assert_eq!(error, "STATUS_SURFACE_SETTINGS_SAVE_FAILED");
    assert_eq!(
        runtime.actions(),
        &[
            RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
            RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
            RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, true),
            RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
        ]
    );
}

#[test]
fn successful_disable_clears_feedback_and_does_not_relatch() {
    let mut runtime = FakeRuntime::enabled();
    let store = FakeStore::with_settings(settings(true, false));

    transition(
        &mut runtime,
        &store,
        StatusSurfaceKind::TaskbarStatus,
        false,
    )
    .unwrap();

    assert_eq!(
        runtime.actions(),
        &[
            RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
            RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
        ]
    );
}
```

Add equivalent literal assertions for:

- runtime disable failure: clear -> apply false -> latch true;
- rollback failure: latch stays true before `Force(surface, true)`;
- enable failure: clear -> apply true, with no `Feedback(..., true)`;
- one surface transition does not change peer feedback.

- [ ] **Step 3: Run RED and record the expected failures**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::controller -- --nocapture
```

Expected: compile failures for missing feedback types/methods and then action
sequence failures until the controller writes feedback in the required order.

- [ ] **Step 4: Implement the minimal feedback types**

In `controller.rs` add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusSurfaceFeedback {
    #[default]
    None,
    CloseFailed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusSurfaceFeedbackState {
    taskbar_status: StatusSurfaceFeedback,
    float_ball: StatusSurfaceFeedback,
}

impl StatusSurfaceFeedbackState {
    pub fn close_failed(self, surface: StatusSurfaceKind) -> bool {
        match surface {
            StatusSurfaceKind::TaskbarStatus => {
                self.taskbar_status == StatusSurfaceFeedback::CloseFailed
            }
            StatusSurfaceKind::FloatBall => {
                self.float_ball == StatusSurfaceFeedback::CloseFailed
            }
        }
    }

    pub fn set_close_failed(&mut self, surface: StatusSurfaceKind, close_failed: bool) {
        let value = if close_failed {
            StatusSurfaceFeedback::CloseFailed
        } else {
            StatusSurfaceFeedback::None
        };
        match surface {
            StatusSurfaceKind::TaskbarStatus => self.taskbar_status = value,
            StatusSurfaceKind::FloatBall => self.float_ball = value,
        }
    }
}
```

Extend the runtime trait:

```rust
pub trait SurfaceRuntime {
    fn apply(&mut self, surface: StatusSurfaceKind, enabled: bool) -> Result<(), String>;
    fn force_enabled(&mut self, surface: StatusSurfaceKind, enabled: bool);
    fn set_close_failed(&mut self, surface: StatusSurfaceKind, close_failed: bool);
}
```

Add `feedback` to the existing state in `status_surfaces.rs`:

```rust
#[derive(Default)]
pub struct StatusSurfaceState {
    pub taskbar: crate::taskbar_overlay::TaskbarOverlay,
    pub float_ball: crate::float_ball::FloatBall,
    pub feedback: controller::StatusSurfaceFeedbackState,
}
```

Implement the Tauri runtime method by mutating `self.state.feedback`.

- [ ] **Step 5: Implement the exact transition semantics**

At the start of every attempt:

```rust
runtime.set_close_failed(surface, false);
```

On `runtime.apply(surface, enabled)` failure, latch only a disable error before
returning or compensating:

```rust
if let Err(error) = runtime.apply(surface, enabled) {
    if !enabled {
        runtime.set_close_failed(surface, true);
    }
    // retain the existing direction-aware rollback branches verbatim
}
```

On settings write failure, latch before rollback only for a disable request:

```rust
Err(error) => {
    if !enabled {
        runtime.set_close_failed(surface, true);
    }
    if runtime.apply(surface, previous_enabled).is_err() {
        runtime.force_enabled(surface, previous_enabled);
        return Err("STATUS_SURFACE_ROLLBACK_FAILED".to_string());
    }
    Err(error)
}
```

Do not change `previous_enabled == enabled`, force-enabled, or stable error
behavior beyond the required feedback actions.

- [ ] **Step 6: Run GREEN and native regressions**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::controller -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 7: Report and commit Task 1**

Write RED/GREEN commands, counts, action ordering, and file scope to the Task 1
report. Commit only the two Rust files:

```powershell
git add apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs `
        apps/desktop-tauri/src-tauri/src/status_surfaces.rs
git diff --cached --check
git commit -m "Latch status close feedback"
```

---

### Task 2: Expose Feedback Through Bootstrap and a Frozen Event

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/events.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Create: `.superpowers/sdd/2026-08-15-status-surface-close-feedback/task-2-report.md`

**Interfaces:**
- Consumes: `StatusSurfaceFeedbackState::close_failed` from Task 1.
- Produces: `StatusSurfaceFeedbackDto`, `StatusSurfaceFeedbackChangedDto`, bootstrap field `status_surface_feedback`, and event `STATUS_SURFACE_FEEDBACK_CHANGED`.
- Preserves: the existing `get_bootstrap_state` command name, command registry, proof account fixture, settings event, and nonfatal event semantics.

- [ ] **Step 1: Write RED bridge serialization tests**

Add DTOs to test construction before defining them:

```rust
#[test]
fn feedback_serialization_uses_frozen_safe_wire_names() {
    let value = serde_json::to_value(StatusSurfaceFeedbackChangedDto {
        surface: StatusSurfaceKind::TaskbarStatus,
        close_failed: true,
    })
    .unwrap();
    assert_eq!(value["surface"], "taskbarStatus");
    assert_eq!(value["closeFailed"], true);
    let text = value.to_string().to_ascii_lowercase();
    for forbidden in ["sqlite", "webview", "path", "token", "error"] {
        assert!(!text.contains(forbidden), "leaked {forbidden}: {text}");
    }
}
```

Update `bootstrap_serialization_has_only_frozen_top_level_fields` to construct:

```rust
status_surface_feedback: StatusSurfaceFeedbackDto {
    taskbar_status_close_failed: true,
    float_ball_close_failed: false,
},
```

and expect the exact extra key `statusSurfaceFeedback`.

- [ ] **Step 2: Write RED bootstrap snapshot tests**

Change `bootstrap_from_state` tests to pass a literal feedback DTO and assert
the returned values. Add a proof bootstrap assertion that both fields default
to `false`.

Add a pure snapshot test in `status_surfaces.rs` or `controller.rs` that maps
native taskbar/float feedback independently into the DTO without repository
access.

- [ ] **Step 3: Write RED event completion tests**

Extract a production-used nonfatal emitter seam:

```rust
fn emit_feedback_with(
    payload: &crate::commands::StatusSurfaceFeedbackChangedDto,
    emit: impl FnOnce(&crate::commands::StatusSurfaceFeedbackChangedDto) -> Result<(), ()>,
) {
    if emit(payload).is_err() {
        tracing::warn!(
            code = "STATUS_SURFACE_FEEDBACK_EVENT_FAILED",
            "status surface feedback event was not delivered"
        );
    }
}
```

Tests must prove a failing emitter does not replace the transition result and
that success/failure payloads contain only `surface` and `closeFailed`.

- [ ] **Step 4: Run RED**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml events -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::controller -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
```

Expected: missing DTO/event/bootstrap fields and missing emitter seam.

- [ ] **Step 5: Implement frozen DTOs and event name**

In `commands/bridge.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSurfaceFeedbackDto {
    pub taskbar_status_close_failed: bool,
    pub float_ball_close_failed: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSurfaceFeedbackChangedDto {
    pub surface: crate::status_surfaces::controller::StatusSurfaceKind,
    pub close_failed: bool,
}
```

Because the event serializes `surface`, extend its existing derive without
changing wire names:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusSurfaceKind {
    TaskbarStatus,
    FloatBall,
}
```

Add `status_surface_feedback: StatusSurfaceFeedbackDto` to `BootstrapDto`.

In `events.rs` add:

```rust
pub const STATUS_SURFACE_FEEDBACK_CHANGED: &str = "status-surface-feedback-changed";
```

Add it to `ALL`; do not add it to `TRAY_REBUILD_EVENTS`.

- [ ] **Step 6: Attach feedback to production and proof bootstrap**

Keep `bootstrap_from_state` pure by accepting a feedback DTO:

```rust
pub(crate) fn bootstrap_from_state(
    state: &AppState,
    status_surface_feedback: StatusSurfaceFeedbackDto,
) -> Result<BootstrapDto, String>
```

Assign that DTO in both normal and proof returns. In `get_bootstrap_state`, do
not hold both mutexes simultaneously:

```rust
let mut bootstrap = {
    let guard = state.lock().map_err(|_| "BOOTSTRAP_STATE_UNAVAILABLE".to_string())?;
    bootstrap_from_state(&guard, StatusSurfaceFeedbackDto::default())?
};
let feedback = {
    let guard = status_surfaces
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    StatusSurfaceFeedbackDto {
        taskbar_status_close_failed: guard
            .feedback
            .close_failed(StatusSurfaceKind::TaskbarStatus),
        float_ball_close_failed: guard
            .feedback
            .close_failed(StatusSurfaceKind::FloatBall),
    }
};
bootstrap.status_surface_feedback = feedback;
Ok(bootstrap)
```

The command receives both managed states through its Tauri parameters. Update
all internal calls/tests with explicit default feedback. Update
`proof_harness::synthetic_bootstrap` with `StatusSurfaceFeedbackDto::default()`.

- [ ] **Step 7: Emit the final feedback result for every controller exit**

In `set_enabled_and_emit`:

1. clear the target feedback before repository lookup so an early repository
   failure cannot leave stale UI;
2. if repository lookup fails for a disable request, latch the target failure;
3. call the existing transition;
4. snapshot the target feedback after transition/rollback;
5. emit `STATUS_SURFACE_FEEDBACK_CHANGED` through `emit_feedback_with`;
6. on transition success, emit the existing settings event and return settings;
7. on transition failure, return the original stable error unchanged.

Use helper functions that lock only `StatusSurfaceState`; do not hold
`AppState` and `StatusSurfaceState` together. Do not add a repository or raw
error to the event payload.

- [ ] **Step 8: Run GREEN and full Tauri checks**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml events -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 9: Report and commit Task 2**

```powershell
git add apps/desktop-tauri/src-tauri/src/commands/bridge.rs `
        apps/desktop-tauri/src-tauri/src/events.rs `
        apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs `
        apps/desktop-tauri/src-tauri/src/proof_harness.rs
git diff --cached --check
git commit -m "Bridge status close feedback"
```

---

### Task 3: Make `useStatusSurface` the Frontend Feedback Owner

**Files:**
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts`
- Modify: `apps/desktop-tauri/src/test/profileUsageFixtures.ts`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.ts`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx`
- Modify: `apps/desktop-tauri/src/types/bridge.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts`
- Create: `.superpowers/sdd/2026-08-15-status-surface-close-feedback/task-3-report.md`

**Interfaces:**
- Consumes: Task 2 wire fields and event name.
- Produces: `StatusSurfaceFeedbackDto`, `StatusSurfaceFeedbackChangedDto`, and `UseStatusSurfaceResult.closeFailedBySurface: Record<StatusSurfaceKind, boolean>`.
- Preserves: typed disable command arguments and rejection, settings-event race handling, usage view model, expansion command, and account isolation.

- [ ] **Step 1: Write RED bridge contract tests**

Extend `types/bridge.test.ts` to assert:

```ts
expect(events.statusSurfaceFeedbackChanged).toBe(
  "status-surface-feedback-changed",
);
expect(bootstrap.statusSurfaceFeedback).toEqual({
  taskbarStatusCloseFailed: false,
  floatBallCloseFailed: false,
});
```

Update the complete fixture, not a partial cast.

- [ ] **Step 2: Write RED hook bootstrap and event tests**

Add exact tests:

```tsx
it("restores both close feedback values from bootstrap", async () => {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.statusSurfaceFeedback = {
    taskbarStatusCloseFailed: true,
    floatBallCloseFailed: false,
  };
  invokeMock.mockResolvedValue(bootstrap);
  const { result } = renderHook(() => useStatusSurface());

  await waitFor(() =>
    expect(result.current.closeFailedBySurface).toEqual({
      taskbarStatus: true,
      floatBall: false,
    }),
  );
});

it("applies feedback events only to their target surface", async () => {
  const bootstrap = bootstrapWithTwoProfiles();
  invokeMock.mockResolvedValue(bootstrap);
  const { result } = renderHook(() => useStatusSurface());
  await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

  act(() =>
    eventHarness.emit(events.statusSurfaceFeedbackChanged, {
      surface: "floatBall",
      closeFailed: true,
    }),
  );

  expect(result.current.closeFailedBySurface).toEqual({
    taskbarStatus: false,
    floatBall: true,
  });
});
```

Add a deferred-bootstrap race test: emit `taskbarStatus=true` before bootstrap
resolves with both false; the emitted taskbar value must win while the bootstrap
float value remains authoritative.

- [ ] **Step 3: Write RED retry and surviving-page tests**

```tsx
it("clears only the retry target and restores it when the command rejects", async () => {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.statusSurfaceFeedback = {
    taskbarStatusCloseFailed: true,
    floatBallCloseFailed: true,
  };
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_bootstrap_state") return bootstrap;
    if (command === "set_status_surface_enabled") {
      throw new Error("STATUS_SURFACE_SETTINGS_SAVE_FAILED");
    }
    return undefined;
  });
  const { result } = renderHook(() => useStatusSurface());
  await waitFor(() => expect(result.current.bootstrap).not.toBeNull());

  await act(async () => {
    await expect(
      result.current.disableSurface("taskbarStatus"),
    ).rejects.toThrow("STATUS_SURFACE_SETTINGS_SAVE_FAILED");
  });

  expect(result.current.closeFailedBySurface).toEqual({
    taskbarStatus: true,
    floatBall: true,
  });
});
```

Use a deferred command promise to assert the target is false while the retry
is pending and the peer remains unchanged.

- [ ] **Step 4: Run RED**

```powershell
$pnpmExact = "$env:LOCALAPPDATA\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs"
$env:CI = "true"
node $pnpmExact --dir apps/desktop-tauri exec vitest run src/types/bridge.test.ts src/hooks/useStatusSurface.test.tsx --reporter=verbose
```

Expected: missing DTO/event fields and missing `closeFailedBySurface`.

- [ ] **Step 5: Implement exact bridge types and complete fixtures**

In `types/bridge.ts` add:

```ts
export interface StatusSurfaceFeedbackDto {
  taskbarStatusCloseFailed: boolean;
  floatBallCloseFailed: boolean;
}

export interface StatusSurfaceFeedbackChangedDto {
  surface: StatusSurfaceKind;
  closeFailed: boolean;
}
```

Add `statusSurfaceFeedback: StatusSurfaceFeedbackDto` to `BootstrapDto`.
Update every complete bootstrap fixture with:

```ts
statusSurfaceFeedback: {
  taskbarStatusCloseFailed: false,
  floatBallCloseFailed: false,
},
```

Add the same literal value to `EMPTY_BOOTSTRAP` in `useStatusSurface.ts`. Add
the hook result value to the typed `surfaceFrom` test fixture in
`taskbarStatusPresentation.test.ts`:

```ts
closeFailedBySurface: {
  taskbarStatus: false,
  floatBall: false,
},
```

Add the exact event key to `lib/tauri.ts`.

- [ ] **Step 6: Implement race-safe hook ownership**

Add to `UseStatusSurfaceResult`:

```ts
closeFailedBySurface: Record<StatusSurfaceKind, boolean>;
```

Use one state and one partial early-event ref:

```ts
const EMPTY_CLOSE_FEEDBACK: Record<StatusSurfaceKind, boolean> = {
  taskbarStatus: false,
  floatBall: false,
};

const [closeFailedBySurface, setCloseFailedBySurface] = useState(
  EMPTY_CLOSE_FEEDBACK,
);
const latestCloseFeedback = useRef<
  Partial<Record<StatusSurfaceKind, boolean>>
>({});
```

Register a `listen<StatusSurfaceFeedbackChangedDto>` effect. Each event updates
only its target in the ref and state. When bootstrap resolves, combine exact
bootstrap fields with the partial early-event overlay; never use a two-false
default to overwrite an authoritative peer field.

Implement `disableSurface` as:

```ts
const disableSurface = useCallback(async (surface: StatusSurfaceKind) => {
  setCloseFailedBySurface((current) => ({ ...current, [surface]: false }));
  try {
    return await setStatusSurfaceEnabled(surface, false);
  } catch (error) {
    setCloseFailedBySurface((current) => ({ ...current, [surface]: true }));
    throw error;
  }
}, []);
```

Return `closeFailedBySurface` from the hook. Keep the settings event and usage
state paths unchanged.

- [ ] **Step 7: Run GREEN and frontend regression**

```powershell
node $pnpmExact --dir apps/desktop-tauri exec vitest run src/types/bridge.test.ts src/hooks/useStatusSurface.test.tsx --reporter=verbose
node $pnpmExact --dir apps/desktop-tauri test
node $pnpmExact --dir apps/desktop-tauri run build
git diff --check
```

- [ ] **Step 8: Report and commit Task 3**

```powershell
git add apps/desktop-tauri/src/types/bridge.ts `
        apps/desktop-tauri/src/lib/tauri.ts `
        apps/desktop-tauri/src/test/profileUsageFixtures.ts `
        apps/desktop-tauri/src/hooks/useStatusSurface.ts `
        apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx `
        apps/desktop-tauri/src/types/bridge.test.ts `
        apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts
git diff --cached --check
git commit -m "Own close feedback in status hook"
```

---

### Task 4: Render Shared Feedback in Both Status Surfaces

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.test.tsx`
- Modify: `docs/WINDOWS_PROOF.md`
- Create: `.superpowers/sdd/2026-08-15-status-surface-close-feedback/task-4-report.md`

**Interfaces:**
- Consumes: `closeFailedBySurface` and the rejecting `disableSurface` from Task 3.
- Produces: taskbar red retry UI and float-ball close error from shared feedback.
- Preserves: taskbar geometry, CSS animation/reduced motion, float expansion error, drag/click behavior, and both surface presentations.

- [ ] **Step 1: Write RED taskbar bootstrap/remount tests**

Add a test whose complete bootstrap has
`taskbarStatusCloseFailed: true`. Assert the actual rendered root has:

```tsx
expect(close).toHaveAttribute("data-error", "true");
expect(close).toHaveAttribute("title", "关闭失败，点击重试");
expect(screen.getByRole("status")).toHaveTextContent("关闭失败，点击重试");
expect(within(visible).getByText("5H 42%")).toBeInTheDocument();
expect(within(visible).getByText("周 61%")).toBeInTheDocument();
```

The test must unmount and render again from the same bootstrap to prove the
feedback is not component-local.

- [ ] **Step 2: Write RED taskbar retry test using the shared hook path**

Retain the existing deferred retry assertions, but drive the error through the
complete bootstrap/event/command contract. Assert target clear while pending,
peer isolation, and error restoration on rejection. Do not mock
`TaskbarStatusContents`.

- [ ] **Step 3: Write RED float-ball shared feedback tests**

Add a complete-bootstrap test:

```tsx
it("restores close feedback after the float window is recreated", async () => {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.statusSurfaceFeedback.floatBallCloseFailed = true;
  invokeMock.mockResolvedValue(bootstrap);
  const first = render(<FloatBall />);

  expect(await screen.findByRole("status")).toHaveTextContent(
    "关闭失败，请重试",
  );
  first.unmount();
  render(<FloatBall />);
  expect(await screen.findByRole("status")).toHaveTextContent(
    "关闭失败，请重试",
  );
});
```

Add an expansion-error case where `expansionError` remains independently
rendered and a close-feedback event later becomes visible after it clears.

- [ ] **Step 4: Run RED**

```powershell
$pnpmExact = "$env:LOCALAPPDATA\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs"
$env:CI = "true"
node $pnpmExact --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx src/surfaces/FloatBall.test.tsx --reporter=verbose
```

Expected: recreated surfaces ignore bootstrap feedback because their local
state starts clear.

- [ ] **Step 5: Remove component-local close state**

In `TaskbarStatus.tsx`, remove `useState` and use:

```tsx
const closeFailed = surface.closeFailedBySurface.taskbarStatus;

const closeSurface = async (event: React.MouseEvent<HTMLButtonElement>) => {
  event.stopPropagation();
  await surface.disableSurface("taskbarStatus").catch(() => undefined);
};
```

In `FloatBall.tsx`, remove `closeError` state only. Keep expansion errors and
render:

```tsx
const closeError = surface.closeFailedBySurface.floatBall
  ? "关闭失败，请重试"
  : null;
```

The close handler cancels expansion work and awaits the shared hook command
with a swallowed, already-reflected rejection. Do not change pointer, drag,
geometry, or opacity code.

- [ ] **Step 6: Freeze the two locked retry contracts in `WINDOWS_PROOF.md`**

Add exact sections:

```text
Locked taskbar persistence retry:
  acquire and independently verify an exclusive writer;
  click the real close control;
  rebuilt visible/helper windows and persisted true prove rollback;
  rebuilt visible root must expose data-error=true, retry title, and live text;
  release must be observed before retry destroys both and persists false.

Locked float-ball persistence retry:
  acquire and independently verify an exclusive writer;
  click the real float close control;
  surviving or rebuilt native float window and persisted true prove rollback;
  close feedback must be visible after recreation;
  release must be observed before retry destroys the window and persists false.
```

- [ ] **Step 7: Run GREEN and full frontend checks**

```powershell
node $pnpmExact --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx src/surfaces/FloatBall.test.tsx src/hooks/useStatusSurface.test.tsx --reporter=verbose
node $pnpmExact --dir apps/desktop-tauri test
node $pnpmExact --dir apps/desktop-tauri run build
git diff --check
```

- [ ] **Step 8: Report and commit Task 4**

```powershell
git add apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx `
        apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx `
        apps/desktop-tauri/src/surfaces/FloatBall.tsx `
        apps/desktop-tauri/src/surfaces/FloatBall.test.tsx `
        docs/WINDOWS_PROOF.md
git diff --cached --check
git commit -m "Restore close feedback after window rebuild"
```

---

### Task 5: Re-run Full Source Gates and Windows Status-Surface Proof

**Files:**
- Create: `docs/verification/windows/2026-08-15/status-surface-close-feedback-observations.md`
- Create: `docs/verification/windows/2026-08-15/screenshots/close-feedback-taskbar-weekly.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/close-feedback-taskbar-opacity-0.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/close-feedback-taskbar-opacity-80.png`
- Create after locked-taskbar PASS: `docs/verification/windows/2026-08-15/screenshots/close-feedback-taskbar-locked.png`
- Create after taskbar PASS: `docs/verification/windows/2026-08-15/screenshots/close-feedback-float-collapsed.png`
- Create after taskbar PASS: `docs/verification/windows/2026-08-15/screenshots/close-feedback-float-expanded.png`
- Create after locked-float PASS: `docs/verification/windows/2026-08-15/screenshots/close-feedback-float-locked.png`
- Create: `.superpowers/sdd/2026-08-15-status-surface-close-feedback/task-5-report.md`

**Interfaces:**
- Consumes: Tasks 1–4 reviewed native latch, bridge, hook, and surface behavior.
- Produces: complete fresh source/Windows evidence and a binary GO/STOP decision for Task 6.

- [ ] **Step 1: Record a privacy-safe clean boundary**

Record HEAD, exact git status, debug/release/installed app-under-test counts,
four surface fields, and absence of an external writer. Do not print the full
settings document, account data, DevTools port, or private path.

- [ ] **Step 2: Run focused and complete source gates**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server
cargo test --manifest-path rust/Cargo.toml storage::settings_repository
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces
$pnpmExact = "$env:LOCALAPPDATA\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs"
$env:CI = "true"
node $pnpmExact --dir apps/desktop-tauri test
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
```

Record exact counts and final exit. Partial/timed-out output is not PASS.

- [ ] **Step 3: Build and hash a fresh debug binary**

Stop only exact app-under-test processes, then run:

```powershell
node $pnpmExact --dir apps/desktop-tauri run tauri:build:debug
Get-FileHash target\debug\codex-barbar.exe -Algorithm SHA256
```

Record hash, size, timestamp, and exact launched path in redacted form.

- [ ] **Step 4: Pass the decisive taskbar weekly and opacity gates**

Launch `taskbar-status:weekly`. Prove complete
`ProofU | 周 98% | 8/20 | ×`, no `5H`, hidden 318x40 helper, visible width
104..317 and height 40, peer isolation, and safe native placement.

In a clean `settings:general` run, prove actual root inline/computed alpha 0 and
0.8, stable text/geometry/HWND/target, zero width commands, different hashes,
and material screen-composited pixel change.

Any failure writes STOP and ends Task 5.

- [ ] **Step 5: Pass unlocked and locked taskbar close gates**

Prove unlocked frontend close and exact-root native `WM_CLOSE` first. Then:

1. re-enable through the typed command;
2. acquire an exclusive writer and verify a second writer is blocked;
3. click the real taskbar close control;
4. assert visible/helper runtime and persisted `true` rollback;
5. assert the rebuilt root has `data-error=true`, title and live text
   `关闭失败，点击重试`;
6. save only the valid red-state screen-composited capture;
7. release and independently observe the lock release;
8. retry and assert both windows/targets disappear and persisted becomes
   `false`.

- [ ] **Step 6: Pass the complete isolated float-ball gate**

Launch `float-ball:weekly` with taskbar peers absent/hidden. Prove native
visibility, weekly-only collapsed content, expanded rows, click-versus-drag,
safe geometry, 0/80 screen composition, close, Dark theme, and intended peer
WebView theme.

Repeat the verified exclusive-writer sequence for float close. The surviving
or rebuilt float UI must display `关闭失败，请重试`; after lock release, retry
must destroy it and persist `false`.

- [ ] **Step 7: Restore state, inspect evidence, and commit**

Restore all four fields, release the writer, stop only the debug app, and
confirm app-under-test/DevTools/writer counts are zero. Inspect every image for
identity, paths, secrets, and false claims.

```powershell
git add docs/verification/windows/2026-08-15/status-surface-close-feedback-observations.md `
        docs/verification/windows/2026-08-15/screenshots/close-feedback-*.png
git diff --cached --check
git commit -m "Verify durable status close feedback"
```

Task 6 is authorized only if every required item is PASS. Otherwise commit
truthful, privacy-safe evidence, report the exact STOP, and do not continue.

---

### Task 6: Verify Live Data, Build Release, and Install Current User

**Files:**
- Modify: `docs/verification/windows/2026-08-15/status-surface-close-feedback-observations.md`
- Create: `.superpowers/sdd/2026-08-15-status-surface-close-feedback/task-6-report.md`

**Interfaces:**
- Consumes: independently reviewed all-PASS Task 5 evidence.
- Produces: resolver/live-quota proof, production NSIS hash, current-user installation, post-install proof, and final release decision.

- [ ] **Step 1: Verify resolver and live quota without wrapper changes**

Record only SHA-256 and modification time of the existing wrapper before and
after. Run `codex --version`, the redacted App Server smoke, and a normal
refresh. Require verified installation class, detected version, signed-in
state, available limits, a new successful snapshot, and weekly period/value/
reset agreement at one observation time. Any mismatch invokes systematic
debugging and STOP.

- [ ] **Step 2: Build and verify production NSIS**

```powershell
$pnpmExact = "$env:LOCALAPPDATA\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs"
$env:CI = "true"
node $pnpmExact --dir apps/desktop-tauri run tauri:build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-artifacts.ps1
```

Require `target/release/bundle/nsis/codex-barbar_1.0.0_x64-setup.exe`, record
hash and size, and do not commit the installer.

- [ ] **Step 3: Install current-user without deleting data**

Use the repository current-user smoke or NSIS current-user silent mode. Do not
elevate and do not select data deletion. Verify install root
`%LOCALAPPDATA%\Programs\codex-barbar`, settings/account-cache survival,
uninstall registration, installed hash/timestamp, and that the running path is
the installed release rather than the worktree debug build.

- [ ] **Step 4: Repeat post-install source and Windows proof**

Stop debug processes. Re-run focused feedback/controller/bridge/frontend
tests. On the installed binary verify weekly taskbar/helper, live opacity,
unlocked close, locked taskbar retry, visible/interactable float ball, locked
float retry, Dark theme, and successful live quota refresh.

- [ ] **Step 5: Update and commit release evidence**

Append exact commands/counts, redacted resolver class/live quota, installer
hash, installed release path, state survival, and limitations.

```powershell
git add docs/verification/windows/2026-08-15/status-surface-close-feedback-observations.md
git diff --cached --check
git commit -m "Verify close feedback release"
```

- [ ] **Step 6: Run the final independent review**

Generate a review package from Spec commit `1c74f07a` through HEAD. The reviewer
must inspect the close-feedback spec, previous status-repair spec, transaction
ordering, feedback-before-rollback proof, bootstrap/event race handling,
settings atomicity, lifecycle invariants, privacy, Windows screenshots,
release artifact hash, installed path, and unrelated-file scope.

Completion requires:

```text
no Critical or Important findings
full local CI PASS
fresh Windows proof with no required inconclusive item
verified production NSIS and current-user installation
successful post-install live quota and both locked retry flows
```

Otherwise report the exact blocker and do not declare the project complete or
push the branch.
