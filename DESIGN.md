# Design

<!-- impeccable:design-schema 1 -->

## World

Night instrument cluster. Remaining quota is a fuel-gauge readout, not a stack of dark glass cards.

Seed: `0e15fe93` / assigned direction 5.

## Palette

- Ink cockpit: `#10131a`
- Instrument metal: `#171c26`
- Phosphor remaining: `#56d98a` / `#2f9e5b`
- Amber caution: `#e2a33a` / `#c48412`
- Alarm red: `#e24b55` / `#c5303a`
- Readout text: `#e8edf6`
- Quiet labels: `#8b95a7`

Bands stay remaining-based: high >= 67, medium >= 34, low below 34.

## Type

- UI: IBM Plex Sans
- Numbers: IBM Plex Mono
- Fallback: Segoe UI / system-ui

## Surfaces

- Tray: cockpit panel, large remaining number, one colored remaining bar, no purple glass glow.
- Taskbar: compact capsule, darker green/yellow/red numerals, same identity + weekly remaining + reset date.
- Float ball: clean circle, dark remaining digits, no hover rectangle, no chrome X.
- Settings: same instrument metal, accent ticks on status cards, no extra chrome.

## Motion

Keep existing 150-250ms state motion. No page-load choreography.
