# Status Surface Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair live taskbar opacity, isolate status-surface proof runtime from persistent user flags, replace contaminated close evidence with separate success/rollback scenarios, and reopen release only after complete Windows proof.

**Architecture:** Apply the runtime alpha to the actual taskbar render root, derive deterministic status-proof settings from one Rust projection shared by activation and synthetic bootstrap, and keep close success separate from deliberate persistence-failure rollback. Product code gains no command, capability, permission, dependency, or native whole-window alpha; CDP, CUA, DPI-aware Win32, and screen-composited evidence each prove only their own layer.

**Tech Stack:** React 18, TypeScript, Vitest 3, Tauri 2.10, Rust 2024, WebView2 CDP, Windows CUA/Win32, pnpm 10.18.1, NSIS.

## Global Constraints

- The approved source of truth is `docs/superpowers/specs/2026-08-15-status-surface-repair-design.md`.
- Do not add product commands, capabilities, permissions, dependencies, package-manager artifacts, or native whole-window opacity.
- `taskbar-status:*` proof means taskbar enabled and float ball disabled in runtime only; `float-ball:*` means float ball enabled and taskbar disabled in runtime only.
- Status proof activation must never call `SettingsRepository::update` or mutate the four persistent surface fields.
- Keep taskbar logical constants exact: minimum 104, maximum 318, safe fallback 318, height 40, measurement helper 318x40.
- Keep the 2-second reconciliation interval, measurement-first shutdown, exact width caller authorization, and identity-safe deferred helper cleanup unchanged.
- Keep float-ball `.theme(Some(tauri::Theme::Dark))`, saved collapsed position, drag threshold, expansion dimensions, and monitor behavior unchanged.
- Proof data must remain synthetic and credential-free. Do not log or capture raw protocol, cookies, tokens, full account identity, database content, private paths, or raw storage/WebView errors.
- Use repository-pinned pnpm 10.18.1 and Node 20-compatible commands. Do not create npm or yarn lockfiles.
- Tasks 1 and 2 use RED -> minimal GREEN -> refactor. Task 3 adds characterization coverage for already-correct close semantics. Every task receives a fresh independent review before the next task.
- Do not stage any pre-existing or invalid untracked PNG under `docs/verification/windows/2026-08-14/screenshots`.
- Any failed or inconclusive Windows gate stops release, installation, and push.

---

### Task 1: Attach Runtime Opacity to the Rendered Taskbar Root

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx`
- Create: `.superpowers/sdd/2026-08-15-status-surface-repair/task-1-report.md`

**Interfaces:**
- Consumes: `TaskbarStatusPresentation.surfaceAlpha: string` from `taskbarStatusPresentation.ts`.
- Produces: the custom property `--surface-bg-alpha` directly on both `.taskbar-status--visible` and `.taskbar-status--measurement` roots.
- Preserves: `TaskbarStatusContentsProps`, visible/measurement DOM order, measurement ref ownership, close handlers, and the width bridge.

- [ ] **Step 1: Write the RED root-ownership tests**

Replace the existing host-only opacity assertion in `TaskbarStatus.test.tsx`:

```tsx
it.each([[0, "0"], [20, "0.2"], [80, "0.8"]])(
  "attaches taskbar opacity %s to the rendered root as alpha %s",
  async (opacity, expectedAlpha) => {
    const bootstrap = readyTwoWindowFixture();
    bootstrap.settings.taskbarStatusOpacity = opacity;
    invokeMock.mockResolvedValue(bootstrap);
    render(<TaskbarStatus />);

    const visible = await screen.findByTestId("taskbar-status-visible");
    await within(visible).findByText("5H 42%");
    expect(visible.style.getPropertyValue("--surface-bg-alpha")).toBe(expectedAlpha);
    expect(visible.parentElement?.style.getPropertyValue("--surface-bg-alpha")).toBe("");
    expect(visible.style.opacity).toBe("");
  },
);
```

Add the equivalent helper assertion in `TaskbarStatusMeasure.test.tsx`:

```tsx
it("uses the same runtime alpha on the independent measurement root", async () => {
  const bootstrap = bootstrapWithTwoProfiles();
  bootstrap.settings.taskbarStatusOpacity = 80;
  invokeMock.mockResolvedValue(bootstrap);
  render(<TaskbarStatusMeasure />);

  const measurement = await screen.findByTestId("taskbar-status-measurement");
  expect(measurement.style.getPropertyValue("--surface-bg-alpha")).toBe("0.8");
});
```

- [ ] **Step 2: Run the focused tests and record the expected RED**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx src/surfaces/TaskbarStatusMeasure.test.tsx --reporter=verbose
```

Expected: the assertions fail because the variable remains on
`.taskbar-status-host`, while the actual `.taskbar-status` element has no inline
runtime variable and continues to use the authored `0.2` fallback.

- [ ] **Step 3: Move the runtime value to the production render root**

In `TaskbarStatusContents.tsx`, import `CSSProperties` as a type and attach the
presentation alpha to the root that owns `.taskbar-status`:

```tsx
import type React from "react";
import type { CSSProperties } from "react";

style={{
  "--surface-bg-alpha": presentation.surfaceAlpha,
} as CSSProperties}
```

In `TaskbarStatus.tsx`, remove the custom-property style from the host:

```tsx
return (
  <div className="taskbar-status-host" data-testid="taskbar-status-content">
    <TaskbarStatusContents
      mode="visible"
      presentation={presentation}
      closeFailed={closeFailed}
      onOpen={() => void surface.openPanel()}
      onClose={closeSurface}
    />
  </div>
);
```

Keep the CSS fallback on `.taskbar-status`: inline style on the same element
wins, while markup without runtime data still gets 20%.

- [ ] **Step 4: Run GREEN, regression, and build checks**

```powershell
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx src/surfaces/TaskbarStatusMeasure.test.tsx src/hooks/useStatusSurface.test.tsx src/hooks/useTaskbarStatusWidth.test.tsx --reporter=verbose
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
```

Expected: all frontend tests pass; both roots carry runtime alpha; visible still
never calls `set_taskbar_status_width`.

- [ ] **Step 5: Write the report and commit only Task 1**

Record RED, GREEN counts, full frontend count, build, files, and root cause.

```powershell
git add apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx `
        apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx `
        apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx `
        apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx
git diff --cached --check
git commit -m "Fix live taskbar opacity"
```

---

### Task 2: Make Status Proof Runtime Deterministic and Non-Persistent

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Create: `.superpowers/sdd/2026-08-15-status-surface-repair/task-2-report.md`

**Interfaces:**
- Produces: `StatusProofProjection`, `status_proof_projection`, `status_proof_settings`, and `apply_status_proof_with` inside `proof_harness.rs`.
- Consumes: `ProofScenario`, `AppSettings`, `status_surfaces::apply_status_surface_settings`, and `AppSettingsDto::from_settings`.
- Guarantees: proof mutual exclusion, opacity 20, and no repository write path.

- [ ] **Step 1: Write RED projection and bootstrap tests**

```rust
#[test]
fn status_proof_projection_is_mutually_exclusive_and_deterministic() {
    let taskbar = status_proof_projection(ProofScenario::TaskbarStatus(
        StatusProofState::Weekly,
    ))
    .unwrap();
    assert_eq!(
        taskbar,
        StatusProofProjection {
            taskbar_status_enabled: true,
            float_ball_enabled: false,
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
        }
    );

    let float = status_proof_projection(ProofScenario::FloatBall(
        StatusProofState::Weekly,
    ))
    .unwrap();
    assert!(!float.taskbar_status_enabled);
    assert!(float.float_ball_enabled);
    assert_eq!((float.taskbar_status_opacity, float.float_ball_opacity), (20, 20));
    assert!(status_proof_projection(ProofScenario::SettingsGeneral).is_none());
}
```

Strengthen the weekly bootstrap test:

```rust
match scenario {
    ProofScenario::TaskbarStatus(_) => {
        assert!(bootstrap.settings.taskbar_status_enabled);
        assert!(!bootstrap.settings.float_ball_enabled);
    }
    ProofScenario::FloatBall(_) => {
        assert!(!bootstrap.settings.taskbar_status_enabled);
        assert!(bootstrap.settings.float_ball_enabled);
    }
    _ => unreachable!(),
}
```

Add a production-seam test:

```rust
#[test]
fn status_proof_activation_calls_only_the_runtime_projection() {
    let applied = std::cell::RefCell::new(Vec::new());
    apply_status_proof_with(
        ProofScenario::FloatBall(StatusProofState::Weekly),
        |settings| {
            applied.borrow_mut().push((
                settings.taskbar_status_enabled,
                settings.float_ball_enabled,
            ));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(*applied.borrow(), [(false, true)]);
}
```

- [ ] **Step 2: Run the RED Rust suite**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
```

Expected: missing projection/seam symbols and default-disabled bootstrap flags.

- [ ] **Step 3: Implement the shared projection**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusProofProjection {
    taskbar_status_enabled: bool,
    float_ball_enabled: bool,
    taskbar_status_opacity: u8,
    float_ball_opacity: u8,
}

fn status_proof_projection(scenario: ProofScenario) -> Option<StatusProofProjection> {
    match scenario {
        ProofScenario::TaskbarStatus(_) => Some(StatusProofProjection {
            taskbar_status_enabled: true,
            float_ball_enabled: false,
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
        }),
        ProofScenario::FloatBall(_) => Some(StatusProofProjection {
            taskbar_status_enabled: false,
            float_ball_enabled: true,
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
        }),
        _ => None,
    }
}

fn status_proof_settings(scenario: ProofScenario) -> Option<codexbar::storage::AppSettings> {
    let projection = status_proof_projection(scenario)?;
    Some(codexbar::storage::AppSettings {
        taskbar_status_enabled: projection.taskbar_status_enabled,
        float_ball_enabled: projection.float_ball_enabled,
        taskbar_status_opacity: projection.taskbar_status_opacity,
        float_ball_opacity: projection.float_ball_opacity,
        ..codexbar::storage::AppSettings::default()
    })
}

fn apply_status_proof_with(
    scenario: ProofScenario,
    apply_runtime: impl FnOnce(&codexbar::storage::AppSettings) -> Result<(), String>,
) -> Result<(), String> {
    let settings = status_proof_settings(scenario)
        .ok_or_else(|| "PROOF_STATUS_SCENARIO_UNAVAILABLE".to_string())?;
    apply_runtime(&settings)
}
```

Use it in `activate`:

```rust
SurfaceMode::Hidden => apply_status_proof_with(config.scenario, |settings| {
    crate::status_surfaces::apply_status_surface_settings(app, settings)
}),
```

Use it in `synthetic_bootstrap`:

```rust
let settings = status_proof_settings(scenario)
    .map(|settings| crate::commands::AppSettingsDto::from_settings(&settings))
    .unwrap_or_default();
```

Assign `settings` to the returned bootstrap. Do not import a repository.

- [ ] **Step 4: Make proof failure logging stable and non-sensitive**

```rust
match result {
    Ok(()) => tracing::info!(
        code = "PROOF_ACTIVATION_SUCCEEDED",
        "proof activation completed"
    ),
    Err(code) => tracing::error!(
        code = code.as_str(),
        "proof activation failed"
    ),
}
```

- [ ] **Step 5: Run GREEN and complete native checks**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Expected: no dependency, registry, capability, dimension, theme, or interval diff.

- [ ] **Step 6: Write the report and commit Task 2**

```powershell
git add apps/desktop-tauri/src-tauri/src/proof_harness.rs
git diff --cached --check
git commit -m "Isolate status surface proofs"
```

---

### Task 3: Separate Close Success from Persistence-Failure Rollback

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `docs/WINDOWS_PROOF.md`
- Create: `.superpowers/sdd/2026-08-15-status-surface-repair/task-3-report.md`

**Interfaces:**
- Consumes: `controller::transition`, `SurfaceRuntime`, `SurfaceSettingsStore`, and the frontend `disableSurface("taskbarStatus")` path.
- Produces: successful-disable, persistence-rollback, fixed-size retry, and unlocked/locked Windows proof contracts.
- Does not change: controller algorithm, stable errors, lifecycle, command registry, or CSS geometry.

- [ ] **Step 1: Add the successful-disable controller contract**

```rust
#[test]
fn successful_disable_persists_false_without_rollback() {
    let mut runtime = FakeRuntime::enabled();
    let store = FakeStore::with_settings(settings(true, false));
    let saved = transition(
        &mut runtime,
        &store,
        StatusSurfaceKind::TaskbarStatus,
        false,
    )
    .unwrap();

    assert!(!saved.taskbar_status_enabled);
    assert_eq!(store.save_count(), 1);
    assert_eq!(runtime.calls(), &[(StatusSurfaceKind::TaskbarStatus, false)]);
    assert!(runtime.forced().is_empty());
}
```

Strengthen `persistence_failure_restores_previous_runtime_state`:

```rust
assert!(store.saved().taskbar_status_enabled);
assert_eq!(store.save_count(), 0);
assert_eq!(
    runtime.calls(),
    &[
        (StatusSurfaceKind::TaskbarStatus, false),
        (StatusSurfaceKind::TaskbarStatus, true),
    ]
);
```

- [ ] **Step 2: Add the separate frontend success assertion**

```tsx
it("keeps a successful close out of the retry error state", async () => {
  const bootstrap = readyTwoWindowFixture();
  invokeMock.mockImplementation(async (command: string) => {
    if (command === "get_bootstrap_state") return bootstrap;
    if (command === "set_status_surface_enabled") {
      return { ...bootstrap.settings, taskbarStatusEnabled: false };
    }
    return undefined;
  });
  render(<TaskbarStatus />);

  const close = await screen.findByRole("button", { name: "关闭任务栏状态" });
  fireEvent.click(close);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", {
      surface: "taskbarStatus",
      enabled: false,
    }),
  );
  expect(close).not.toHaveAttribute("data-error");
  expect(screen.getByRole("status")).toHaveTextContent("");
});
```

Keep the existing rejection/retry test unchanged.

- [ ] **Step 3: Run focused tests before documentation**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::controller -- --nocapture
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/TaskbarStatus.test.tsx --reporter=verbose
```

Expected: the current algorithm passes. If an assertion exposes a production
failure, stop and debug before editing the controller.

- [ ] **Step 4: Update the exact Windows close contract**

Add to `docs/WINDOWS_PROOF.md`:

```text
Unlocked frontend close:
  CDP clicks the real taskbar close button with no SQLite writer.
  visible + measurement targets disappear and persisted enabled becomes false.

Unlocked native close:
  WM_CLOSE is sent to the exact visible root HWND with no SQLite writer.
  the same typed controller converges to false and destroys both HWNDs.

Locked persistence retry:
  an exclusive diagnostic writer is acquired and verified before the click.
  first click rolls back to true and exposes the fixed-size red retry state.
  release is observed before the second click; retry converges to false.
```

State that locked rollback preserving `true` is expected, not normal failure.

- [ ] **Step 5: Run regression and commit Task 3**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
cargo fmt --all -- --check
git diff --check
git add apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs `
        apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx `
        docs/WINDOWS_PROOF.md
git diff --cached --check
git commit -m "Separate status close proof states"
```

---

### Task 4: Run Full Source Gates and Fresh Windows Repair Proof

**Files:**
- Create: `docs/verification/windows/2026-08-15/status-surface-repair-observations.md`
- Create: `docs/verification/windows/2026-08-15/screenshots/taskbar-repair-weekly.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/taskbar-repair-opacity-0.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/taskbar-repair-opacity-80.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/taskbar-repair-close-retry.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/float-repair-weekly-collapsed.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/float-repair-weekly-expanded.png`
- Create after weekly PASS: `docs/verification/windows/2026-08-15/screenshots/settings-repair-general.png`
- Create: `.superpowers/sdd/2026-08-15-status-surface-repair/task-4-report.md`

**Interfaces:**
- Consumes: reviewed opacity root ownership, proof projection, and close proof contract.
- Produces: source-gate evidence, fresh debug hash, privacy-reviewed screenshots, and binary GO/STOP release decision.

- [ ] **Step 1: Record the clean starting boundary**

Record only:

```text
HEAD SHA
git status --short
debug/release/installed process paths and counts
the four surface settings
absence of an external SQLite writer
```

Do not print complete settings JSON or account data. Preserve all existing
2026-08-14 untracked screenshots.

- [ ] **Step 2: Run focused and complete source gates**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server
cargo test --manifest-path rust/Cargo.toml storage::settings_repository
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces
pnpm --dir apps/desktop-tauri test
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
```

Record exact counts and final exit line. Partial or timed-out output is not PASS.

- [ ] **Step 3: Build a fresh debug binary**

Stop only an exact app-under-test process. Run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
Get-FileHash target\debug\codex-barbar.exe -Algorithm SHA256
```

Record hash, size, timestamp, and exact launched path.

- [ ] **Step 4: Pass the decisive taskbar weekly gate**

Launch `CODEXBAR_PROOF_MODE=taskbar-status:weekly`. Use CUA first; after the
documented no-activate retry limit, use DPI-aware Win32 and screen evidence.

```text
measurement exists, hidden, and 318x40 logical
float-ball is absent or hidden
visible row is ProofU | 周 98% | 8/20 | ×
5H is absent
visible logical width is 104..317 and height is 40
visible rect lies between fresh task-list and notification endpoints
```

Save `taskbar-repair-weekly.png`. Any failure writes STOP and ends Task 4.

- [ ] **Step 5: Prove live opacity in a clean Settings proof run**

Launch `CODEXBAR_PROOF_MODE=settings:general` with a loopback-only WebView2 CDP
port in the process environment. Never commit or externally expose the port.

1. Enable taskbar through the real command.
2. Set opacity 0 and read `.taskbar-status` inline/computed alpha through CDP.
3. Capture the exact native screen-composited rect.
4. Set opacity 80 and repeat.
5. Assert no width command, rebuild, text change, or geometry change.

```text
runtime root values are 0 and 0.8
computed background alpha changes
screen captures differ in bytes and SHA-256
pixel comparison shows material background composition change
```

Save both opacity captures and a privacy-safe Settings screenshot.

- [ ] **Step 6: Prove all three close scenarios**

- **Unlocked frontend:** CDP clicks `.taskbar-status__close`; both targets/HWNDs
  disappear and persisted enabled becomes false.
- **Unlocked native:** re-enable, send `WM_CLOSE` to exact visible root with no
  writer; both HWNDs disappear and persisted false is observed.
- **Locked retry:** re-enable, acquire and verify an exclusive writer, click via
  CDP, assert `data-error=true`, title/live text `关闭失败，点击重试`, both HWNDs
  and persisted true remain. Release and observe the lock release; retry must
  destroy both and persist false.

Save only the valid red-state screenshot.

- [ ] **Step 7: Pass isolated float-ball proof**

Launch `CODEXBAR_PROOF_MODE=float-ball:weekly` and prove:

```text
taskbar visible/helper windows are absent or hidden
float-ball IsWindowVisible == true
collapsed content is weekly-only, no 5H, safe size/position
expanded card includes weekly quota/reset/update rows
click opens panel without drag
drag moves collapsed window without opening panel
close destroys float-ball
0/80 screen-composited captures differ
Settings/other WebViews retain intended theme while float remains Dark
```

Use CUA for targetable interactions. CDP proves DOM/command state but cannot
replace native visibility or screen composition.

- [ ] **Step 8: Restore state and commit truthful evidence**

Restore four surface fields, release writer, stop only debug app, and confirm no
DevTools/app-under-test process remains. Inspect images for identity, secrets,
and paths. Write exact commands/counts/hashes/rectangles/tool limits.

```powershell
git add docs/verification/windows/2026-08-15/status-surface-repair-observations.md `
        docs/verification/windows/2026-08-15/screenshots
git diff --cached --check
git commit -m "Verify status surface repairs"
```

Task 5 is authorized only when every required item is PASS.

---

### Task 5: Verify Live Data, Build Release, and Install Current User

**Files:**
- Modify: `docs/verification/windows/2026-08-15/status-surface-repair-observations.md`
- Create: `.superpowers/sdd/2026-08-15-status-surface-repair/task-5-report.md`

**Interfaces:**
- Consumes: independently reviewed Task 4 PASS evidence.
- Produces: resolver/live quota proof, production NSIS hash, current-user install, post-install proof, and final review.

- [ ] **Step 1: Verify resolver and live quota without changing wrappers**

Record only SHA-256 and modification time of the existing OpenCodex wrapper
before and after. Run `codex --version`, repository redacted App Server smoke,
and a normal refresh. Assert:

```text
installation is verifiedNpmLayout or nativeExe
installed Codex version is detected
signed-in state and rate limits are available
a new successful snapshot is persisted
weekly period, remaining percent, and reset agree at one observation time
```

Do not record protocol, token, cookie, full identity, or full private path. A
smoke/persistence mismatch invokes systematic debugging and stops release.

- [ ] **Step 2: Build and verify production NSIS**

```powershell
pnpm --dir apps/desktop-tauri run tauri:build
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-artifacts.ps1
```

Expected:

```text
target\release\bundle\nsis\codex-barbar_1.0.0_x64-setup.exe
```

Record hash and size; do not commit the installer.

- [ ] **Step 3: Install current-user without deleting data**

Use the repository current-user smoke or NSIS current-user silent mode. Do not
request elevation and do not select data deletion. Confirm:

```text
install root is %LOCALAPPDATA%\Programs\codex-barbar
four surface settings and account cache survive
uninstall registration remains
installed hash/timestamp matches release build
running path is installed release, not debug worktree
```

- [ ] **Step 4: Run post-install source and Windows verification**

Stop debug processes. Repeat focused taskbar/status/proof/frontend tests. Verify
installed version/path/hash, dual taskbar windows, hidden helper, live opacity,
normal close, visible/interactable float ball, and successful quota refresh.

- [ ] **Step 5: Update and commit release evidence**

Append exact commands/counts, resolver classification, redacted live quota,
installer hash, installed path, settings survival, and limitations.

```powershell
git add docs/verification/windows/2026-08-15/status-surface-repair-observations.md
git diff --cached --check
git commit -m "Verify repaired status surface release"
```

- [ ] **Step 6: Run final independent review**

Generate a review package from repair-spec commit `9f000380` through HEAD.
Review the repair and independent-measurement specs, settings atomicity, proof
non-persistence, resolver signature security, provider isolation, lifecycle,
screenshots, release hash, installed path, and unrelated-file scope.

Completion requires:

```text
no Critical or Important findings
full local CI PASS
fresh Windows proof with no required inconclusive item
verified production NSIS and current-user installation
successful post-install live quota refresh
```

Otherwise report the exact blocker and do not declare completion.
