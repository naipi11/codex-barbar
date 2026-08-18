# Taskbar Independent Measurement Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace visible-WebView width feedback with a hidden independent 318x40 measurement WebView, prove the complete weekly capsule on Windows, then finish the blocked release and current-user installation.

**Architecture:** The user-facing `taskbar-status` WebView renders only visible content. A second hidden `taskbar-status-measure` WebView renders the same shared geometry inside an independent 318x40 viewport and is the sole caller of the existing width command. Rust owns both windows, treats measurement creation as recoverable, authorizes width calls by caller label and enabled state, and preserves the 318-pixel fallback.

**Tech Stack:** Rust 2024, Tauri 2.10, React 18, TypeScript 5.6, CSS intrinsic sizing, ResizeObserver, Vitest 3, Windows WebView2, Win32/CUA proof, NSIS.

## Global Constraints

- Windows x64 only; taskbar, WebView2, DPI, no-activate behavior, and installer evidence must come from a fresh Windows-native build.
- Do not add dependencies, raw frontend resize permissions, new settings, or schema migrations.
- Visible label is `taskbar-status`; measurement label is `taskbar-status-measure`.
- Measurement viewport and safe fallback are 318x40 logical pixels; supported visible width remains inclusive 104 through 318 and height remains 40.
- The visible WebView is never a width source after production rollout.
- The measurement helper exists only while taskbar status is enabled, is destroyed first, and remains hidden, non-focusable, skip-taskbar, and `Theme::Dark`.
- Missing/invalid measurement or helper/bridge failure preserves 318 before first success and the last confirmed native width afterward.
- `set_taskbar_status_width` remains the bridge name; only `taskbar-status-measure` may call it while taskbar status is enabled.
- Frontend emits no console diagnostics. Rust diagnostics use stable codes through `tracing` and contain no tokens, cookies, identities, raw payloads, or private paths.
- Real quotas only: no fabricated 5H metric. Preserve six-code-point compact identity, band colors, 166-pixel quota-track cap, reset and close columns, background-only opacity, and taskbar-safe positioning.
- A close failure uses a fixed-size red retry button, tooltip, `aria-live`, and reduced-motion-safe animation; it must not add inline geometry text.
- Preserve the five pre-existing untracked historical screenshots. Do not stage them unless a verification task explicitly classifies one.
- Production release and installation remain blocked until fresh proof shows `ProofU | 周 98% | 8/20 | ×`, no `5H`, width below 318, and height 40.

---

## File Structure

- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`: visible and measurement constants/builders.
- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`: two-window ownership, recovery, cleanup ordering, and width state.
- `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`: command boundary, measurement-destroyed routing, monitor reconciliation.
- `apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs`: inject invoking WebView label into the width command.
- `apps/desktop-tauri/src-tauri/src/main.rs`: measurement-window event routing only; no user disable intent for the helper.
- `apps/desktop-tauri/src-tauri/src/proof_harness.rs`: proof-only first-gate detection.
- `apps/desktop-tauri/src-tauri/capabilities/default.json`: allow the new app window label without adding permissions.
- `apps/desktop-tauri/src/App.tsx`: route the measurement label.
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`: visible controller only after rollout.
- `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.tsx`: hidden measurement controller only.
- `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx`: shared field geometry and mode semantics.
- `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts`: pure shared presentation derivation.
- `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts`: serialized measurement submission, used only by the helper route.
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`: visible, measurement, and compact close-error states.
- `docs/WINDOWS_PROOF.md`: final independent-window proof contract.
- `docs/verification/windows/2026-08-14/independent-measurement-observations.md`: new proof timeline; prior STOP record remains unchanged.

---

### Task 1: Build a Proof-Only Hidden WebView2 Vertical Probe

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/capabilities/default.json`
- Modify: `apps/desktop-tauri/src/App.tsx`
- Modify: `apps/desktop-tauri/src/App.test.tsx`
- Create: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts`
- Create: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts`
- Create: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.tsx`
- Create: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Create: `.superpowers/sdd/2026-08-14-taskbar-independent-measurement/task-1-report.md`

**Interfaces:**
- Consumes: existing `TaskbarStatusContents`, `useStatusSurface`, `useTaskbarStatusWidth`, `set_taskbar_status_width`, proof scenario `taskbar-status:weekly`.
- Produces: `TASKBAR_MEASUREMENT_WINDOW_LABEL`, `TASKBAR_MEASUREMENT_FRONTEND_ROUTE`, `get_or_create_measurement`, `is_taskbar_status_proof`, `buildTaskbarStatusPresentation`, measurement-only React route, and a Windows go/no-go result.
- Scope: the independent helper is active only in taskbar proof mode. Normal user operation retains the existing in-page replica until Task 2.

- [ ] **Step 1: Write failing proof-mode and builder contract tests**

Add exact Rust tests:

```rust
#[test]
fn measurement_window_contract_is_fixed_and_hidden() {
    assert_eq!(TASKBAR_MEASUREMENT_WINDOW_LABEL, "taskbar-status-measure");
    assert_eq!(
        TASKBAR_MEASUREMENT_FRONTEND_ROUTE,
        "index.html?window=taskbar-status-measure"
    );
    assert_eq!(TASKBAR_MEASUREMENT_LOGICAL_WIDTH, 318);
    assert_eq!(TASKBAR_LOGICAL_HEIGHT, 40);
}

#[test]
fn only_taskbar_proof_scenarios_enable_the_probe() {
    assert!(is_taskbar_status_scenario(
        ProofScenario::TaskbarStatus(StatusProofState::Weekly)
    ));
    assert!(!is_taskbar_status_scenario(
        ProofScenario::FloatBall(StatusProofState::Weekly)
    ));
}
```

Add a pure route switch in `TaskbarStatus.tsx` and test it:

```ts
export function usesExternalTaskbarMeasurement(search: string): boolean {
  return new URLSearchParams(search).get("measurement") === "external";
}

expect(usesExternalTaskbarMeasurement("?measurement=external")).toBe(true);
expect(usesExternalTaskbarMeasurement("")).toBe(false);
```

Add a failing shared-model test requiring both routes to consume:

```ts
export function buildTaskbarStatusPresentation(
  surface: UseStatusSurfaceResult,
): TaskbarStatusPresentation;
```

The weekly fixture must produce `ProofU`, `周 98%`, nearest reset `8/20`, and identical visible/measurement geometric fields.

Expected failure: measurement constants/builder, proof predicate, measurement route, and search helper do not exist.

- [ ] **Step 2: Run the RED slices**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri exec vitest run src/App.test.tsx src/surfaces/TaskbarStatus.test.tsx src/surfaces/TaskbarStatusMeasure.test.tsx --reporter=verbose
```

Expected: fail for the missing measurement label, route, component, and proof-only external-measurement path.

- [ ] **Step 3: Add the proof-only measurement builder**

In `taskbar_overlay/window.rs`, add:

```rust
pub const TASKBAR_MEASUREMENT_WINDOW_LABEL: &str = "taskbar-status-measure";
pub const TASKBAR_MEASUREMENT_FRONTEND_ROUTE: &str =
    "index.html?window=taskbar-status-measure";
pub const TASKBAR_PROBE_FRONTEND_ROUTE: &str =
    "index.html?window=taskbar-status&measurement=external";
pub const TASKBAR_MEASUREMENT_LOGICAL_WIDTH: u32 = 318;

pub fn get_or_create_measurement(
    app: &tauri::AppHandle,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(TASKBAR_MEASUREMENT_WINDOW_LABEL) {
        return Ok(window);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        TASKBAR_MEASUREMENT_WINDOW_LABEL,
        WebviewUrl::App(TASKBAR_MEASUREMENT_FRONTEND_ROUTE.into()),
    )
    .title("codex-barbar taskbar measurement")
    .inner_size(318.0, 40.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .skip_taskbar(true)
    .focusable(false)
    .focused(false)
    .theme(Some(tauri::Theme::Dark))
    .visible(false)
    .build()
    .map_err(|_| "TASKBAR_MEASUREMENT_WINDOW_CREATE_FAILED".to_string())
}
```

Add the proof predicates to `proof_harness.rs`:

```rust
pub fn is_taskbar_status_scenario(scenario: ProofScenario) -> bool {
    matches!(scenario, ProofScenario::TaskbarStatus(_))
}

pub fn is_taskbar_status_proof(app: &AppHandle) -> bool {
    let state = app.state::<Mutex<AppState>>();
    state
        .lock()
        .ok()
        .and_then(|state| state.proof_config.as_ref().map(|cfg| cfg.scenario))
        .is_some_and(is_taskbar_status_scenario)
}
```

The visible builder uses `TASKBAR_PROBE_FRONTEND_ROUTE` only while this predicate is true.

- [ ] **Step 4: Add the proof-only frontend measurement route**

Create `TaskbarStatusMeasure.tsx`:

```tsx
export default function TaskbarStatusMeasure() {
  const surface = useStatusSurface();
  const presentation = buildTaskbarStatusPresentation(surface);
  const measurementRef = useRef<HTMLDivElement>(null);
  useTaskbarStatusWidth(measurementRef);

  return (
    <TaskbarStatusContents
      mode="measurement"
      displayName={presentation.displayName}
      compactIdentity={presentation.compactIdentity}
      metrics={presentation.metrics}
      reset={presentation.reset}
      trustState={presentation.trustState}
      closeError={null}
      ariaLabel={presentation.ariaLabel}
      measurementRef={measurementRef}
    />
  );
}
```

Route label `taskbar-status-measure` in `App.tsx`:

```tsx
if (label === "taskbar-status-measure") {
  return <TaskbarStatusMeasure />;
}
```

In proof mode, `TaskbarStatus` renders visible mode only; outside proof mode it retains the current in-page measurement replica. Add `taskbar-status-measure` to `capabilities/default.json` without adding a permission.

Create `taskbarStatusPresentation.ts` in Task 1 and move `nearestReset`, metric accessibility text, trust text, and alpha conversion into it. Both `TaskbarStatus` and `TaskbarStatusMeasure` must use this one builder from the first probe onward; no provisional duplicate reset logic is allowed.

- [ ] **Step 5: Create and destroy the helper only in proof mode**

Add `measurement_window: Option<tauri::WebviewWindow>` to `TaskbarOverlay`. In `apply_enabled(true)`, after the visible window exists, create the helper only when `is_taskbar_status_proof(app)` is true. On disable, close the proof helper before the visible window with `close_cached_or_labeled`. Keep normal non-proof behavior unchanged.

The proof-only enable branch is explicit:

```rust
self.ensure_window(app)?;
if crate::proof_harness::is_taskbar_status_proof(app) {
    let measurement = window::get_or_create_measurement(app)?;
    self.measurement_window = Some(measurement);
}
self.reposition(app)
```

If proof helper creation fails, leave the visible proof surface at 318 and return the stable measurement-window error so the probe cannot be mistaken for a pass.

- [ ] **Step 6: Run GREEN source checks**

Run the three RED commands again, then:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run build
cargo fmt --all -- --check
git diff --check
```

Expected: focused tests and build pass; normal `TaskbarStatus` test still proves the pre-rollout in-page behavior, while proof-route tests prove the visible root has no local measurement root.

- [ ] **Step 7: Run the decisive hidden-WebView probe**

Stop only `codex-barbar.exe`, build fresh debug, and launch:

```powershell
Get-Process -Name codex-barbar -ErrorAction SilentlyContinue | Stop-Process -Force
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run tauri:build:debug
$env:CODEXBAR_PROOF_MODE='taskbar-status:weekly'
& '.\target\debug\codex-barbar.exe'
```

Prefer CUA. If the exact driver is absent, use Win32 enumeration, `IsWindowVisible`, DPI-aware `GetWindowRect`, and `PrintWindow`.

Required PASS:

```text
taskbar-status-measure exists; IsWindowVisible=false
taskbar-status visible: ProofU | 周 98% | 8/20 | ×
5H absent; logical height=40; logical width 104..317
```

If any line fails, stop the whole plan. Do not proceed to Task 2 or introduce an off-screen visible helper.

- [ ] **Step 8: Commit Task 1**

Stage only the listed source/test/capability files. Do not stage screenshots or the SDD report.

```powershell
git commit -m "Probe hidden taskbar measurement"
```

---

### Task 2: Productionize Dual-Window Lifecycle and Width Authorization

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/capabilities/default.json`
- Modify: `apps/desktop-tauri/src/App.tsx`
- Modify: `apps/desktop-tauri/src/App.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx`
- Create: `.superpowers/sdd/2026-08-14-taskbar-independent-measurement/task-2-report.md`

**Interfaces:**
- Consumes: Task 1 probe-approved builder and measurement route.
- Produces: measurement helper for every enabled taskbar surface, `handle_taskbar_measurement_window_destroyed`, label-authorized `set_taskbar_status_width`, measurement-first cleanup, and nonfatal two-second helper recreation.

- [ ] **Step 1: Write failing lifecycle order and authorization tests**

Extract small deterministic helpers and test exact ordering:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeasurementAvailability { Ready, Deferred }

#[test]
fn measurement_creation_failure_keeps_visible_enabled_at_fallback() {
    let result = enable_windows_with(|| Ok(()), || Err("CREATE".into())).unwrap();
    assert_eq!(result, MeasurementAvailability::Deferred);
}

#[test]
fn disable_closes_measurement_before_visible() {
    let calls = std::cell::RefCell::new(Vec::new());
    disable_windows_with(
        || { calls.borrow_mut().push("measurement"); Ok(()) },
        || { calls.borrow_mut().push("visible"); Ok(()) },
    ).unwrap();
    assert_eq!(*calls.borrow(), ["measurement", "visible"]);
}

#[test]
fn measurement_close_failure_does_not_close_visible() {
    let visible_closed = std::cell::Cell::new(false);
    assert!(disable_windows_with(
        || Err("CLOSE".into()),
        || { visible_closed.set(true); Ok(()) },
    ).is_err());
    assert!(!visible_closed.get());
}

#[test]
fn hidden_pending_destroy_is_not_a_completed_measurement_close() {
    assert_eq!(
        require_measurement_destroyed(CloseOutcome::HiddenPendingDestroy),
        Err("TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED")
    );
}
```

Add a pure authorization test:

```rust
assert_eq!(authorize_taskbar_width("taskbar-status-measure", true), Ok(()));
assert_eq!(
    authorize_taskbar_width("taskbar-status", true),
    Err("TASKBAR_MEASUREMENT_UNAUTHORIZED")
);
assert_eq!(
    authorize_taskbar_width("taskbar-status-measure", false),
    Err("TASKBAR_STATUS_DISABLED")
);
```

Expected failure: helpers and caller-aware command do not exist.

- [ ] **Step 2: Run the RED Rust suites**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces -- --nocapture
```

- [ ] **Step 3: Generalize the Task 1 helper to all enabled sessions**

Remove the proof-only creation guard and visible-route query switch. `TaskbarStatus` now always renders visible mode only. `TaskbarStatusMeasure` remains the sole owner of `useTaskbarStatusWidth`.

Implement deterministic sequencing:

```rust
fn enable_windows_with(
    ensure_visible: impl FnOnce() -> Result<(), String>,
    ensure_measurement: impl FnOnce() -> Result<(), String>,
) -> Result<MeasurementAvailability, String> {
    ensure_visible()?;
    Ok(if ensure_measurement().is_ok() {
        MeasurementAvailability::Ready
    } else {
        MeasurementAvailability::Deferred
    })
}

fn disable_windows_with(
    close_measurement: impl FnOnce() -> Result<(), String>,
    close_visible: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    close_measurement()?;
    close_visible()
}
```

`TaskbarOverlay::apply_enabled(true)` treats `Deferred` as enabled success, keeps logical width 318 before first measurement, and logs:

```rust
tracing::debug!(
    code = "TASKBAR_MEASUREMENT_CREATE_DEFERRED",
    "taskbar measurement helper retry deferred"
);
```

The two-second monitor calls `ensure_measurement_window` before repositioning the visible window. Measurement failure is nonfatal; visible reposition failure retains its existing error semantics.

- [ ] **Step 4: Implement shutdown rollback and destroyed routing**

Measurement cleanup uses `close_cached_or_labeled` first, but only `CloseOutcome::Destroyed` authorizes visible cleanup:

```rust
fn require_measurement_destroyed(
    outcome: CloseOutcome,
) -> Result<(), &'static str> {
    match outcome {
        CloseOutcome::Destroyed => Ok(()),
        CloseOutcome::HiddenPendingDestroy => {
            Err("TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED")
        }
    }
}
```

`HiddenPendingDestroy` and hard close errors both stop the sequence before visible cleanup and restore the previous enabled bool. If measurement destruction succeeds but visible close fails, the enabled monitor recreates measurement on its next tick.

Add:

```rust
pub fn handle_measurement_window_destroyed(&mut self) {
    self.measurement_window = None;
}

pub fn is_measurement_window_label(label: &str) -> bool {
    label == TASKBAR_MEASUREMENT_WINDOW_LABEL
}
```

In `main.rs`, handle this label before `surface_for_window_label`:

```rust
if taskbar_overlay::window::is_measurement_window_label(window.label()) {
    if matches!(event, tauri::WindowEvent::Destroyed) {
        status_surfaces::handle_taskbar_measurement_window_destroyed(
            window.app_handle(),
        );
    }
    return;
}
```

Do not map the helper to `StatusSurfaceKind`; its destruction is not a user disable intent.

- [ ] **Step 5: Authorize the width command by injected caller**

Change the command signature:

```rust
#[tauri::command]
pub fn set_taskbar_status_width(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    width: f64,
) -> Result<(), String> {
    crate::status_surfaces::set_taskbar_status_width(
        &app,
        window.label(),
        width,
    )
}
```

Change the shared entry point to:

```rust
pub fn set_taskbar_status_width(
    app: &tauri::AppHandle,
    caller_label: &str,
    width: f64,
) -> Result<(), String>;
```

It checks exact label, then checks `state.taskbar.is_enabled()`, then delegates to the existing clamp/transaction. Rejected calls perform no state mutation. Do not add a command or permission.

- [ ] **Step 6: Run GREEN and full native checks**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --all -- --check
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri exec vitest run src/App.test.tsx src/surfaces/TaskbarStatus.test.tsx src/surfaces/TaskbarStatusMeasure.test.tsx --reporter=verbose
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run build
git diff --check
```

- [ ] **Step 7: Commit Task 2**

Stage only the listed source/test/capability files and commit:

```powershell
git commit -m "Own taskbar measurement lifecycle"
```

---

### Task 3: Harden Shared Presentation and Make Close Errors Width-Stable

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx`
- Modify: `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx`
- Create: `.superpowers/sdd/2026-08-14-taskbar-independent-measurement/task-3-report.md`

**Interfaces:**
- Consumes: Task 1 `buildTaskbarStatusPresentation(surface)` plus production dual-window routing from Task 2.
- Produces: hardened identical visible/measurement props, fixed-size close failure state, and measurement-only ResizeObserver ownership.

- [ ] **Step 1: Write failing pure-model and close-state tests**

Define the intended model:

```ts
export interface TaskbarStatusPresentation {
  displayName: string;
  compactIdentity: string;
  metrics: readonly StatusQuotaMetric[];
  reset: StatusQuotaMetric | null;
  trustState: TrustState;
  ariaLabel: string;
  surfaceAlpha: string;
}

export function buildTaskbarStatusPresentation(
  surface: UseStatusSurfaceResult,
): TaskbarStatusPresentation;
```

Test a weekly fixture from both routes and compare:

```ts
expect(measurementGeometry()).toEqual(visibleGeometry());
expect(visibleGeometry()).toEqual([
  "taskbar-status__avatar:P",
  "taskbar-status__identity:ProofU",
  "taskbar-status__metric:周 98%",
  "taskbar-status__reset:8/20",
  "taskbar-status__close:×",
]);
```

Replace the old inline error assertion with:

```ts
expect(close).toHaveAttribute("data-error", "true");
expect(close).toHaveAttribute("title", "关闭失败，点击重试");
expect(screen.getByRole("status")).toHaveTextContent("关闭失败，点击重试");
expect(visible).not.toHaveTextContent("关闭失败，请重试");
expect(close).toBeEnabled();
```

Add a structural CSS test requiring `@media (prefers-reduced-motion: reduce)` to disable the close-error animation.

- [ ] **Step 2: Run RED frontend tests**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri exec vitest run src/surfaces/taskbarStatusPresentation.test.ts src/surfaces/TaskbarStatus.test.tsx src/surfaces/TaskbarStatusMeasure.test.tsx src/hooks/useTaskbarStatusWidth.test.tsx --reporter=verbose
```

Expected: shared-model baseline assertions pass from Task 1, while new close-state and hardened multi-window assertions fail.

- [ ] **Step 3: Extract the shared presentation builder**

Keep `nearestReset`, metric accessibility text, trust text, and alpha conversion exclusively in `taskbarStatusPresentation.ts`. Expand its tests for multiple real windows, cached state, used/remaining modes, and opacity boundaries. The measurement route must never independently select a reset or format labels.

Use one implementation:

```ts
export function buildTaskbarStatusPresentation(
  surface: UseStatusSurfaceResult,
): TaskbarStatusPresentation {
  const reset = nearestReset(surface.metrics);
  const metricsText =
    surface.metrics.map(compactTaskbarMetric).join("，") || "无可用额度";
  const trustText =
    surface.trustState === "cached" ? "缓存数据" : surface.refreshStatus;
  return {
    displayName: surface.displayName,
    compactIdentity: surface.compactIdentity,
    metrics: surface.metrics,
    reset,
    trustState: surface.trustState,
    ariaLabel: [
      "打开完整面板",
      surface.displayName,
      metricsText,
      reset?.resetText,
      trustText,
      surface.updatedText,
    ].filter(Boolean).join("，"),
    surfaceAlpha: String(
      Math.max(0, Math.min(
        100,
        surface.bootstrap?.settings.taskbarStatusOpacity ?? 20,
      )) / 100,
    ),
  };
}
```

`TaskbarStatusContents` receives only presentation fields plus mode/handlers. Remove `closeError` from the shared geometry. Keep the close glyph in both modes so its 24-pixel column is measured.

- [ ] **Step 4: Implement fixed-size close failure feedback**

In visible mode:

```tsx
<button
  data-error={closeFailed ? "true" : undefined}
  aria-label="关闭任务栏状态"
  title={closeFailed ? "关闭失败，点击重试" : "关闭任务栏状态"}
  onClick={onClose}
>
  <span aria-hidden="true">×</span>
</button>
{visible ? (
  <span className="taskbar-status__live" role="status" aria-live="polite">
    {closeFailed ? "关闭失败，点击重试" : ""}
  </span>
) : null}
```

The live region is visually hidden and absolutely removed from geometry. Add:

```css
.taskbar-status__close[data-error="true"] {
  color: var(--quota-low);
  animation: taskbar-close-error 180ms ease-in-out 2;
}

.taskbar-status__live {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}

@keyframes taskbar-close-error {
  25% { transform: translateX(-1px); }
  75% { transform: translateX(1px); }
}

@media (prefers-reduced-motion: reduce) {
  .taskbar-status__close[data-error="true"] { animation: none; }
}
```

Click retry first clears `closeFailed`; rejection sets it true again. Success closes through the existing typed controller.

- [ ] **Step 5: Prove measurement-only observer ownership**

In `TaskbarStatus.test.tsx`, assert rendering visible status never invokes `set_taskbar_status_width`. In `TaskbarStatusMeasure.test.tsx`, stub a 247-pixel root and assert the helper invokes exactly once. Keep the hook's growth, shrink, dedupe, rejection retry, latest queue, missing observer, invalid width, and unmount tests.

- [ ] **Step 6: Run GREEN and frontend regression**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri exec vitest run src/surfaces/taskbarStatusPresentation.test.ts src/surfaces/TaskbarStatus.test.tsx src/surfaces/TaskbarStatusMeasure.test.tsx src/hooks/useTaskbarStatusWidth.test.tsx --reporter=verbose
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run build
git diff --check
```

Expected: at least the current 168 tests plus new tests pass; production build passes.

- [ ] **Step 7: Commit Task 3**

Stage only the listed source/test files and commit:

```powershell
git commit -m "Stabilize taskbar measurement content"
```

---

### Task 4: Run Full CI and Fresh Independent-Window Windows Proof

**Files:**
- Modify: `docs/WINDOWS_PROOF.md`
- Create: `docs/verification/windows/2026-08-14/independent-measurement-observations.md`
- Create: `docs/verification/windows/2026-08-14/screenshots/taskbar-weekly-independent-measurement.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-14/screenshots/taskbar-independent-opacity-0.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-14/screenshots/taskbar-independent-opacity-80.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-14/screenshots/taskbar-independent-close-retry.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-14/screenshots/float-ball-independent-weekly-20.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-14/screenshots/float-ball-independent-expanded-weekly.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-14/screenshots/settings-independent-general.png`
- Create: `.superpowers/sdd/2026-08-14-taskbar-independent-measurement/task-4-report.md`

**Interfaces:**
- Consumes: reviewed Tasks 1 through 3.
- Produces: decisive Windows PASS/STOP evidence. A weekly failure blocks Task 5.

- [ ] **Step 1: Run focused and complete source gates**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server
cargo test --manifest-path rust/Cargo.toml storage::settings_repository
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
```

Record exact counts and any documented Corepack substitution. Do not claim pass from partial output.

- [ ] **Step 2: Update the proof contract**

In `docs/WINDOWS_PROOF.md`, replace the same-WebView off-screen replica wording with:

```text
The visible taskbar WebView is never measured. While taskbar status is enabled,
a hidden independent 318x40 WebView renders the shared content geometry and is
the sole width source. Failure preserves the functional 318px fallback.
```

State that `cua-observations.md` is the prior failed architecture record and remains immutable historical evidence.

- [ ] **Step 3: Build fresh debug and capture the hard weekly gate**

Stop only the app under test, build pinned `tauri:build:debug`, record debug EXE SHA-256, launch `CODEXBAR_PROOF_MODE=taskbar-status:weekly`, then prove:

```text
taskbar-status-measure exists
IsWindowVisible(taskbar-status-measure) == false
measurement window is not foreground and has skip-taskbar/tool-window behavior
visible text: ProofU | 周 98% | 8/20 | ×
5H absent
logical visible rect width 104..317 and height 40
visible rect inside taskbar-safe region
```

Save `taskbar-weekly-independent-measurement.png`. If any condition fails, write an explicit STOP record, commit only truthful evidence, and stop. Do not change geometry or proceed to release.

- [ ] **Step 4: Complete remaining UI proof only after weekly PASS**

Capture taskbar opacity 0/80, close failure/retry, close persistence outside proof mode, measurement-helper destruction on disable, float-ball weekly collapsed/expanded, Settings General sliders/keyboard/Escape, float-ball drag/click, and dark-theme isolation. Use CUA when present; otherwise document Win32/UIA/PrintWindow limits.

- [ ] **Step 5: Commit Task 4 evidence**

Inspect every image for account data/secrets, stage only the new proof document, updated `WINDOWS_PROOF.md`, and selected fresh screenshots:

```powershell
git commit -m "Verify independent taskbar measurement"
```

Do not stage the five old untracked screenshots, installers, databases, logs, or SDD report.

---

### Task 5: Verify Live Data, Build Release, and Install Current User

**Files:**
- Modify: `docs/verification/windows/2026-08-14/independent-measurement-observations.md`
- Create: `.superpowers/sdd/2026-08-14-taskbar-independent-measurement/task-5-report.md`

**Interfaces:**
- Consumes: Task 4 weekly and UI PASS evidence.
- Produces: verified official resolver/live quota boundary, production NSIS SHA-256, current-user installation classification, post-install verification, and final review package.

- [ ] **Step 1: Verify resolver and live quota without touching the wrapper**

Record only SHA-256 and modification time of the existing OpenCodex wrapper before and after. Run `codex --version`, build/run the repository's redacted App Server smoke, trigger a normal app refresh, and assert:

```text
installation = verifiedNpmLayout or nativeExe
installed Codex version detected
signed-in state available
rate limits available
new successful snapshot persisted
weekly period, remaining percentage, and reset date agree at one observation time
```

If smoke succeeds but persistence fails, use systematic debugging. Never record raw protocol, cookies, tokens, full account identity, or full private paths.

- [ ] **Step 2: Build and verify production NSIS**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run tauri:build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-artifacts.ps1
```

Expected artifact:

```text
target\release\bundle\nsis\codex-barbar_1.0.0_x64-setup.exe
```

Record SHA-256; do not commit the installer.

- [ ] **Step 3: Install without elevation or data deletion**

Use the repository current-user smoke path or NSIS current-user silent mode. Never select a data-delete option. Confirm:

- install root `%LOCALAPPDATA%\Programs\codex-barbar`;
- existing taskbar/float enable flags, account cache, and both opacity settings survive;
- uninstall registration remains present;
- installed executable hash/timestamp corresponds to the release build; and
- launched process path is the installed release, not the debug worktree.

- [ ] **Step 4: Run post-install verification**

Stop any debug process. Repeat focused taskbar/status/frontend tests and inspect the installed process path/version/hash. Confirm normal taskbar enable creates both windows, helper remains hidden, close disables and destroys both, and current quota refresh succeeds.

- [ ] **Step 5: Update evidence and commit**

Append exact commands, counts, resolver classification, live quota method/result, installer SHA-256, installed path classification, settings survival, and limitations to `independent-measurement-observations.md`.

```powershell
git add docs/verification/windows/2026-08-14/independent-measurement-observations.md
git diff --cached --check
git commit -m "Verify independent measurement release"
```

- [ ] **Step 6: Run final independent review**

Generate a review package from commit `17a33a5f` through HEAD. The reviewer must check every acceptance criterion in the independent-window spec, resolver signature security, settings atomicity, provider isolation, taskbar/float lifecycle, proof authenticity, release hash, installed process path, and unrelated-file scope.

Completion requires no Critical or Important findings, full local CI pass, fresh Windows proof, a verified production NSIS, and confirmed current-user installation. If any required evidence is absent, report the remaining blocker rather than declaring completion.
