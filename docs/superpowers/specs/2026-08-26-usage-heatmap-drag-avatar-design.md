# Usage Heatmap, Settings Drag, and Avatar Fallback

## Goal

Complete the Usage & Spend surface with a daily cost heatmap, restore native movement for the borderless settings window, and give users a safe local avatar fallback when Codex does not expose an official image.

## Requirements

- Preserve the selected Usage & Spend range for daily/model tables; fetch a separate rolling 365-day activity series for the heatmap.
- Render Sunday-first calendar weeks, purple cost levels, neutral cells for zero/unpriced days, and accessible date/cost/token details.
- Format token values as Chinese `万/亿` or English `K/M/B`, capped at two decimals.
- Make only the settings title area draggable; the close button must remain a normal button.
- Allow PNG avatar selection, preview, save, and restore-default through the existing profile-scoped bridge. Keep the server-side PNG validation and never send cookies, tokens, or remote avatar URLs to React.
- Continue automatic official-avatar resolution only when an approved public URL is returned by Codex; otherwise explain the limitation and use the local upload path.

## Non-goals

- No private ChatGPT profile endpoints, browser-cookie reuse, or guessed avatar URLs.
- No change to pricing provenance or invoice claims.
- No process watcher or new account mutation.

## Acceptance

- Usage & Spend opens with the normal 7-day table and a 365-day purple cost grid.
- Empty/unpriced days remain neutral and expose token counts in their accessible label.
- Settings can be moved by dragging its header and cannot be accidentally dragged by pressing Close.
- A valid PNG updates the selected account's avatar everywhere after the profile event; invalid type/size is rejected before bridge invocation.
