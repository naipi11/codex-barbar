# codex-barbar v1.1 Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver v1.1.0 identity-aware status surfaces, dynamic local cost estimates, and reliable Windows surface lifecycle behavior without expanding codex-barbar beyond its read-only contract.

**Architecture:** Execute four independently testable implementation plans in a controlled sequence. Settings/identity establishes the typed DTO and migration foundation; pricing consumes that foundation; lifecycle/motion stays isolated from pricing; final integration proves the combined app on real Windows before a release is authorized.

**Tech Stack:** Rust 2024, Tauri 2, React 18, TypeScript, Vitest, SQLite-backed settings, existing `reqwest`, raw Win32 FFI, CSS/WebView2, CUA Driver.

**Spec:** `docs/superpowers/specs/2026-08-24-v1-1-identity-pricing-surfaces-design.md`

## Plan Index

## Global Constraints

- Keep codex-barbar read-only: no reset redemption, purchase, plan/account mutation, model request, script, arbitrary URL, or arbitrary executable command.
- Do not add dependencies, Tauri permissions, or external tools without explicit approval.
- Do not read browser cookies or scrape browser pages for profile identity or avatar data.
- Full email is visible only in the tray-panel account selector.
- Numeric model pricing is not embedded in source; unknown/ambiguous names remain unpriced.
- User-facing visual percentages are `0..100`: transparency is 0 most opaque to 100 most transparent; glow is 0 darkest to 100 brightest.
- Preserve stable settings tab IDs: `general`, `providers`, `notifications`, `menuBar`, `menu`, `usageSpend`, `advanced`, `about`.
- Preserve the hidden taskbar measurement window contract and shared visible/measurement presentation derivation.
- Every new string ships in English and Simplified Chinese.
- Native/UI work requires a fresh Windows build and CUA proof; unit tests alone are insufficient.
- Never publish, tag, push, install, uninstall, or change a user's Windows setting without explicit current authorization.

## Execution Order

| Order | Plan | Produces | Depends on |
| --- | --- | --- | --- |
| 1 | `2026-08-24-v1-1-identity-surfaces-settings.md` | v2 settings migration, shared range control, presentation identity, avatar protocol, fixed tray, panel preferences | approved v1.1 spec |
| 2 | `2026-08-24-v1-1-pricing-catalog.md` | dynamic catalog/cache/FX, safe aliases, partial local estimates, usage UI | plan 1 settings DTOs and notifications settings |
| 3 | `2026-08-24-v1-1-windows-lifecycle-motion.md` | trace-proven Shell state repair and event-driven smooth motion | plan 1 surface settings and shared status view model |
| 4 | `2026-08-24-v1-1-integration-release.md` | localization, upgrade matrix, Windows proof ledger, release-ready artifact evidence | plans 1–3 |

## Cross-plan Interfaces

```text
PresentationIdentityDto
  -> TrayHeader, TaskbarStatusContents, taskbar measurement route

SurfaceAppearancePreferences
  -> CommittedRangeField, taskbarStatusPresentation, FloatBall CSS variables

PanelPreferences
  -> TrayPanel quick-action rendering

PricingCatalogSnapshot + CostEstimateDto
  -> UsageSpendDto -> UsageSpendTab

SurfaceLifecyclePhase + FloatBallMotionDto
  -> status_surfaces controller / FloatBall renderer
```

## Required Start-of-plan Procedure

- [ ] Create an isolated implementation worktree from the branch containing
  this index, the approved spec, and all four plans. Use a `codex/` branch.
- [ ] Record `git status --short --branch`, `git rev-parse HEAD`, Node version,
  Rust version, and the installed Codex version without printing secrets.
- [ ] Run `pnpm --dir apps/desktop-tauri test`, both Rust test manifests, and
  `scripts/local-check.ps1`; stop and report baseline failures before making
  product changes.
- [ ] Execute plans 1 through 4 in order. Every task has its own focused red,
  green, and commit evidence. Do not batch unrelated tasks into one commit.

### Task 1: Coordinate the independently testable plans

**Files:**
- Read: every plan named in Execution Order.

**Interfaces:**
- Consume the approved spec and the four execution plans.
- Produce one sequential execution ledger with plan name, task number, commit,
  focused test result, and Windows-proof result.

- [ ] Create the ledger from the required start-of-plan procedure before Task 1
  of the identity/surface plan.
- [ ] Run the identity/surface plan through its Windows proof before starting
  pricing or lifecycle integration work.
- [ ] Run pricing and lifecycle plans only after their typed settings/view-model
  dependencies exist on the implementation branch.
- [ ] Execute the integration/release plan only when each prior plan reports
  all task-owned tests green and records unperformed native checks explicitly.

## Required End-of-plan Procedure

- [ ] Read the acceptance criteria in the v1.1 spec line-by-line and record
  the command, test, or Windows proof that demonstrates each criterion.
- [ ] Run `git diff origin/main...HEAD --check`, full local checks, a fresh
  Windows debug build, and the CUA matrix in the integration plan.
- [ ] Request user authorization before any remote push, tag, GitHub Release,
  installer replacement, or Winget action.
