# Usage Heatmap, Settings Drag, and Avatar Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the approved Usage & Spend heatmap, borderless settings drag region, and local avatar fallback.

**Architecture:** Keep Rust authoritative for date-range scanning, cost estimates, and avatar validation. Add a separate 365-day activity field to the existing read-only usage DTO; React formats and renders it without changing the selected range tables. Use Tauri's native drag-region attribute for the detached settings window and the existing avatar commands for local PNG persistence.

**Tech Stack:** Rust 2024, Tauri 2, React 18, TypeScript, Vitest, existing AvatarStore.

**Spec:** `docs/superpowers/specs/2026-08-26-usage-heatmap-drag-avatar-design.md`

## Global Constraints

- No private endpoints, cookies, tokens, guessed avatar URLs, or new dependencies.
- Heatmap colors represent estimated daily cost; unpriced/zero days are neutral.
- Daily/model tables retain the selected range; heatmap activity is independently 365 days.
- UI changes require fresh Windows/CUA proof after rebuilding.

---

### Task 1: Usage activity and token formatting

**Files:** `apps/desktop-tauri/src-tauri/src/commands/{bridge,usage_spend}.rs`, `apps/desktop-tauri/src/types/bridge.ts`, `UsageSpendTab.tsx`, related tests/styles.

- [ ] Write failing tests for `activity`, compact token units, 365 cells, and neutral/cost labels.
- [ ] Run the focused Vitest/Rust tests and confirm RED.
- [ ] Add the independent activity scan and map it to the bridge DTO.
- [ ] Add formatter, Sunday-first calendar padding, purple levels, month labels, and accessible cell labels.
- [ ] Run focused tests, full frontend tests/build, Rust usage tests, and fmt.

### Task 2: Settings drag region

**Files:** `apps/desktop-tauri/src/surfaces/Settings.tsx`, `Settings.test.tsx`, `styles.css`.

- [ ] Add a failing DOM assertion for a drag region excluding Close.
- [ ] Add `data-tauri-drag-region` to the title wrapper and preserve the button outside it.
- [ ] Run Settings focused tests and fresh Windows drag proof.

### Task 3: Local avatar fallback

**Files:** `AccountsTab.tsx`, `AccountsTab.test.tsx`, `settingsCopy.ts`, `styles.css`.

- [ ] Add failing tests for valid PNG save/preview/restore and invalid-file rejection.
- [ ] Use existing `saveProfileAvatar`/`clearProfileAvatar` commands, with 1 MiB/MIME/extension checks before invocation.
- [ ] Run account focused tests and fresh Windows upload/restore proof without committing the user's file.

### Task 4: Full verification and handoff

- [ ] Run both Rust suites/clippy, frontend tests/build, boundary/policy checks, and production Tauri build.
- [ ] Capture CUA evidence for heatmap, settings drag, avatar fallback, and existing status surfaces.
- [ ] Review `git diff --check`, scan secrets, exclude `tmp-cua-proof`, commit, and present merge/release options.
