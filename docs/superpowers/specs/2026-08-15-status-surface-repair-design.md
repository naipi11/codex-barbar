# Status Surface Repair Design

Date: 2026-08-15

Status: Approved design, pending written-spec review

## Context

The independent taskbar measurement architecture is accepted. A fresh Windows
proof rendered a separate visible 197x40 logical taskbar surface with
`ProofU | 周 98% | 8/20 | ×`, no `5H`, and a hidden 318x40 measurement WebView.
The release gate remains stopped because the post-weekly proof reported
unchanged taskbar opacity captures, a contaminated close result, and
inconclusive float-ball native visibility.

Fresh diagnosis established the following facts:

- `settings-changed` updates the taskbar host's inline
  `--surface-bg-alpha` value, but `.taskbar-status` redeclares the variable as
  `0.2` and shadows the inherited runtime value. The computed background
  therefore remains at 20% when the setting changes to 0% or 80%.
- With no SQLite writer lock, the visible taskbar close button completes the
  typed disable transaction: the visible and measurement WebViews disappear
  and `taskbarStatusEnabled` persists as `false`.
- The previous native close failure was observed in the same sequence as an
  intentionally held SQLite write lock. Normal close success and persistence
  failure rollback must be verified as separate scenarios.
- The float-ball WebView loads complete DOM and a same-state runtime enable
  transition succeeds. Native visibility still requires a clean, isolated
  proof because the prior run became polluted by peer-surface state and a
  timed-out hidden-window capture.

## Goals

- Make taskbar opacity changes apply immediately to the actual rendered root.
- Make status-surface proof scenarios deterministic, mutually exclusive, and
  independent of persistent user enable flags.
- Separate successful close verification from deliberate persistence-failure
  rollback verification.
- Produce reliable Windows proof for taskbar and float-ball native behavior.
- Preserve the release STOP boundary until every source and Windows gate passes.

## Non-Goals

- Do not replace per-surface background opacity with whole-window native alpha.
- Do not add a product diagnostics command, capability, dependency, or
  externally callable proof API.
- Do not redesign the taskbar or float-ball visuals.
- Do not change quota selection, identity formatting, measurement geometry,
  the 2-second reconciliation interval, or provider behavior.
- Do not release, install, or push while any repair gate is failing or
  inconclusive.

## Architecture

### 1. Runtime opacity ownership

`TaskbarStatus` remains the owner of the presentation produced by
`buildTaskbarStatusPresentation`. The runtime `surfaceAlpha` value must be
applied to the element that renders `.taskbar-status`, not to a
`display: contents` ancestor whose custom property can be shadowed.

The CSS authored default remains only a fallback for markup without a runtime
value. It must not override a React-provided 0, 0.2, or 0.8 value. The visible
and measurement routes continue to render through `TaskbarStatusContents` and
receive the same alpha value. Changing opacity must not recreate a native
window, call `set_taskbar_status_width`, or alter measured content geometry.

The float ball already applies its inline variable on `.float-ball-shell`, the
same element that declares the CSS fallback. Its ownership remains unchanged.

### 2. Runtime-only proof surface settings

Status proof scenarios use a small deterministic runtime projection:

```text
taskbar-status:* -> taskbar enabled, float ball disabled
float-ball:*     -> float ball enabled, taskbar disabled
```

This projection is applied only to `StatusSurfaceState`. It never calls
`SettingsRepository::update` and never changes the user's stored enable flags
or opacity values. The synthetic bootstrap payload returns matching enable
flags and deterministic opacity values so the frontend cannot disagree with
the native proof runtime.

Settings proof scenarios keep their existing role: they exercise real settings
commands and events. Live opacity proof must use a clean normal/settings run,
not a status proof bootstrap whose purpose is deterministic synthetic content.

Proof activation is successful only after the target runtime transition
succeeds. A peer cleanup error or target show error is recorded as proof
activation failure; the run stops before screenshots or release gates.

### 3. Close scenarios

Close proof is split into two independent scenarios:

1. **Normal close:** no external database writer exists. Clicking the frontend
   `×` must destroy the visible taskbar and measurement helper and persist
   `taskbarStatusEnabled=false`.
2. **Persistence failure and retry:** an external diagnostic writer holds the
   documented SQLite lock. Clicking `×` must leave the prior enabled state and
   both runtime windows intact, while the fixed-size close button enters its red
   retry state. After releasing the lock, a second click must destroy both
   windows and persist `false`.

The native `WM_CLOSE` route is tested without a database lock. It must route to
the same typed controller and converge to the same disabled state. A locked
rollback that deliberately preserves `true` is expected behavior, not a normal
close failure.

### 4. Proof observability

Use each tool only for the layer it can prove:

- WebView2 CDP reads the actual DOM, inline custom property, computed style,
  button error state, live-region text, and command result.
- CUA drives targetable Settings interactions and validates keyboard behavior.
- DPI-aware Win32 inspection proves auxiliary HWND existence, native
  visibility, geometry, focus/z-order behavior, and cleanup.
- Screen-composited captures prove actual opacity against the desktop. A
  `PrintWindow` result alone cannot prove layered-window composition.
- CDP content capture may prove hidden WebView content, but cannot substitute
  for native `IsWindowVisible=true` or screen-composited proof.

No product command or permission is added for proof convenience.

## Data Flow

### Opacity

```text
Settings slider
  -> update_settings
  -> SettingsRepository save
  -> settings-changed event
  -> useStatusSurface bootstrap.settings update
  -> buildTaskbarStatusPresentation.surfaceAlpha
  -> rendered .taskbar-status custom property
  -> WebView2 repaint
```

Acceptance requires computed background alpha to change, not merely the React
inline value. At 0%, the panel background is transparent while text and quota
colors remain readable. At 80%, the dark panel background is visibly more
opaque. Both updates occur without a window rebuild or width command.

### Proof activation

```text
CODEXBAR_PROOF_MODE
  -> fixed ProofScenario
  -> runtime-only target/peer projection
  -> StatusSurfaceState transition
  -> matching synthetic bootstrap settings
  -> native visibility and content proof
```

The persistent repository is read only for normal startup needs. Status proof
activation does not write status-surface flags. Process exit therefore restores
the pre-proof runtime without a compensating settings write.

## Error Handling

- If settings persistence fails, `update_settings` returns the existing stable
  code and does not emit a false authoritative settings event.
- If `settings-changed` emission fails after persistence, log a stable,
  non-sensitive diagnostic. A later window bootstrap still reads the saved
  value.
- If proof peer cleanup or target creation/show fails, record proof activation
  failure and stop the scenario. Do not fall back to the user's persisted peer
  surface.
- Taskbar disable keeps measurement-first cleanup. Only a destroyed
  measurement helper permits visible cleanup.
- Persistence failure restores the previous runtime enabled state. If rollback
  also fails, preserve the existing stable rollback error behavior.
- A float-ball show failure retains enabled state so the 2-second monitor can
  retry, but proof does not pass until native visibility is independently
  observed.
- Diagnostic teardown restores the four surface settings, releases any SQLite
  lock, stops only the app under test, and leaves no debug process or selected
  misleading screenshot.

## Test Strategy

### Frontend

- Assert the runtime alpha is attached to the actual `.taskbar-status` element.
- Cover opacity values 0, 20, and 80.
- Assert a settings event changes the rendered root value without invoking
  `set_taskbar_status_width`.
- Add a structural CSS regression that prevents a child-authored default from
  overriding the runtime variable.
- Preserve visible/measurement geometry equality, close failure/retry,
  accessibility, and reduced-motion tests.

### Rust and Tauri

- Test the exact taskbar/float proof projection and mutual exclusion.
- Test that status proof activation uses runtime ports without repository
  writes.
- Test matching synthetic bootstrap enable flags and deterministic opacity.
- Keep normal close success and persistence-failure rollback as distinct tests.
- Preserve measurement-first cleanup, deferred helper recovery, width caller
  authorization, and the 2-second monitor contract.

### Windows acceptance

- Full Rust, Tauri, frontend, formatting, clippy, boundary, and production
  frontend build gates pass.
- Fresh taskbar weekly proof shows the complete row, no `5H`, 104..317 logical
  width, exact 40 logical height, hidden 318x40 measurement helper, and safe
  taskbar placement.
- Screen-composited 0% and 80% taskbar captures have a measurable pixel/hash
  difference while content and geometry remain stable.
- Normal frontend close and unlocked native `WM_CLOSE` both destroy visible and
  measurement windows and persist `false`.
- Locked persistence failure shows the red retry state, preserves the prior
  enabled state, and succeeds after lock release.
- Float weekly proof shows a natively visible collapsed window; expanded,
  click, drag, close, 0/80 opacity, and dark-theme isolation all pass.
- Proof runs leave the persistent surface settings byte-for-byte equivalent in
  the four surface fields to their pre-proof values.

Any failed or inconclusive Windows acceptance item keeps release, installation,
and push blocked.

## Security and Privacy

- Proof payloads remain synthetic and credential-free.
- No screenshot may contain a real account, token, private path, database
  content, or secret.
- Logs and errors use stable codes and omit raw storage, Tauri, WebView2, and
  filesystem error text.
- Only the exact app-under-test process may be stopped during validation.

## Delivery Boundary

This repair phase ends when source gates and all Windows acceptance checks pass
and an independent review approves the evidence. Only then may the existing
Task 5 live-account, release, installation, and final-review plan resume.
