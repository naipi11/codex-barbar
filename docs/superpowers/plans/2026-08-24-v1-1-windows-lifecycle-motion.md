# v1.1 Windows Lifecycle and Floating-Ball Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax - [ ] for tracking.

**Goal:** Eliminate persistent taskbar/floating-ball disappearance around Windows Shell interactions and make Fast/Thinking/Idle motion respond smoothly within a bounded time.

**Architecture:** Establish a sanitized native trace before changing behavior. A stateful controller then separates user intent, Shell transient state, and verified real full-screen. A native motion monitor emits typed changes to the floating-ball webview; a monotonic browser animation consumes the event without four-second polling or phase resets.

**Tech Stack:** Rust 2024, Tauri 2 events, existing raw user32 FFI style, Tokio, React 18, TypeScript, CSS/requestAnimationFrame, Vitest, CUA Driver.

**Spec:** docs/superpowers/specs/2026-08-24-v1-1-identity-pricing-surfaces-design.md

## Global Constraints

- Follow docs/superpowers/plans/2026-08-24-v1-1-rollout-index.md and finish the identity/surface settings plan first.
- Do not write a native fix before the trace reproduces the disappearing behavior and identifies the component boundary that changes state.
- Trace events contain only window label, desired visibility, actual visibility/minimized state, bounds, topmost result, foreground classification, and suspension reason.
- Trace events never contain window title, browser URL, page content, account identity, email, token, Cookie, or API key.
- Shell transient interactions do not set the full-screen suspension flag and do not erase float-ball geometry.
- The user may place the floating ball on the taskbar. Never move it away unless the user drags it.
- Full-screen hiding remains user-controlled and applies only to verified real full-screen.
- Motion remains clockwise: Idle 1x, Thinking 2x, Fast 3x; Fast wins when both observations are true.
- No PowerShell process or visible console is spawned for process/config observation.

---

### Task 1: Add a sanitized lifecycle trace and foreground classifier

**Files:**
- Create: apps/desktop-tauri/src-tauri/src/shell/surface_lifecycle_trace.rs
- Create: apps/desktop-tauri/src-tauri/src/shell/foreground_events.rs
- Modify: apps/desktop-tauri/src-tauri/src/shell/mod.rs
- Modify: apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/settings.rs
- Test: apps/desktop-tauri/src-tauri/src/shell/surface_lifecycle_trace.rs
- Test: apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs
- Test: apps/desktop-tauri/src-tauri/src/status_surfaces.rs

**Interfaces:**
- Produce ForegroundClass::{Normal, ShellTransient, RealFullscreen}.
- Produce SurfaceLifecycleSnapshot { surface, desired_visible, actual_visible, minimized, bounds, topmost_result, foreground_class, suspension_reason, observed_at }.
- Produce SurfaceLifecycleTrace::record(snapshot) and ::recent(limit).
- Produce get_status_surface_diagnostics() -> Vec<SurfaceLifecycleSnapshot> for proof/diagnostics only.
- Produce start_foreground_event_monitor(app: AppHandle) using raw user32 FFI plus the existing bounded polling fallback.

- [ ] **Step 1: Write failing pure-classifier and ring-buffer tests.**

~~~rust
#[test]
fn start_and_search_are_shell_transients_not_fullscreen() {
    assert_eq!(classify_foreground(window("Windows.UI.Core.CoreWindow", "Start")), ForegroundClass::ShellTransient);
    assert_eq!(classify_foreground(window("Windows.UI.Core.CoreWindow", "Search")), ForegroundClass::ShellTransient);
}

#[test]
fn trace_discards_oldest_events_at_capacity() {
    let mut trace = SurfaceLifecycleTrace::with_capacity(2);
    trace.record(event(1));
    trace.record(event(2));
    trace.record(event(3));
    assert_eq!(trace.recent(10), vec![event(2), event(3)]);
}
~~~

- [ ] **Step 2: Run trace/classifier tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml shell::surface_lifecycle_trace -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml shell::fullscreen_guard -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::tests -- --nocapture
~~~

Expected: ForegroundClass, trace storage, and proof command are absent.

- [ ] **Step 3: Implement the privacy-bounded trace.**

~~~rust
pub enum ForegroundClass { Normal, ShellTransient, RealFullscreen }

pub struct SurfaceLifecycleTrace {
    events: VecDeque<SurfaceLifecycleSnapshot>,
    capacity: usize,
}

impl SurfaceLifecycleTrace {
    pub fn record(&mut self, snapshot: SurfaceLifecycleSnapshot) {
        if self.events.len() == self.capacity { self.events.pop_front(); }
        self.events.push_back(snapshot);
    }
}
~~~

Classify foreground by safe native class/process state only. Preserve existing
real video/full-screen scan; do not use a window title except the fixed Start
and Search labels already needed to classify Windows.Core shell surfaces.

- [ ] **Step 4: Add raw foreground event observation without a new dependency.**

Use the same direct user32 FFI pattern as fullscreen_guard to register
foreground/show/hide event callbacks. Call a short, non-blocking reconciliation
scheduler; retain the 250ms monitor as a recovery path. The callback must not
take the status-surface mutex or execute WebView calls directly.

- [ ] **Step 5: Run focused tests and commit diagnostics.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml shell:: status_surfaces:: -- --nocapture
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
~~~

Commit:

~~~powershell
git add apps/desktop-tauri/src-tauri/src/shell apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/commands
git commit -m "Trace status surface lifecycle"
~~~

### Task 2: Reproduce the Windows Shell failure and establish the root cause

**Files:**
- Local verification evidence only unless Task 1 instrumentation exposes a narrowly owned defect.

**Interfaces:**
- Consume get_status_surface_diagnostics and the fresh debug build.
- Produce a sanitized sequence of lifecycle snapshots with exact transition
  ordering for each manual scenario.

- [ ] **Step 1: Build a fresh debug binary and close the exact old process.**

~~~powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
~~~

Launch the fresh binary with taskbar status and floating ball enabled. Run one
case with the ball on the taskbar and one with it away from the taskbar.

- [ ] **Step 2: Use CUA to run the reproducibility matrix.**

Execute, in separate runs: open/close Start; click blank taskbar; open Explorer;
minimize Edge; press Win+D; exit the desktop; enter/exit browser full-screen;
enter/exit video full-screen. After each action capture the window list and
call the diagnostics command.

- [ ] **Step 3: Identify the first divergent event and record a single hypothesis.**

The evidence must state one of these exact outcomes:

~~~text
A. desired_visible changed unexpectedly
B. actual window visibility/minimized state changed while desired_visible stayed true
C. bounds changed from the saved float-ball position
D. SetWindowPos/DWM topmost call failed or was superseded
E. a Shell transient was classified as RealFullscreen
~~~

If no first divergence is visible, retain the trace and add the missing
observation in Task 1 before attempting the controller change.

- [ ] **Step 4: Preserve sanitized evidence locally.**

Store only timestamped event codes and outcome A–E in a local proof artifact.
Do not commit personal coordinates, window titles, browser URLs, screenshots of
unrelated windows, or raw trace dumps.

### Task 3: Implement the trace-proven surface state machine

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces.rs
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/window.rs
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs
- Test: apps/desktop-tauri/src-tauri/src/status_surfaces.rs
- Test: apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs
- Test: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs
- Test: apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs

**Interfaces:**
- Produce SurfaceSuspensionReason::{None, Fullscreen}.
- Produce reconcile_surfaces(app, foreground: ForegroundClass, cause: ReconcileCause) -> Result<(), String>.
- Produce restore_enabled_surfaces(app, state) that restores visibility, z-order, and preserved geometry without altering stored enabled settings.
- Produce ReconcileCause::{ForegroundChanged, ShellChanged, PeriodicFallback, FullscreenTransition}.

- [ ] **Step 1: Write failing state-machine tests for all three phases.**

~~~rust
#[test]
fn shell_transient_preserves_enabled_intent_and_geometry() {
    let state = enabled_state_with_float_at(physical_point(1500, 1040));
    let next = reduce_surface_phase(state, ForegroundClass::ShellTransient);
    assert!(next.float_ball.desired_visible);
    assert_eq!(next.float_ball.saved_position, physical_point(1500, 1040));
    assert_eq!(next.suspension_reason, SurfaceSuspensionReason::None);
}

#[test]
fn normal_after_shell_transient_requests_restore_without_click() {
    let state = shell_transient_state();
    assert_eq!(reduce_surface_phase(state, ForegroundClass::Normal).action, ReconcileAction::Restore);
}

#[test]
fn real_fullscreen_hides_only_when_preference_is_enabled() {
    assert_eq!(reconcile_action(true, ForegroundClass::RealFullscreen), ReconcileAction::Suspend);
    assert_eq!(reconcile_action(false, ForegroundClass::RealFullscreen), ReconcileAction::KeepVisible);
}
~~~

- [ ] **Step 2: Run focused tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball::tests -- --nocapture
~~~

Expected: current boolean fullscreen_suspended state cannot represent ShellTransient and restore action.

- [ ] **Step 3: Implement intent/actual separation.**

Replace the boolean fullscreen_suspended with explicit desired visibility and
SurfaceSuspensionReason. ShellTransient never calls hide_for_fullscreen, never
cleans up the window, and never calls a geometry clamp. On Normal transition,
restore_enabled_surfaces calls the existing no-activate, show, reassert-topmost,
and position paths once in a deterministic order.

- [ ] **Step 4: Apply the hypothesis-specific repair from Task 2.**

- Outcome A: remove the mutation that clears enabled intent and add an assertion
  that setting persistence is never invoked by ShellTransient.
- Outcome B: call the existing visible-window creation/show path from Restore
  and keep a retryable actual-visibility marker.
- Outcome C: change the geometry path so a saved point inside the monitor,
  including its taskbar region, remains valid.
- Outcome D: record the failed native call, retry through PeriodicFallback, and
  keep state enabled.
- Outcome E: tighten the classifier so the observed Shell class can never
  enter RealFullscreen.

Implement only the outcome observed in Task 2; add its regression test before
the production change.

- [ ] **Step 5: Run Rust checks and commit the state repair.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml shell:: -- --nocapture
cargo fmt --all -- --check
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
~~~

Commit:

~~~powershell
git add apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/status_surfaces apps/desktop-tauri/src-tauri/src/taskbar_overlay apps/desktop-tauri/src-tauri/src/float_ball apps/desktop-tauri/src-tauri/src/shell
git commit -m "Restore surfaces after shell transitions"
~~~

### Task 4: Replace polling-only motion with native events and phase-safe rotation

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/float_ball_motion.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src/lib/tauri.ts
- Create: apps/desktop-tauri/src/hooks/useFloatBallMotion.ts
- Create: apps/desktop-tauri/src/hooks/useFloatBallMotion.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.tsx
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.css
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.test.tsx
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Test: apps/desktop-tauri/src-tauri/src/float_ball_motion.rs

**Interfaces:**
- Produce FloatBallMotion { state: MotionState, observed_at: DateTime<Utc> }.
- Produce MotionState::{Idle, Thinking, Fast}.
- Produce start_float_ball_motion_monitor(app: AppHandle).
- Emit codexbar:float-ball-motion-changed with FloatBallMotionDto.
- Produce useFloatBallMotion() returning MotionState and a two-second recovery query.

- [ ] **Step 1: Write failing monitor and renderer tests.**

~~~rust
#[test]
fn explicit_fast_tier_wins_over_thinking() {
    assert_eq!(derive_motion(true, true), MotionState::Fast);
}

#[test]
fn unchanged_config_metadata_does_not_reparse() {
    let mut monitor = test_monitor("service_tier = \"fast\"");
    monitor.tick(metadata(1)).unwrap();
    monitor.tick(metadata(1)).unwrap();
    assert_eq!(monitor.parse_count(), 1);
}
~~~

~~~tsx
it("applies a fast event without recreating the rotation phase", async () => {
  render(<FloatBall />);
  emit("codexbar:float-ball-motion-changed", { state: "fast" });
  await waitFor(() => expect(screen.getByTestId("float-ball-shell")).toHaveAttribute("data-motion", "fast"));
  expect(screen.getByTestId("float-ball-shell")).toHaveStyle({ "--float-speed": "3" });
});
~~~

- [ ] **Step 2: Run focused tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball_motion -- --nocapture
pnpm --dir apps/desktop-tauri test -- useFloatBallMotion FloatBall
~~~

Expected: four-second frontend polling has no event channel or monotonic phase.

- [ ] **Step 3: Implement a bounded native monitor.**

Use a 250ms Tokio cadence. Compare config-file metadata before reading content;
parse only an explicit Fast service tier/model condition. Obtain Thinking from
best-effort approved local task activity evidence. Emit only a state transition;
do not emit every tick and do not spawn a child process.

- [ ] **Step 4: Implement phase-safe browser rotation.**

~~~ts
const advance = (now: number) => {
  const elapsed = now - lastFrame.current;
  phase.current = (phase.current + elapsed * speedRef.current * DEGREES_PER_MS) % 360;
  node.style.setProperty("--float-rotation-deg", String(phase.current));
  lastFrame.current = now;
  frame.current = requestAnimationFrame(advance);
};
~~~

Update speedRef on the native event without resetting phase. Preserve clockwise
direction and the existing no-breathing/no-hover-expansion design. Disable the
animation under prefers-reduced-motion.

- [ ] **Step 5: Run focused/full checks and commit.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball_motion -- --nocapture
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
~~~

Commit:

~~~powershell
git add apps/desktop-tauri/src-tauri/src/float_ball_motion.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src/hooks/useFloatBallMotion.ts apps/desktop-tauri/src/surfaces/FloatBall.tsx apps/desktop-tauri/src/surfaces/FloatBall.css apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/types/bridge.ts
git commit -m "React to float ball motion changes"
~~~

### Task 5: Prove native recovery and motion on real Windows

**Files:**
- Verification evidence only unless this task exposes a defect owned by Tasks 1–4.

**Interfaces:**
- Consume lifecycle trace, state machine, motion events, taskbar overlay, and
  float-ball geometry.
- Produce CUA screenshots/window snapshots plus event-timing evidence.

- [ ] **Step 1: Launch a fresh debug binary with proof mode and trace enabled.**

~~~powershell
$env:CODEXBAR_PROOF_MODE = 'settings:menu'
pnpm --dir apps/desktop-tauri run tauri:build:debug
~~~

Close only the exact older codex-barbar process before opening the binary.

- [ ] **Step 2: Execute the Shell matrix with CUA.**

For both float placements, execute Start, blank taskbar, Explorer, Edge
minimize, Win+D, desktop restore, browser full-screen, and video full-screen.
After every Shell transient closes, assert both enabled surfaces restore without
a click in Codex or Edge.

- [ ] **Step 3: Measure Fast response.**

Change an approved local Fast source once, observe the emitted motion event, and
capture the floating-ball state within 500ms. Confirm the rotation angle
continues through the speed change and that Idle/Thinking/Fast are 1x/2x/3x.

- [ ] **Step 4: Restore original positions/settings and record evidence.**

Restore the float position and full-screen preference exactly. Keep the
sanitized trace and screenshot paths in the implementation ledger; do not
publish, tag, install, or alter unrelated desktop settings.
