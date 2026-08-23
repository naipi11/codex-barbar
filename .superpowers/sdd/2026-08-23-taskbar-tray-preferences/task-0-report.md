# Task 0 report — carried repair-plan findings

## Status

PASS for source and automated validation. Fresh-binary CUA/native proof is explicitly deferred to final integration; no implementation was changed solely because live proof was unavailable.

Base: `eabbe0e7b2446a532154d90c32b303d5230db759`

## Implemented behavior

### Windows notification capability

- The system probe now establishes the existing fixed `CodexBar` AppUserModelId registration before reading authoritative `ToastNotifier.Setting`.
- Registration writes only `HKCU:\SOFTWARE\Classes\AppUserModelId\CodexBar` and its existing `DisplayName=codex-barbar` metadata. It does not read or write Windows notification permission values.
- A registration failure returns `unsupported` without invoking the setting probe. Non-Windows detection still returns `unsupported` without registration or probe calls.
- Existing app/global-disabled mappings, toast recheck behavior, settings recovery action, and notification delivery behavior remain unchanged.

### Committed range acknowledgement

- `useCommittedRange` now accepts a `Promise<number>` acknowledgement and treats that saved numeric result as authoritative. Matching prop values no longer acknowledge a save.
- Boundary saves are serialized in submission order, so backend settings events cannot be reordered by concurrent invokes.
- While newer work is queued or in flight, the hook preserves the latest local draft. An older rejection is consumed and suppressed; the newest rejection rolls back to the latest successful Promise acknowledgement and reports one sanitized error.
- Stale prop events are ignored until the authoritative saved value is reflected. After a rejection without a stale echo, later external values are accepted normally.
- `GeneralTab` maps the returned `AppSettingsDto` to the opacity field saved by that card, so the hook receives the persisted number and successful acknowledgement clears the localized error.
- Animation frames remain preview-only; writes occur once per completed pointer, keyboard, or blur interaction.

## TDD evidence

### RED

1. Rust registration-order test failed to compile before production changes with `E0407`: `ensure_registration` was not a member of `NotificationSettingProbe`.
2. Initial frontend race suite failed four tests:
   - matching echo followed by rejection stayed at `30` instead of rolling back to `20`;
   - the two-deferred-commit test invoked both commits immediately instead of one;
   - ABA and older-rejection tests likewise observed two immediate invokes instead of serialization.
3. The `GeneralTab` saved-field test failed with slider value `0` instead of `35` while the callback returned the entire settings object.
4. The post-rejection external-value test failed with `20` instead of accepting `25`, exposing an over-retained stale-prop barrier.

### GREEN

- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml notification_controller::tests -- --nocapture`
  - PASS: 23 passed, 0 failed.
- `pnpm exec vitest run src/hooks/useCommittedRange.test.tsx src/surfaces/settings/tabs/GeneralTab.test.tsx`
  - PASS: 2 files, 16 tests passed, 0 failed, no React warnings.

Coverage includes registration-before-probe ordering and failure short-circuiting; permission-safe fixed metadata; echo-before-Promise settlement; ABA `30→40→30`; two deferred serialized commits; newest rejection rollback; older rejection suppression; rejection consumption; stale prop events; post-rejection external updates; successful localized-error clearing; zero per-frame writes; and one boundary commit.

## Full validation

- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy`
  - PASS: V1 boundary guard.
  - PASS: `cargo fmt --all --check`.
  - PASS: shared Rust Clippy and Tauri Clippy with `-D warnings`.
  - PASS: shared Rust — 317 unit tests passed, 1 explicitly ignored; 17 app-server contract tests passed; 3 icon asset tests passed.
  - PASS: Tauri — 209 tests passed.
  - PASS: frontend — 30 files, 243 tests passed.
  - PASS: TypeScript and Vite production frontend build.
- `pnpm run check-locale`
  - PASS: 32 Rust/TypeScript locale keys match.
- `node C:\Users\stack\.codex\skills\impeccable\scripts\detect.mjs --json apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx apps/desktop-tauri/src/hooks/useCommittedRange.ts`
  - PASS: `[]` (no findings).
- `pnpm --dir apps/desktop-tauri run tauri:build:debug`
  - PASS: fresh debug executable and x64 NSIS bundle produced.
- `git diff --check`
  - PASS.

## Self-review and scope

- Reviewed the complete diff against the Task 0 brief after formatting.
- Changed only the notification controller and focused tests, the committed-range hook and focused tests, GeneralTab's acknowledgement adapter/test, and this report.
- No dependency, permission/capability, settings-key, transparency-mapping, taskbar/menu/tray feature, URI/command surface, notification-permission, or unrelated behavior change was introduced.
- Promise rejection handlers are attached synchronously and all rejection paths are consumed.

## Deferred native proof

The fresh debug build completed, but a live proof launch was intercepted by the app's single-instance guard and handed off to a pre-existing installed CodexBar process. The pre-existing process was not stopped. The user then stopped Computer Use, so no further CUA/UI automation was performed. Per task direction, fresh-binary native proof is deferred to final integration and remains the only validation concern.
