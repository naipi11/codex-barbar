# Taskbar Unconstrained Measurement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the taskbar's visible-viewport width feedback loop with an inert, unconstrained measurement replica, prove the complete weekly capsule on Windows, then finish the blocked release and current-user installation.

**Architecture:** A shared `TaskbarStatusContents` component renders identical visible and measurement geometry. Only the off-screen, inert replica is observed; Rust starts from a functional 318-logical-pixel fallback and remains responsible for clamp, native resize, positioning, and rollback. Release and installation stay blocked until a fresh Windows screenshot shows the complete weekly identity, quota, reset, and close sequence.

**Tech Stack:** React 18, TypeScript 5.6, CSS intrinsic sizing, ResizeObserver, Tauri 2, Rust 2024, Vitest 3, Windows Win32/UIA or CUA proof, NSIS.

## Global Constraints

- Windows x64 only; validate taskbar, WebView2, tray, DPI, and installer behavior on a Windows-native host.
- Do not add dependencies or raw frontend window-resize permissions.
- Taskbar logical height remains 40; supported width remains inclusive 104 through 318.
- The failure fallback is 318 logical pixels; complete controls take priority over compactness during failure.
- The visible taskbar surface is never a frontend width source.
- Measurement and visible geometry must share one presentational component.
- The measurement replica is invisible, inert, outside the accessibility tree, outside tab order, and free of duplicate test IDs.
- Preserve real-quota-only rendering, six-code-point compact identity, 166-pixel quota-track cap, background-only opacity, band colors, close retry, native no-activate behavior, and taskbar-safe positioning.
- Preserve the serialized latest-width queue, bridge command name `set_taskbar_status_width`, Rust transaction rollback, stable diagnostics, and settings/provider isolation.
- Do not modify or execute the user's OpenCodex wrapper; snapshot its hash and timestamp before and after resolver proof.
- Do not record tokens, cookies, raw protocol payloads, full email addresses, or private account paths in committed evidence.
- Do not build or install release artifacts until the post-redesign taskbar proof passes.
- Existing untracked failed proof screenshots are generated task evidence, not source; do not stage them until Task 3 classifies and names them.

---

## File Structure

- `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx`: pure shared geometry for visible and measurement modes.
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`: surface controller, derived labels, interactions, visible/replica composition, and measurement ref.
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`: visible viewport layout and isolated off-screen intrinsic replica rules.
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`: component, accessibility, geometry-sharing, cap, and interaction contracts.
- `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts`: replica-only width extraction and serialized bridge queue; no recursive traversal.
- `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx`: initial observation, growth, shrink, dedupe, rejection, unmount, and missing-observer contracts.
- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`: width constants, including explicit 318-pixel safe fallback.
- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`: manager initialization and existing transactional resize tests.
- `docs/WINDOWS_PROOF.md`: fallback and measurement-replica proof contract.
- `docs/verification/windows/2026-08-14/`: historical failure and fresh passing evidence, followed by release/install observations.

---

### Task 1: Establish the Native 318-Pixel Safe Fallback

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/positioning.rs` only if a constant-name assertion requires it
- Test: existing test modules in the same Rust files

**Interfaces:**
- Produces: `TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH: u32 = 318`.
- Preserves: `TASKBAR_MIN_LOGICAL_WIDTH = 104`, `TASKBAR_MAX_LOGICAL_WIDTH = 318`, `TASKBAR_LOGICAL_HEIGHT = 40`, `clamp_logical_width`, and `TaskbarOverlay::set_content_width` transaction semantics.
- Consumed by Task 2: the frontend may fail without narrowing the native window below a fully functional width.

- [ ] **Step 1: Write failing fallback initialization tests**

Add in `taskbar_overlay/window.rs`:

```rust
#[test]
fn taskbar_dimensions_include_a_functional_safe_fallback() {
    assert_eq!(TASKBAR_MIN_LOGICAL_WIDTH, 104);
    assert_eq!(TASKBAR_MAX_LOGICAL_WIDTH, 318);
    assert_eq!(TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH, 318);
    assert_eq!(TASKBAR_LOGICAL_HEIGHT, 40);
}
```

Add in `taskbar_overlay/mod.rs`:

```rust
#[test]
fn overlay_starts_at_the_safe_fallback_width() {
    let overlay = TaskbarOverlay::default();
    assert_eq!(overlay.logical_width, window::TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH);
    assert_eq!(overlay.last_slot, None);
}
```

Keep the existing clamp table and resize/reposition compensation tests unchanged.

- [ ] **Step 2: Run the RED tests**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_dimensions_include_a_functional_safe_fallback -- --exact
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml overlay_starts_at_the_safe_fallback_width -- --exact
```

Expected: compile failures because `TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH` does not exist and `TaskbarOverlay::default()` still uses 168.

- [ ] **Step 3: Implement the explicit fallback constant**

In `window.rs`, replace the obsolete fixed default contract:

```rust
pub const TASKBAR_MIN_LOGICAL_WIDTH: u32 = 104;
pub const TASKBAR_MAX_LOGICAL_WIDTH: u32 = 318;
pub const TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH: u32 = 318;
pub const TASKBAR_LOGICAL_HEIGHT: u32 = 40;
```

In `TaskbarOverlay::default()` use:

```rust
logical_width: window::TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH,
```

Remove `TASKBAR_DEFAULT_LOGICAL_WIDTH`; do not retain an unused 168-pixel constant.

- [ ] **Step 4: Run GREEN and regression checks**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay -- --nocapture
cargo fmt --all -- --check
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: all taskbar overlay tests pass; transaction failure codes remain `TASKBAR_STATUS_RESIZE_FAILED`.

- [ ] **Step 5: Commit Task 1**

Stage only the native fallback files and inspect the staged list:

```powershell
git add apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs apps/desktop-tauri/src-tauri/src/taskbar_overlay/positioning.rs
git diff --cached --name-only
git commit -m "Use a safe taskbar fallback width"
```

---

### Task 2: Replace Recursive Geometry with the Hidden Measurement Replica

**Files:**
- Create: `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts`
- Modify: `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx`

**Interfaces:**
- Produces:

```ts
export type TaskbarStatusContentsMode = "visible" | "measurement";

export interface TaskbarStatusContentsProps {
  mode: TaskbarStatusContentsMode;
  displayName: string;
  compactIdentity: string;
  metrics: readonly StatusQuotaMetric[];
  reset: StatusQuotaMetric | null;
  trustState: TrustState;
  closeError: string | null;
  ariaLabel: string;
  onOpen?(): void;
  onClose?(event: React.MouseEvent<HTMLButtonElement>): void;
  measurementRef?: React.Ref<HTMLDivElement>;
}

export function TaskbarStatusContents(props: TaskbarStatusContentsProps): JSX.Element;
```

- `useTaskbarStatusWidth(ref: RefObject<HTMLElement>)` consumes only the measurement-replica ref.
- Preserves the typed bridge `setTaskbarStatusWidth(width: number): Promise<void>` and serialized one-request-in-flight behavior.

- [ ] **Step 1: Write failing shared-geometry and accessibility tests**

In `TaskbarStatus.test.tsx`, render the weekly proof fixture and require two geometry roots:

```tsx
const visible = await screen.findByTestId("taskbar-status-visible");
const replica = screen.getByTestId("taskbar-status-measurement");

expect(within(visible).getByText("ProofU")).toBeVisible();
expect(within(visible).getByText("周 98%")).toBeVisible();
expect(within(visible).getByText("8/20")).toBeVisible();
expect(within(visible).getByRole("button", { name: "关闭任务栏状态" })).toBeVisible();

expect(replica).toHaveAttribute("aria-hidden", "true");
expect(replica).toHaveAttribute("inert");
expect(within(replica).queryByTestId("taskbar-status-metric")).toBeNull();
expect(screen.getAllByRole("button")).toHaveLength(2);
```

Assert the visible and measurement roots contain identical classed geometric fields in the same order by comparing:

```ts
const geometry = (root: HTMLElement) =>
  Array.from(root.querySelectorAll(
    ".taskbar-status__avatar,.taskbar-status__identity,.taskbar-status__metric,.taskbar-status__reset,.taskbar-status__close",
  )).map((element) => `${element.className}:${element.textContent}`);

expect(geometry(replica)).toEqual(geometry(visible));
```

For many real metrics, assert all metrics exist in both instances, the measurement root has the 318 cap class, and reset/close remain outside the quota track.

- [ ] **Step 2: Write failing replica-only hook tests**

Replace recursive-geometry fixtures in `useTaskbarStatusWidth.test.tsx` with an unconstrained replica subject:

```tsx
function ReplicaSubject() {
  const replicaRef = useRef<HTMLDivElement>(null);
  useTaskbarStatusWidth(replicaRef);
  return <div ref={replicaRef} data-testid="replica" />;
}
```

Stub the replica at 247.4 pixels while a separate visible element is 168 pixels; assert the bridge receives only 247:

```ts
expect(invokeMock).toHaveBeenLastCalledWith(
  "set_taskbar_status_width",
  { width: 247 },
);
```

Add exact tests for:

- initial replica measurement before the first observer callback;
- observer growth 247 to 281;
- genuine shrink 281 to 226;
- repeated 226 dedupe;
- rejected 226 retry only after a future observation;
- queued latest width serialization;
- unmount while a request is pending;
- missing `ResizeObserver`, zero, NaN, or infinite measurement: no bridge call; the native 318-pixel fallback remains in place and the frontend emits no console log.

Delete every test that sums nested descendants, gaps, frames, or recursive max widths.

- [ ] **Step 3: Run the RED tests**

Run:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx src/hooks/useTaskbarStatusWidth.test.tsx --reporter=verbose
```

Expected: failures because no shared contents component or measurement root exists and the hook still recursively measures the visible element.

- [ ] **Step 4: Implement `TaskbarStatusContents`**

Move the complete avatar/identity/quota/reset/close geometry into the new component. Use the same classes in both modes. In measurement mode:

```tsx
<div
  ref={measurementRef}
  className="taskbar-status taskbar-status--measurement"
  data-testid="taskbar-status-measurement"
  aria-hidden="true"
  inert
>
  {/* shared geometry; buttons have tabIndex={-1}, no handlers, no nested test IDs */}
</div>
```

In visible mode:

```tsx
<div
  className="taskbar-status taskbar-status--visible"
  data-testid="taskbar-status-visible"
  data-trust={trustState}
>
  {/* same shared geometry with handlers, accessible names, and existing test IDs */}
</div>
```

Use one internal render path for the field sequence; do not copy the metric map into two independent branches. Keep error text inside the main geometry in both modes so a close-error width change is measurable.

- [ ] **Step 5: Compose visible and measurement modes in `TaskbarStatus`**

Keep label derivation and close state in `TaskbarStatus.tsx`. Replace `contentRef` with `measurementRef`, call:

```tsx
useTaskbarStatusWidth(measurementRef);
```

Return a fragment containing visible mode first and measurement mode second. Pass handlers only to visible mode.

- [ ] **Step 6: Implement isolated CSS contracts**

Change the shared base to geometry/tokens only. Add:

```css
.taskbar-status--visible {
  width: 100%;
  max-width: 100%;
}

.taskbar-status--measurement {
  position: fixed;
  left: -10000px;
  top: -10000px;
  width: max-content;
  max-width: 318px;
  visibility: hidden;
  pointer-events: none;
  contain: layout style;
}
```

The quota track keeps `max-width: 166px` and overflow containment. Do not apply `display: none`, `content-visibility: hidden`, transforms, or root opacity to the replica because they invalidate geometry. Keep the visible close as the final outer sibling.

- [ ] **Step 7: Simplify the width hook**

Delete `intrinsicWidth` and every descendant traversal. Add one replica measurement helper:

```ts
function measuredReplicaWidth(
  element: HTMLElement,
  entry?: ResizeObserverEntry,
): number | null {
  const border = entry ? borderBoxWidth(entry) : null;
  const width = Math.max(
    border ?? 0,
    element.getBoundingClientRect().width,
    element.scrollWidth,
  );
  return Number.isFinite(width) && width > 0 ? Math.round(width) : null;
}
```

Observe the replica, submit one initial measurement immediately, and preserve the existing serialized queue. If `ResizeObserver` is missing, return without calling the bridge or emitting a frontend console log. The native 318 fallback remains unchanged; actual native resize and positioning failures continue through existing Rust `tracing` paths.

- [ ] **Step 8: Run GREEN, regression, and build checks**

Run:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx src/hooks/useTaskbarStatusWidth.test.tsx --reporter=verbose
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run build
git diff --check
```

Expected: focused tests pass, the full frontend count is at least the current 168 tests, and the production build passes.

- [ ] **Step 9: Commit Task 2**

```powershell
git add apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx apps/desktop-tauri/src/surfaces/TaskbarStatus.css apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx
git diff --cached --name-only
git commit -m "Measure taskbar content off-screen"
```

---

### Task 3: Prove the Redesign and Complete the Blocked Release

**Files:**
- Modify: `docs/WINDOWS_PROOF.md`
- Create/modify: `docs/verification/windows/2026-08-14/cua-observations.md`
- Create: `docs/verification/windows/2026-08-14/screenshots/taskbar-weekly-before-probe.png`
- Create: `docs/verification/windows/2026-08-14/screenshots/taskbar-weekly-probe-20.png`
- Create remaining final evidence screenshots from the original Task 9 brief
- Modify source files from Tasks 1–2 only if fresh evidence reveals a new defect and a new RED test precedes the fix

**Interfaces:**
- Consumes: Task 1 safe fallback and Task 2 measurement replica.
- Produces: fresh Windows acceptance evidence, release NSIS hash, installed current-user app verification, and completion or an explicit failed criterion.

- [ ] **Step 1: Update proof documentation before execution**

Replace the obsolete 168-pixel default wording with:

```text
The taskbar starts from a functional 318px failure fallback. An inert off-screen
replica supplies the compact content width; the visible surface is never
measured. A successful weekly proof normally shrinks below 318px.
```

Document that pre-probe screenshots are historical failures and cannot serve as passing evidence.

- [ ] **Step 2: Run focused and full source verification**

Run the original Task 9 focused commands, then:

```powershell
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
```

If the combined helper fails only because Corepack refuses the declared version, run the exact pinned pnpm CJS test/build plus both Cargo test/clippy manifests and `cargo fmt --all -- --check`; record every substitute command.

- [ ] **Step 3: Build a fresh debug desktop binary**

Stop only the app under test, then build:

```powershell
Get-Process -Name codex-barbar -ErrorAction SilentlyContinue | Stop-Process -Force
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run tauri:build:debug
```

Record the debug executable SHA-256. Do not reuse any binary or screenshot created before the Task 2 commit.

- [ ] **Step 4: Capture the decisive weekly taskbar proof**

Launch with `CODEXBAR_PROOF_MODE=taskbar-status:weekly`. Prefer CUA Driver at the documented exact path; if absent, use the documented Win32/UIA plus `PrintWindow` fallback and state the limitation.

After the window settles, assert and record:

```text
Identity: ProofU
Quota: 周 98%
Reset: 8/20
Close: visible and adjacent to content
5H: absent
Height: 40 logical pixels
Width: 104..318 logical pixels, and strictly below 318 for this weekly proof
Rect: inside the taskbar-safe region
```

Save the new passing screenshot as `taskbar-weekly-probe-20.png`. Preserve one prior failing screenshot as `taskbar-weekly-before-probe.png` and mark it FAIL/historical in the observation record. A 318-pixel weekly result is functional fallback evidence, not compact-layout PASS evidence.

If any required field is clipped, stop. Do not proceed to release or attempt another geometry patch without a new architecture review.

- [ ] **Step 5: Complete the remaining Windows proof**

Only after Step 4 passes, execute the remaining original Task 9 scenarios:

- taskbar opacity 0 and 80;
- float-ball weekly collapsed and expanded, fixed 88x88 and 260x148;
- Settings General 680x500, Chinese sidebar, independent sliders, close, arrows, native Enter and Space single activation, and Escape;
- taskbar/float close persistence outside proof mode;
- float-ball drag/click and dark-theme isolation;
- Explorer/DPI reposition only when the documented non-destructive helper can prove it; otherwise record a limitation.

Committed screenshots use only the proof identity and contain no account data.

- [ ] **Step 6: Re-run official resolver and live quota checks**

Record before/after hash and modification time of the OpenCodex wrapper without recording its full content. Verify:

```powershell
codex --version
```

Then run the repository's redacted App Server smoke and a normal app refresh. Assert `verifiedNpmLayout`, installed Codex version, signed-in state, rate-limit availability, and a newly persisted successful snapshot. Compare weekly period, remaining percent, and reset date at the same observation time without automating the Codex desktop UI or recording raw protocol.

If App Server smoke succeeds but normal persistence fails, use `superpowers:systematic-debugging`; do not claim live quota equality until the persistence boundary is explained and fixed or explicitly marked failed.

- [ ] **Step 7: Build and verify the production installer**

Run:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run tauri:build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-artifacts.ps1
```

Expected NSIS artifact:

```text
target\release\bundle\nsis\codex-barbar_1.0.0_x64-setup.exe
```

Record SHA-256. Do not commit the installer binary.

- [ ] **Step 8: Install current-user coverage without deleting data**

Use the repository smoke path or silent NSIS current-user mode. Do not select any user-data deletion option. Confirm:

- `%LOCALAPPDATA%\Programs\codex-barbar` remains the install root;
- no elevation is required;
- existing enable flags, account cache, and opacity values survive;
- installed executable hash/timestamp corresponds to the release build;
- uninstall registration remains present;
- the launched process path is the installed release, not the debug worktree.

- [ ] **Step 9: Run post-install verification and write evidence**

Stop any debug process, repeat focused status tests, read the installed process path/version/hash, and complete `cua-observations.md` with PASS/FAIL for every criterion, tool/fallback status, DPI/orientation, rectangles, opacity values, live quota method, installer hash, and limitations.

- [ ] **Step 10: Commit Task 3 evidence**

Stage only the verified documentation and screenshots:

```powershell
git add docs/WINDOWS_PROOF.md docs/verification/windows/2026-08-14
git diff --cached --name-only
git commit -m "Verify off-screen taskbar measurement"
```

Do not stage installers, databases, logs, raw protocol, account screenshots, or `.superpowers` reports.

- [ ] **Step 11: Run the final review gate**

Generate one full review package from the redesign merge base through HEAD. The final reviewer must check:

- every acceptance criterion in the measurement design and parent status-surface spec;
- resolver signature security and OpenCodex preservation;
- settings atomicity and opacity bounds;
- dynamic real-quota rendering and accessible bands;
- taskbar/float lifecycle and close paths;
- proof authenticity, installer hash, installed process path, and unrelated-file scope.

Do not declare completion unless the final reviewer reports no Critical or Important findings and the worktree contains no unexpected tracked or untracked files.
