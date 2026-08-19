# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary user: a Windows Codex user who keeps the app running while they work. They glance at remaining weekly quota from the taskbar or a floating ball, then open the tray panel only when they need account, reset, or settings details.

## Product Purpose

codex-barbar is a Windows tray app that shows live Codex / AI-provider usage and remaining limits. Success is glancing at remaining quota in under a second, without opening Codex itself.

## Positioning

It lives in the Windows taskbar notification area and an optional always-on-top float ball, reading the local Codex CLI / account session rather than asking the user to open a website.

## Operating Context

Used on a Windows desktop while coding. Surfaces: tray panel, settings window, compact taskbar capsule, circular float ball. Quota colors are green / yellow / red for high / mid / low remaining. New installs enable the float ball and start-at-login. The taskbar capsule must stay compact and can be transparent. The float ball must be a clean circle with no chrome X and no square window halo.

## Capabilities and Constraints

- Tauri 2 + React tray shell over a Rust backend.
- Account identity shows a short OpenAI / Codex name, not "Current CLI".
- Weekly remaining is the primary Codex quota; do not invent a 5-hour window when the account is weekly-only.
- Taskbar and float-ball visibility and opacity are settings, not overlay chrome.
- Chinese and English UI, default English, switchable in settings.
- Preserve existing commands, settings keys, and provider refresh behavior.

## Brand Commitments

Product name is `codex-barbar`. Users have rejected generic dark-card chrome, square float-ball backgrounds, hover-expand rectangles, and washed-out light-gray type on white.

## Evidence on Hand

Live Windows screenshots from the user showing the tray panel, taskbar capsule, and float ball. No marketing claims or third-party testimonials.

## Product Principles

- Glance first: remaining quota must be readable at a glance.
- Status color is remaining, not decoration: green / yellow / red.
- Compact Windows surfaces beat padded card stacks.
- Settings own visibility; overlays stay chrome-free.
- Keep product truth; replace the current visual world.
