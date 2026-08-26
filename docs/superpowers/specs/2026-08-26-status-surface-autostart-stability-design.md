# Status Surface and Autostart Stability

## Goal

Make live settings changes, Windows shell transitions, and Windows login startup deterministic for the taskbar status capsule and floating ball without changing the existing full-screen preference or exposing new privileged data.

## Scope

### In scope

1. A settings-only event must update opacity/glow presentation without replacing a fresher in-memory usage snapshot with the stale bootstrap snapshot.
2. Taskbar, Start, Explorer, and desktop transitions must preserve visibility intent and position, then reassert the enabled surfaces without requiring a click in Codex or Edge.
3. `start_at_login` must reconcile the HKCU Run entry during startup and when toggled. The registered command is the absolute executable path plus `--background`.
4. `--background` must be parsed as a first-class startup option and must not open a user-facing panel during login launch.
5. Add deterministic unit tests and a fresh Windows/CUA acceptance pass for all three behaviors.

### Out of scope

- No private ChatGPT/Codex endpoints, browser-cookie scraping, or token handling.
- No process watcher that launches codex-barbar only when `Codex.exe` starts. The supported contract for this release is Windows login startup; a Codex-process watcher remains a separate opt-in feature if needed.
- No changes to the user's saved floating-ball coordinates, opacity semantics, full-screen preference, pricing, or notification policy.

## Root-cause evidence

- `useStatusSurface` applies `settings-changed` by replacing the bootstrap shell. `useProfileUsage` keys its cache from the entire bootstrap, so an opacity/glow patch resets a fresher usage event to the older bootstrap state. Missing trust maps to the gray `unknown` band in `statusSurfaceViewModel` and `FloatBall.css`.
- `status_surfaces::reconcile_surfaces` intentionally returns `Ok(())` for `ShellTransient`. `Progman`/`WorkerW` can remain foreground after returning to the desktop, so no later Normal transition restores or reasserts the surfaces.
- The default setting `start_at_login=true` is persisted but does not itself call the Windows registry writer. Registry reconciliation only happens after a settings patch, and `--background` is not currently parsed.

## Design

### 1. Usage continuity across settings changes

Keep the `useProfileUsage` bootstrap identity key limited to usage-bearing fields (`profiles`, `selectedProfileId`, `usageByProfile`). Settings and other presentation-only fields must not invalidate the usage cache. When a settings event arrives, only the settings state and surface presentation are updated; the last accepted `profile-usage-state-changed` snapshot remains authoritative until a newer usage event arrives.

The regression test emits a fresh usage event, then a settings event changing transparency/glow, and asserts that the selected profile, metric, trust, and band remain unchanged while the new presentation settings are applied.

### 2. Shell transition reconciliation

Keep `RealFullscreen` as the only suspension class. For `ShellTransient` and the desktop shell, retain the enabled/disabled intent and saved geometry, call the existing restore/reposition path when the window was displaced, and periodically reassert topmost without activating the surface. The operation must be idempotent and non-fatal when Windows briefly owns the z-order.

The reducer tests cover a long-lived ShellTransient sequence, a transition back to Normal, and a RealFullscreen suspension. CUA proof performs taskbar click, Start, Explorer, and desktop transitions and checks that both surfaces remain visible after a bounded settle window.

### 3. Startup registration

Add a pure `StartupOptions` parser for `--background` and run an idempotent `autostart::reconcile(enabled)` after settings are loaded during app setup. The existing explicit settings toggle continues to write/remove the same HKCU Run value. Startup errors remain visible in diagnostics/logging but do not prevent the tray process from starting.

The startup command remains quoted and absolute, and the parser has no effect on normal foreground launch. Tests cover default-enabled reconciliation, disable/remove, exact command quoting, and background parsing. Windows acceptance reads the HKCU Run value after a fresh launch and starts the built executable with `--background` to verify no panel is opened.

## Compatibility and safety

- No new dependencies.
- No raw identity, cookie, token, or path data crosses the React bridge.
- Existing full-screen hide behavior remains unchanged and wins over shell reconciliation.
- Existing status-surface dimensions, saved position, transparency direction, and color mapping remain unchanged.

## Acceptance criteria

- Changing either float-ball opacity or glow brightness updates the color and intensity immediately without a refresh or gray intermediate state.
- After taskbar, Start, Explorer, or desktop actions, taskbar status and floating ball remain visible/reassert within 2 seconds when enabled and not in real full-screen.
- With `start_at_login` enabled, HKCU Run contains the current executable command after startup; disabling removes it; `--background` is recognized.
- Focused regression tests, both Rust suites, frontend tests/build, boundary/audit checks, production Tauri build, and fresh CUA proof pass.
