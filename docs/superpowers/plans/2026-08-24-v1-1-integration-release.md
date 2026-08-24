# v1.1 Integration, Upgrade, and Release Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax - [ ] for tracking.

**Goal:** Integrate the approved v1.1 identity, surface, pricing, and lifecycle work into a localized, upgrade-safe, Windows-proven build that is ready for release authorization.

**Architecture:** This plan owns cross-cutting validation rather than adding a second product architecture. It verifies settings migration from v1.0.26 data, language coverage, shared presentation parity, release-manifest consistency, and native Windows behavior; any defect is returned to the task that owns its boundary.

**Tech Stack:** Rust 2024, Tauri 2, React 18, TypeScript, Vitest, PowerShell release scripts, NSIS, GitHub Actions, CUA Driver.

**Spec:** docs/superpowers/specs/2026-08-24-v1-1-identity-pricing-surfaces-design.md

## Global Constraints

- Follow docs/superpowers/plans/2026-08-24-v1-1-rollout-index.md and complete plans 1–3 before this plan.
- Do not create a tag, GitHub Release, remote push, Winget submission, or installer replacement without explicit current user authorization.
- Do not modify user accounts, Codex settings, credits, notification permissions, or browser data during proof.
- All release-relevant behavior must work on a fresh Windows-native build; WebView/jsdom evidence is additive only.
- No secrets, cookies, raw app-server responses, account email, local logs, or external source payloads enter test snapshots, diagnostics, README screenshots, release notes, or commit messages.

---

### Task 1: Complete bilingual copy, accessibility, and surface parity

**Files:**
- Modify: apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts
- Modify: apps/desktop-tauri/src/surfaces/Settings.tsx
- Modify: apps/desktop-tauri/src/surfaces/TrayPanel.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.tsx
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.tsx
- Modify: apps/desktop-tauri/src/styles.css
- Modify: apps/desktop-tauri/src/surfaces/Settings.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/*.test.tsx

**Interfaces:**
- Produce a complete English/Simplified-Chinese SettingsCopy record for every v1.1 label, error, empty state, status, tooltip, and cost provenance.
- Preserve identical TaskbarStatusPresentation output for visible and measurement routes.
- Produce keyboard-operable range, action-reorder, avatar upload/remove, currency, and close controls.

- [ ] **Step 1: Write failing locale and parity tests.**

~~~tsx
it.each(["en-US", "zh-CN"] as const)("renders all v1.1 settings labels in %s", (language) => {
  const copy = settingsCopy(language);
  render(<Settings bootstrap={bootstrap({ language })} />);
  expect(screen.queryByText(copy.placeholder)).not.toBeInTheDocument();
});

it("keeps visible and measurement taskbar markup structurally identical", () => {
  const presentation = buildTaskbarStatusPresentation(surface());
  expect(renderContents("visible", presentation)).toMatchTaskbarStructure(renderContents("measurement", presentation));
});
~~~

- [ ] **Step 2: Run focused frontend tests and verify RED where copy is missing.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri test -- Settings TaskbarStatus TaskbarStatusMeasure FloatBall
~~~

Expected: any leftover 80-percent help copy, Taskbar & Tray wording, Menu wording, Cost unavailable label, or missing v1.1 locale key fails.

- [ ] **Step 3: Fill every localized state and remove obsolete copy.**

Audit all settings tabs, taskbar tooltip, tray panel, float aria labels, cost table,
notification labels, and About. Replace obsolete terms with Taskbar & Float
Ball / 任务栏与悬浮球, Panel / 面板, Cost / 费用, source-aware cost states, and
0–100 percent help. Keep the installed-version rendering sourced from bootstrap,
not localized text.

- [ ] **Step 4: Validate keyboard and reduced-motion interaction.**

Tab through every new control, use Arrow/Home/End on each range, operate panel
reorder buttons without drag and upload/remove avatar with keyboard focus. With
reduced motion enabled, confirm floating ball becomes static while status and
identity remain legible.

- [ ] **Step 5: Run full frontend tests/build and the UI detector, then commit.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
node C:/Users/stack/.codex/skills/impeccable/scripts/detect.mjs --json apps/desktop-tauri/src/surfaces apps/desktop-tauri/src/components apps/desktop-tauri/src/styles.css
git diff --check
~~~

Commit:

~~~powershell
git add apps/desktop-tauri/src
git commit -m "Polish v1.1 surface copy and accessibility"
~~~

### Task 2: Prove upgrade/migration and settings preservation

**Files:**
- Create: rust/src/storage/fixtures/app-settings-v1-0-26.json
- Modify: rust/src/storage/settings_migration.rs
- Modify: rust/src/storage/settings_repository.rs
- Modify: apps/desktop-tauri/src-tauri/src/proof_harness.rs
- Modify: docs/CONFIGURATION.md
- Test: rust/src/storage/settings_repository.rs
- Test: apps/desktop-tauri/src-tauri/src/commands/bridge.rs

**Interfaces:**
- Produce a sanitized v1.0.26 settings fixture containing old visual values, legacy tray options, tray panel order, language, theme, and full-screen preference.
- Produce a v2 bootstrap snapshot that contains v1.1 fields without exposing retired tray controls.
- Preserve unrelated values through each partial settings update.

- [ ] **Step 1: Write failing end-to-end migration tests.**

~~~rust
#[test]
fn v1_0_26_settings_upgrade_preserves_theme_language_and_fullscreen_behavior() {
    let settings = load_fixture("app-settings-v1-0-26.json");
    let migrated = migrate_settings_json(settings).unwrap().0;
    let app: AppSettings = serde_json::from_value(migrated).unwrap();
    assert_eq!(app.theme, ThemePreference::Dark);
    assert_eq!(app.language, LanguagePreference::ZhCn);
    assert!(app.taskbar_presentation.hide_status_surfaces_in_fullscreen);
    assert_eq!(app.surface_appearance.taskbar_transparency_percent, 25);
}
~~~

- [ ] **Step 2: Run migration/bridge tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
~~~

Expected: fixture and v1.1 bootstrap expectations are absent.

- [ ] **Step 3: Add the sanitized fixture and migration assertions.**

The fixture contains no real email, account ID, local path, source URL, or token.
Assert old 0–80 visual values scale once, legacy tray settings are ignored,
panel quick actions normalize Refresh, and a settings patch changes only the
intended v2 field.

- [ ] **Step 4: Document user-visible migration behavior.**

Update docs/CONFIGURATION.md with 0–100 semantics, fixed tray behavior, panel
personalization, avatar privacy boundary, pricing source/cache behavior, and
the read-only cost disclaimer. Do not include a live model-price table.

- [ ] **Step 5: Run full Rust checks and commit.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo fmt --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
~~~

Commit:

~~~powershell
git add rust/src/storage apps/desktop-tauri/src-tauri/src/proof_harness.rs docs/CONFIGURATION.md
git commit -m "Verify v1.1 settings upgrade"
~~~

### Task 3: Run the full native and application acceptance matrix

**Files:**
- Create: docs/verification/v1-1-windows-acceptance-template.md
- Modify: scripts/local-check.ps1 only if a verified v1.1 guard is absent and no existing command can enforce it.
- Test: existing Rust/TypeScript suites and scripts.

**Interfaces:**
- Produce a local, non-secret proof ledger with command result, build SHA, binary path, screenshot path, scenario, expected result, actual result, and timestamp.
- The ledger has no real account name, email, raw price response, session log, browser URL, or screenshot of unrelated private content.

- [ ] **Step 1: Create the acceptance template with exact scenarios.**

~~~markdown
| Scenario | Expected observable |
| --- | --- |
| Signed-out profile | Default product icon; no email |
| Signed-in profile | Avatar + username in panel/taskbar |
| Panel selector | Full email appears only here |
| Taskbar/float controls | 0–100 smooth slider and persistent value |
| Fixed tray | Dynamic band, fixed tooltip/menu |
| Cost table | direct/equivalent/unpriced/partial states |
| Start/desktop/Explorer/Edge | no click required to restore surfaces |
| Browser/video full-screen | hides only when preference is enabled |
| Fast transition | event observed and 3x state within 500ms |
~~~

- [ ] **Step 2: Run full local gate before native UI proof.**

Run:

~~~powershell
.scriptslocal-check.ps1
~~~

Expected: exit code 0. If it fails, stop this task, attach the first failure,
and return to the task that owns the component.

- [ ] **Step 3: Build the fresh installer and debug binary.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
.scriptswindows-release-build.ps1 -Version 1.1.0 -OutputDirectory .artifacts1-1-proof
.scriptserify-release-artifacts.ps1 -Version 1.1.0 -AssetsDirectory .artifacts1-1-proof
~~~

Expected: every artifact name, version resource, checksum, SBOM, and manifest
matches 1.1.0. Do not upload artifacts.

- [ ] **Step 4: Use CUA for all acceptance rows.**

Launch only the fresh binary, run each row of the template, save tightly scoped
screenshots, restart the app between persistence checks, and record result.
When a native condition cannot be driven by CUA, use an equivalent explicit
Windows procedure and mark the limitation in the ledger.

- [ ] **Step 5: Run installer smoke test in an isolated temporary install root.**

Run:

~~~powershell
.scriptswindows-smoke-install.ps1 -InstallerPath .artifacts1-1-proofcodex-barbar_1.1.0_x64-setup.exe -ExpectedVersion 1.1.0
~~~

Expected: silent install, executable version, upgrade, shortcut, and uninstall
checks pass. Do not remove or replace the user's ordinary installation.

- [ ] **Step 6: Commit documentation/guard changes only.**

~~~powershell
git add docs/verification scripts/local-check.ps1
git commit -m "Document v1.1 Windows acceptance"
~~~

Do not commit generated artifacts.

### Task 4: Prepare a release candidate without publishing it

**Files:**
- Modify: rust/Cargo.toml
- Modify: apps/desktop-tauri/package.json
- Modify: apps/desktop-tauri/src-tauri/Cargo.toml
- Modify: apps/desktop-tauri/src-tauri/tauri.conf.json
- Modify: Cargo.lock
- Modify: README.md
- Modify: CHANGELOG.md
- Modify: docs/BUILDING.md
- Test: scripts/release-doctor.ps1
- Test: scripts/assert-release-workflow.ps1

**Interfaces:**
- All product manifest versions equal 1.1.0.
- README/release notes say costs are local estimates, summarize new identity,
  fixed tray/panel, price sources, and Windows reliability behavior without
  claiming a provider invoice.
- The release workflow remains guarded by DEPENDABOT_ALERTS_TOKEN policy.

- [ ] **Step 1: Write failing version-consistency and documentation assertions.**

~~~powershell
.scriptselease-doctor.ps1 -Version 1.1.0 -AssetsDirectory .artifacts1-1-proof
rg -n "1\.0\.26|费用不可用|Taskbar & Tray|0% is most opaque; 80%" README.md docs apps/desktop-tauri rust
~~~

Expected: version doctor fails before manifests change and stale user-facing
phrases are listed before documentation is updated.

- [ ] **Step 2: Bump exactly the four manifest versions and regenerate lock data.**

~~~powershell
corepack pnpm@10.18.1 --dir apps/desktop-tauri install --lockfile-only
cargo check --workspace --locked
~~~

Keep package identity, NSIS behavior, release asset names, and existing
Dependabot-token guard unchanged.

- [ ] **Step 3: Write concise release candidate notes and README updates.**

Document new behavior, migration semantics, privacy boundary, pricing
provenance, offline cache behavior, Windows surface behavior, and upgrade
guidance. Do not embed personal screenshots or a live price snapshot.

- [ ] **Step 4: Run release doctor and static workflow policy guard.**

Run:

~~~powershell
.scriptsassert-release-workflow.ps1
.scriptselease-doctor.ps1 -Version 1.1.0 -AssetsDirectory .artifacts1-1-proof
git diff --check
~~~

Expected: both scripts exit 0.

- [ ] **Step 5: Commit the release candidate manifest/docs.**

~~~powershell
git add rust/Cargo.toml apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/Cargo.toml apps/desktop-tauri/src-tauri/tauri.conf.json Cargo.lock README.md CHANGELOG.md docs/BUILDING.md
git commit -m "Prepare v1.1.0 release candidate"
~~~

### Task 5: Final verification and authorization boundary

**Files:**
- Verification evidence only.

**Interfaces:**
- Consume the complete branch, proof ledger, and release candidate artifacts.
- Produce a factual handoff: commit SHA, commands, result, known limitations,
  artifact hashes, and explicit user authorization request.

- [ ] **Step 1: Verify branch identity and clean state.**

Run:

~~~powershell
git status --short --branch
git log --oneline origin/main..HEAD
git diff --check origin/main...HEAD
~~~

Expected: only scoped v1.1 commits; no generated artifacts, credentials, or
unrelated files.

- [ ] **Step 2: Run all final automated gates.**

Run:

~~~powershell
.scriptslocal-check.ps1
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
.scriptserify-release-artifacts.ps1 -Version 1.1.0 -AssetsDirectory .artifacts1-1-proof
~~~

Expected: every command exits 0 with fresh output.

- [ ] **Step 3: Re-read the spec acceptance criteria against evidence.**

For each criterion 1–13, attach one automated test, CUA observation, or
installer evidence row. Identify any unperformed item as unperformed; do not
infer native verification from a unit test.

- [ ] **Step 4: Stop before external release actions.**

Report the verified candidate SHA and artifacts to the user. Ask explicitly for
permission to push, tag v1.1.0, invoke the release workflow, publish a GitHub
Release, or install/replace software. Do not perform any of those actions in
this task.
