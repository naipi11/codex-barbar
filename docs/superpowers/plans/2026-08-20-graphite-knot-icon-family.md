# Graphite Knot Icon Family Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the boxed numeric tray icon and blue code-glyph bundle icon with one transparent, small-size-safe Graphite Knot identity.

**Architecture:** Keep tray state selection unchanged and replace only the deterministic RGBA renderer with a colorized anti-aliased knot mask. Add an editable SVG source for the application identity and derive the existing PNG/ICO bundle paths from it, with asset-contract tests that inspect headers and ICO frames without new dependencies.

**Tech Stack:** Rust 2024, Tauri 2, SVG, PNG, ICO, Pillow from the bundled workspace runtime for deterministic asset generation, Windows/CUA for native proof.

**Spec:** `docs/superpowers/specs/2026-08-20-graphite-knot-icon-family-design.md`

## Global Constraints

- Do not add dependencies or change Tauri permissions/configuration.
- Do not modify quota selection, thresholds, tooltip behavior, settings, taskbar status, or float-ball behavior.
- Preserve all unrelated dirty and untracked user files.
- Do not commit, push, tag, or release in this task without separate authorization.
- Application and tray icons must contain no account name, email, or other private data.

---

### Task 1: Transparent tray knot renderer

**Files:**
- Modify: `rust/src/tray/render.rs`
- Test: `rust/src/tray/render.rs`

**Interfaces:**
- Consumes: existing `TrayVisualState`, `TrayLevel`, and `render_tray_icon_rgba(state) -> (Vec<u8>, u32, u32)`.
- Produces: the same public function and compatibility wrappers, now rendering one transparent knot silhouette tinted by state.

- [ ] **Step 1: Write failing tests for the approved visual contract**

Add tests that assert transparent corners, equal alpha silhouettes for normal/warning/danger/stale states, center negative space, approved palette pixels, neutral API/unavailable rendering, and absence of the old opaque background rectangle.

- [ ] **Step 2: Run the focused renderer test and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml tray::render::tests -- --nocapture
```

Expected: the new transparency/silhouette assertions fail against the boxed numeric renderer.

- [ ] **Step 3: Implement the minimal renderer**

Replace glyph and bar drawing with a readable 32 × 32 anti-aliased knot alpha mask. Add a one-pixel graphite keyline outside the mask, tint the inner alpha by the four approved colors, and leave all other pixels transparent. Keep all exported APIs and dimensions unchanged.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same focused command and confirm every renderer test passes with no warning.

### Task 2: Canonical application-icon source and bundle assets

**Files:**
- Create: `rust/icons/codex-barbar.svg`
- Create: `rust/icons/tray-knot.svg`
- Modify: `rust/icons/codex-barbar.png`
- Modify: `rust/icons/codex-barbar.ico`
- Create: `rust/tests/icon_assets.rs`

**Interfaces:**
- Consumes: the existing ChatGPT-style path geometry in `apps/desktop-tauri/src/theme/ChatGptMark.tsx` and the fixed bundle paths in `tauri.conf.json`.
- Produces: a canonical 1024 px Graphite Knot SVG, 1024 px PNG, and ICO frames at 16/20/24/32/48/64/128/256 px.

- [ ] **Step 1: Write failing binary asset-contract tests**

Add a Rust integration test that asserts `codex-barbar.svg` exists and contains the approved graphite/emerald palette and knot path, the PNG header reports 1024 × 1024, and the ICO directory contains exactly the required frame dimensions.

- [ ] **Step 2: Run the focused asset test and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml --test icon_assets -- --nocapture
```

Expected: failure because the SVG does not exist and the legacy ICO does not satisfy the new frame contract.

- [ ] **Step 3: Create the SVG and derive PNG/ICO assets**

Create the SVG with a graphite rounded square, restrained emerald halo, and centered white knot. Render it at high resolution with the installed Windows browser, then use bundled Pillow only for deterministic resizing and ICO packaging. Do not add a project dependency.

- [ ] **Step 4: Inspect rendered assets**

Open the 1024 px PNG and a contact sheet of the 16/20/24/32/48/64/128/256 frames. Verify the knot remains centered, recognizable, and unclipped.

- [ ] **Step 5: Run the asset test and verify GREEN**

Run the focused asset command and confirm the header/frame assertions pass.

### Task 3: Regression verification and native Windows proof

**Files:**
- Verify only; do not broaden scope unless a failing test exposes a task-owned defect.

**Interfaces:**
- Consumes: Tasks 1 and 2 output.
- Produces: fresh automated and Windows-native evidence for the final handoff.

- [ ] **Step 1: Run focused and full automated verification**

Run:

```powershell
cargo fmt --all -- --check
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
```

- [ ] **Step 2: Request independent code review**

Ask a read-only reviewer to inspect the scoped diff against the spec, with special attention to alpha edges, palette state mapping, ICO frame completeness, and unrelated dirty-file preservation. Resolve Critical and Important findings before continuing.

- [ ] **Step 3: Fresh-build the desktop application**

Close only the running `codex-barbar.exe` instance after resolving its exact path, then run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Launch the newly built executable and record its path/hash so a stale single-instance process cannot invalidate proof.

- [ ] **Step 4: Validate Windows observables**

Use CUA Driver or the documented Windows screenshot fallback to capture:

- the new application icon at native Windows size;
- the tray knot on a light taskbar background;
- the tray knot on a dark taskbar/background where available;
- at least green, amber, red, and neutral states through deterministic proof data or focused renderer contact sheets.

Confirm there is no opaque square, pixel number, clipping, or unreadable center fill.

- [ ] **Step 5: Final scope and privacy audit**

Run `git status --short`, inspect the exact diff, verify pre-existing dirty/untracked files are untouched, and scan new icon/evidence assets for private account data. Report verified results and any limitation without committing or pushing.
