# Settings Completion Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Windows notifications truthful and recoverable, correct taskbar/float-ball transparency semantics, and make both transparency sliders smooth before completing the remaining settings modules.

**Architecture:** Add a typed Windows notification-capability boundary in the Tauri shell so delivery never reports success when Windows suppresses the app. Preserve legacy settings field names but centralize transparency-to-alpha conversion, then decouple slider preview state from asynchronous persistence so drag frames stay local and storage commits only at interaction boundaries.

**Tech Stack:** Rust 2024, Tauri 2, Windows registry/fixed settings URI, React 18, TypeScript, Vitest, existing CSS status surfaces, Windows Computer Use/CUA proof.

**Spec:** `docs/superpowers/specs/2026-08-23-settings-feature-expansion-design.md`

## Global Constraints

- Do not change Windows notification permission; detect it and direct the user to the fixed Windows notification settings surface.
- Do not report test-notification success when Windows app/global notifications are disabled.
- Do not add a reset, purchase, account mutation, arbitrary URL, arbitrary command, or secret-bearing DTO.
- User-facing transparency is monotonic: 0 is most opaque, 80 is highly transparent.
- Keep legacy bridge/storage keys `taskbarStatusOpacity` and `floatBallOpacity` for compatibility.
- Drag frames update local preview only; persistence happens on pointer release, keyboard commit, or blur.
- Preserve the Night instrument cluster design, bilingual English/Chinese copy, keyboard focus, and reduced motion.
- No new dependency or Tauri permission without explicit approval.
- Do not publish another version until this plan and the existing Taskbar & Tray, Menu, Usage & Spend, and integration plans all pass final Windows proof.

---

### Task 1: Detect Windows notification suppression and expose recovery

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/notification_controller.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/fixed_actions.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts`

**Interfaces:**
- Produce `NotificationCapabilityDto { status: "available" | "appDisabled" | "globalDisabled" | "unsupported"; canOpenSettings: boolean }`.
- Produce `get_notification_capability() -> NotificationCapabilityDto` and fixed `open_windows_notification_settings() -> Result<(), String>` commands.
- `WindowsToastSink::send` must return `NOTIFICATION_PERMISSION_DISABLED` before starting PowerShell when the capability is disabled.

- [ ] Write failing Rust tests using an injected registry/capability reader for app `Enabled=0`, global `ToastEnabled=0`, missing/default-enabled keys, and unsupported platform behavior.
- [ ] Run `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml notification_controller::tests -- --nocapture`; verify RED because capability types and preflight do not exist.
- [ ] Implement the minimal capability reader, preflight, typed commands, and fixed `ms-settings:notifications` opener with no arbitrary URI input.
- [ ] Write failing frontend tests proving app-disabled test attempts show the localized recovery message/action and never show “sent”.
- [ ] Implement capability loading and recovery UI; re-query after the window regains focus so returning from Windows Settings refreshes state.
- [ ] Run focused Rust/frontend tests, full relevant suites, fmt/clippy/build, and `git diff --check`; commit `Fix Windows notification capability reporting`.

### Task 2: Correct transparency mapping and smooth slider interaction

**Files:**
- Create: `apps/desktop-tauri/src/lib/surfaceTransparency.ts`
- Create: `apps/desktop-tauri/src/lib/surfaceTransparency.test.ts`
- Create: `apps/desktop-tauri/src/hooks/useCommittedRange.ts`
- Create: `apps/desktop-tauri/src/hooks/useCommittedRange.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts`
- Modify: `apps/desktop-tauri/src/styles.css`

**Interfaces:**
- Produce `surfaceAlphaFromTransparency(value: number): number`, clamped to `0..80`, with exact values `0 -> 1`, `20 -> 0.8`, `80 -> 0.2`.
- Produce `useCommittedRange({ value, min, max, onCommit })` returning draft value and input/pointer/keyboard/blur handlers. While active, external saved values do not replace the draft.

- [ ] Write failing conversion tests for exact endpoints, monotonic decrease, clamp, and non-finite fallback; verify RED against current direct division behavior.
- [ ] Implement the shared pure conversion and apply it to taskbar and float-ball CSS variables.
- [ ] Write failing hook/component tests: ten input frames produce zero persistence calls, pointer release produces one final call, external stale event during drag does not snap the thumb, keyboard/blur commits, and reduced-motion styling remains valid.
- [ ] Implement requestAnimationFrame-local draft preview and boundary commit. Use `onInput` for visual frames; do not persist in the input-frame handler.
- [ ] Rename English copy from Opacity to Transparency while retaining Chinese 透明度 and legacy field keys.
- [ ] Run focused tests, frontend full suite/build, Impeccable detector on changed UI, and `git diff --check`; commit `Fix surface transparency controls`.

### Task 3: Fresh Windows regression proof

**Files:**
- Verification only unless a task-owned defect is exposed.

**Interfaces:**
- Consume Tasks 1–2 and produce native proof, not product code.

- [ ] Fresh-build `pnpm --dir apps/desktop-tauri run tauri:build:debug` after closing only the exact older process.
- [ ] With Windows Computer Use/CUA, prove notification-disabled state is detected on this machine (`CodexBar Enabled=0`), the test action does not falsely report success, and the fixed recovery action opens Windows notification settings without changing it.
- [ ] Prove both sliders track pointer movement smoothly without snapping; capture 0, 20, and 80 states and verify transparency increases visually for taskbar and float ball.
- [ ] Restore original settings, stop the proof binary by exact path, run full local CI, and record paths/hashes/results in the plan ledger.

## Required continuation after this plan

Execute these already-approved plans in order on the same feature branch, each with its own SDD ledger and task reviews:

1. `docs/superpowers/plans/2026-08-23-taskbar-tray-preferences.md`
2. `docs/superpowers/plans/2026-08-23-menu-layout-customization.md`
3. `docs/superpowers/plans/2026-08-23-usage-spend-readonly.md`
4. `docs/superpowers/plans/2026-08-23-settings-integration-polish.md`

No merge, tag, installer, or GitHub Release occurs until all four continuation plans and this repair plan pass a final whole-branch review and Windows acceptance run.
