# codex-barbar v1.1 Identity, Pricing, and Surface Design

## Status

The project owner approved the four design sections in chat on 2026-08-24.
This document is awaiting the owner's final written-spec review before an
implementation plan or production code is started.

## Objective

Deliver `v1.1.0` as a coherent Windows desktop release that makes
codex-barbar feel like a trustworthy, compact status companion rather than a
collection of independent overlays:

1. identify the signed-in Codex user by username and avatar without exposing
   their email outside the tray-panel account selector;
2. make the taskbar capsule, floating ball, fixed tray, and configurable panel
   behave as one product surface;
3. replace static model prices with daily refreshed, source-labelled local
   estimates for official and clearly mapped third-party model names; and
4. diagnose and repair the Windows Shell lifecycle and Fast-motion defects
   before a release is cut.

The product remains read-only. It must never redeem a reset credit, buy
usage, modify a Codex/OpenAI account, submit a model request, or change
Windows notification permissions.

## Product and Visual Direction

The established visual world remains **Night instrument cluster**:

- taskbar capsule and tray panel are compact quota readouts, not generic card
  stacks;
- weekly remaining uses one shared green / amber / red band definition:
  `>=67`, `34..=66`, and `<=33` remaining percent;
- the floating ball remains the animated product mark, not a profile photo;
- identity is useful context, not a wall of account metadata;
- all controls must work in English and Simplified Chinese, with visible
  focus, keyboard operation, and reduced-motion handling.

The visual signature is the relationship between the user avatar in the
compact taskbar/panel identity strip and the independent, animated product
mark in the floating ball. The former says whose quota is shown; the latter
says how that quota is feeling.

## Approved Boundaries

### Privacy and account identity

- Read identity only from the official local Codex app-server interface that
  codex-barbar already uses. Do not scrape a browser page or read browser
  cookies.
- A full email address is permitted only in the tray-panel account selector.
  It is never used as the panel title, taskbar label, tooltip identity, or
  floating-ball label.
- A manual avatar is an optional local override, scoped to one profile. It is
  not uploaded, synchronized, or sent to a pricing provider.
- Do not forward avatar source URLs, raw local file paths, cookies, tokens,
  API keys, or raw app-server payloads to React, diagnostics, logs, or toast
  text.

### Read-only costs

- Costs are local estimates, not a provider invoice or a promise of a
  third-party gateway's price.
- Numeric prices are never hard-coded in the executable. A model with no
  successfully validated price remains unpriced; it never receives a guessed
  value.
- A gateway model may use an official-equivalent estimate only when a
  deterministic, versioned alias maps it to exactly one canonical model.
- No API key is required for catalog or exchange-rate refreshes.

### Native surface safety

- The native tray is a fixed product surface. Users do not configure tray icon
  mode, tooltip fields, or native-menu order.
- The status surfaces may hide only for a verified real full-screen window and
  only if the user has enabled the existing full-screen preference. Opening
  Start, Explorer, the desktop, or a taskbar flyout must not clear a user's
  visibility intent or saved floating-ball position.
- Existing taskbar measurement contracts remain intact: the visible and hidden
  measurement routes share the same presentation calculation, while only the
  measurement route changes taskbar width.

## Architecture

```text
local Codex app-server                 official public price pages
        |                                        |
        v                                        v
Identity resolver                         Pricing source adapters
        |                                        |
        +--> protected local avatar store        +--> validated catalog cache
        |                                        |          + FX cache
        v                                        v
  PresentationIdentity ----------------> shared view models
        |                                        |
        +-------------+-------------+------------+
                      |             |
             tray panel/taskbar    fixed tray     Usage & Spend
                      |
                 floating ball motion monitor
                      |
             Windows surface lifecycle controller
```

Rust remains the source of truth for identity, avatar eligibility, native
surface state, pricing, mapping, and persistence. React renders typed DTOs,
keeps local drafts while a user drags a slider, and never invents a price,
identity, or visibility state.

## Identity and Avatar Design

### Canonical presentation identity

Extend the protected account identity cache with a presentation-only identity
record. Its public DTO contains only:

```text
displayName: string
accountStatus: signedIn | signedOut | unavailable
planType: string | null
avatar: default | official | manual
avatarAssetUri: generated-local-uri | null
avatarRevision: opaque revision | null
```

The visible name uses this deterministic precedence:

1. a locally supplied official `handle` / `username` / `userName` field;
2. locally supplied `displayName` / `name`;
3. the email local part, without `@` or the domain;
4. a localized signed-in, signed-out, or unavailable fallback.

This prevents `Current CLI` and full email addresses from leaking into status
surfaces. Compact taskbar text remains exactly the first six Unicode grapheme
clusters of this display name when the user enables the account-name element.

### Avatar sources

1. **Default:** the product mark whenever no signed-in account is known or an
   image cannot safely be used.
2. **Official local metadata:** parse optional avatar metadata only from the
   existing official local `account/read` response. The backend may retrieve
   an HTTPS asset only after URL, redirect, MIME type, byte length, pixel
   dimensions, and private-network destination validation. Unknown hosts,
   non-image payloads, redirects, or failures fall back to Default.
3. **Manual override:** the user selects an image through the settings UI. The
   frontend normalizes it to a bounded square PNG before invoking a typed save
   command. Rust writes it to the profile's application-data avatar directory
   using an atomic replace. A user can remove the override and return to the
   official/default source.

React receives only an opaque URI served by a constrained Tauri asset protocol
whose profile ID is validated by Rust. It never gets the original URL or file
system path. Avatar files are bounded, local, and non-secret; identity metadata
remains in the existing protected cache.

### Surface use

- The tray-panel header and taskbar capsule render the same avatar and display
  name immediately after a successful account refresh or profile switch.
- The native tray stays the product mark tinted by the universal weekly quota
  band; it does not become a personal avatar.
- The floating ball stays the animated product mark and remains free of user
  name, email, quota digits, and close chrome.
- The only full-email rendering is the tray-panel account selector. The
  settings Accounts page identifies a profile by the presentation name and
  never repeats the full email address.

## Settings Information Architecture

The case-sensitive tab IDs remain stable for deep links and backend
validation. Only the user-facing titles and ownership change.

| Stable ID | English title | Chinese title | Responsibility |
| --- | --- | --- | --- |
| `general` | General | 通用 | Startup, refresh cadence, theme, language, visual defaults |
| `providers` | Accounts | 账户 | Existing account management and optional manual avatar override |
| `notifications` | Notifications | 通知 | Existing opt-in notification behavior plus pricing-source events |
| `menuBar` | Taskbar & Float Ball | 任务栏与悬浮球 | Overlay visibility, fields, density, transparency, glow, full-screen behavior |
| `menu` | Panel | 面板 | Tray-panel density, details, and shortcut layout |
| `usageSpend` | Usage & Spend | 用量与费用 | Read-only allowance, reset credits, local tokens, and cost estimates |
| `advanced` | Advanced | 高级 | Existing executable and diagnostics controls |
| `about` | About | 关于 | Installed version, update state, license, and links |

The settings sidebar is a single, fixed navigation list. Only its content area
scrolls, eliminating repeated/duplicated entries at narrow or tall window
sizes.

About obtains the installed version from the live backend bootstrap/package
metadata. It must never contain a manually localized or hard-coded version
string.

### Shared range controls

All percentage and scalar range fields use one `CommittedRangeField` component
over the existing draft/commit hook:

- input frames update only the local draft and live preview;
- persistence occurs on pointer release, keyboard commit, or blur;
- a delayed settings event cannot overwrite an active drag;
- a failed save returns the thumb to the last acknowledged value and shows a
  localized inline error;
- keyboard, pointer cancellation, focus, ticks, output text, and reduced
  motion behave identically on every range field.

User-facing percentages always span `0..100`, never `0..80`.

| Control | `0%` | `100%` |
| --- | --- | --- |
| Taskbar transparency | maximally opaque | maximally transparent |
| Float-ball transparency | maximally opaque | maximally transparent |
| Float-ball glow | darkest | brightest |

Legacy stored `0..80` transparency/glow values are migrated on load with
`round(old * 100 / 80)`, then written in the new canonical range after the
next successful settings save. The old `*Opacity` JSON/bridge fields are read
only migration aliases; new typed settings/DTO fields use
`*TransparencyPercent` so future code cannot reverse the semantics again.

### Taskbar and floating ball

`Taskbar & Float Ball` provides:

- enable/disable each surface;
- taskbar fields: avatar/product mark, account name, weekly label, weekly
  percent, and reset date; at least one meaningful item remains visible when
  the taskbar is enabled;
- compact or standard taskbar density;
- taskbar transparency;
- floating-ball transparency and glow brightness;
- existing full-screen hiding preference, applied to both status surfaces.

The user can still drag the floating ball to any screen coordinate, including
the taskbar region. The saved coordinate is never silently replaced by a
work-area-clamped coordinate just because the Windows Shell opens.

### Fixed native tray

The tray becomes fixed product behavior:

- quota-band product icon;
- tooltip rows: product/username, universal weekly remaining, reset date, and
  last successful update;
- fixed right-click order: Open panel, Refresh, Accounts, Usage & Spend,
  Settings, Quit.

Legacy tray icon, tooltip, and native-menu settings deserialize safely but no
longer affect the native tray. They are ignored after migration rather than
deleted, so older settings files remain loadable.

### Panel personalization

The former Menu tab becomes Panel. It permits:

- compact or standard panel density;
- show/hide reset time, data freshness, account status, and other non-secret
  helper lines;
- show/hide and reorder eligible quick actions;
- restore the documented panel layout.

Refresh is always visible and first-class. It cannot be hidden or reordered
out of reach. Eligible actions include Usage & Spend, Settings, Hide panel,
and Quit; no scripts, URLs, executable paths, purchase actions, or Codex
account mutations can be added.

## Dynamic Pricing and Cost Estimates

### Catalog lifecycle

Create a focused Rust `PricingCatalog` subsystem with these separable units:

```text
PricingSourceAdapter  fetches one source and returns validated source records
PricingNormalizer     canonicalizes model names and price dimensions
ModelAliasResolver    maps only explicit, unambiguous gateway aliases
PricingCatalogStore   atomically persists last successful snapshots
FxRateStore           atomically persists daily USD/CNY source data
PricingRefreshService schedules refresh, dedupe, and notifications
```

The executable contains source locations, parser schemas, validation rules,
and alias rules, but no numeric model prices. Each accepted source record
stores model ID, input/cache/output dimensions, context-tier conditions,
currency, source URL, observed time, parser version, and trust state.

Official public sources are attempted first for OpenAI, DeepSeek, xAI,
Moonshot/Kimi, and Alibaba/Qwen. Structured official data is preferred; an
official page parser is permitted only if it validates the expected currency,
units, non-negative rates, and model identifiers. A trusted catalog such as
models.dev is a supplementary fallback and cannot overwrite a valid official
record.

Each source has a bounded timeout and validation boundary. Failed or malformed
responses preserve the most recent successful cache rather than replacing it
with zero, an empty catalog, or an invented default.

### Source registry

The first registry contains these public source families, each with a dedicated
adapter and sanitized parser fixture:

| Family | Official source | Intended data |
| --- | --- | --- |
| OpenAI | `platform.openai.com/pricing` | input, cached input, output, context tiers, USD |
| DeepSeek | `api-docs.deepseek.com/quick_start/pricing` | cache-hit/cache-miss input, output, source currency |
| xAI | `docs.x.ai/developers/pricing` | model prices and any documented cost dimensions |
| Moonshot/Kimi | `platform.kimi.com/docs/pricing/*` | model prices, cache dimensions, source currency |
| Alibaba/Qwen | Model Studio official model-pricing page | model, regional/tiered rates, source currency |
| USD/CNY | PBOC/CFETS official central-rate publication | date and USD/CNY conversion rate |

The registry's source URLs and parser revisions are versioned configuration,
not numeric pricing. A source may be temporarily unavailable without disabling
other source families. New providers or source formats require an explicit
adapter and fixture; an arbitrary page is never scraped just because it names
a model.

### Canonicalization and estimates

For every scanned model, the resolver returns one of:

```text
ExactObservedCost       provider response supplied a usable exact cost
OfficialDirect          model name directly matches an official catalog record
OfficialEquivalent      explicit third-party alias maps to one official record
Unpriced                no safe unique match exists
```

Examples of the approved explicit mapping family include:

```text
4sapi-gpt/gpt-5.6-sol     -> gpt-5.6-sol
4sapi-gpt/gpt-5.6-terra   -> gpt-5.6-terra
4sapi-gpt/gpt-5.6-luna    -> gpt-5.6-luna
```

Provider prefixes such as `openai/`, `deepseek/`, `xai/`, `kimi/`, and `qwen/`
are normalized only when they lead to one canonical model. Similar names,
undocumented variants, and ambiguous aliases remain Unpriced. A third-party
equivalent estimate is explicitly labelled as such because its gateway bill
may differ from the official API rate.

The calculator uses input, cached-input, output, and documented context-tier
rules from the catalog. Where a supported provider exposes an exact response
cost in a local record, that exact observed amount outranks a catalog estimate.

### Currency and presentation

- Default display currency is USD; users can choose USD or CNY.
- Each price keeps its original source currency. Conversion uses the latest
  successfully cached official USD/CNY central rate and displays the rate
  date/source.
- If a conversion cannot be made, the UI shows the native price or `—`; it
  never silently converts with an invented exchange rate.
- The model-table column is always `Cost / 费用`, not `Cost unavailable / 费用不可用`.
- A priced row displays exact, estimate, or official-equivalent status.
- An unpriced row shows `—` while retaining token totals.
- A mixed report shows a **partial estimate** total plus an unpriced-model
  count instead of suppressing every known cost.
- The existing persistent disclosure remains: **Local estimate, not a
  provider bill / 本地估算，并非供应商账单**.

### Daily refresh and notifications

- At startup, refresh in the background only when the last successful catalog
  update is older than 24 hours.
- While running, refresh once per 24-hour interval; concurrent refreshes
  coalesce.
- Offline or source failures retain the last successful cache and a stale
  source label.
- With the existing notification master switch enabled, pricing events are
  independently opt-in: notify only when a validated model price changes or
  when three consecutive catalog refresh cycles fail. One recovery can be
  reported; repeated identical failures are deduped.
- No catalog response, exchange-rate response, or model log is uploaded.

## Windows Surface Lifecycle Repair

### Mandatory root-cause gate

The Start-menu disappearance bug has had repeated symptom-level attempts.
Before changing native behavior, implement a sanitized, in-memory diagnostic
trace and reproduce the following matrix on a fresh desktop build:

1. floating ball on the taskbar, then open/close Start;
2. floating ball off the taskbar, then open/close Start;
3. click blank taskbar, open Explorer, minimize Edge, and press `Win+D`;
4. enter/exit video full-screen and browser full-screen.

For taskbar and floating-ball windows independently, the trace records only
visibility intent, native visible/minimized state, position, topmost result,
surface enabled state, detected foreground class, and suspension reason. It
does not record window titles, user content, account data, or browser URLs.

No lifecycle implementation proceeds until the trace establishes which Win32
event or failure path changes actual state. The eventual test fixture is built
from that trace, not from an assumed Start-menu class name.

### Desired state machine

The controller separates a user's desired visibility from transient native
window state:

```text
Enabled + Normal           -> visible and reassertable
Enabled + ShellTransient   -> intent and geometry retained; never fullscreen-hidden
Enabled + RealFullscreen   -> hidden only when full-screen hiding is enabled
Disabled                   -> intentionally destroyed/hidden
```

`ShellTransient` covers Windows shell interactions such as Start, search,
desktop, taskbar, and Explorer transitions. It must never set the
full-screen-suspended flag. If Windows temporarily places a system flyout above
the status surface, the app retains the intent and automatically restores the
surface without requiring a click in Codex or Edge. On transition back to
Normal, it explicitly restores visibility, z-order, and the user-saved
coordinate once; periodic reconciliation remains only a bounded fallback.

`RealFullscreen` continues to hide both status surfaces only when the existing
preference is on. Full-screen detection and Shell classification are separate
results so a shell transient cannot be misclassified as a video or game.

Native `SetWindowPos`/DWM failures are retryable state observations, not a
reason to clear the enabled setting or forget the saved float-ball geometry.

## Floating-Ball Motion Repair

The current four-second frontend polling interval is a confirmed source of
slow Fast response. Replace it with a native `FloatBallMotionMonitor`:

- sample Codex configuration metadata at a bounded 250ms cadence and parse
  only when it changes;
- derive an explicit `idle | thinking | fast` snapshot, with Fast taking
  precedence;
- emit a typed Tauri motion event immediately when the snapshot changes;
- retain a two-second frontend query only as a recovery fallback if the native
  event stream cannot be attached;
- treat active task detection as best-effort local activity evidence; if no
  trustworthy activity signal exists, use Idle rather than faking Thinking;
- parse Fast from an explicit configured tier/model condition, not a broad
  substring that can create false positives.

The floating-ball renderer uses a monotonic animation phase. A motion event
changes speed without resetting the rotation angle or reloading the webview:

| State | Speed |
| --- | --- |
| Idle | 1x |
| Thinking | 2x |
| Fast | 3x |

The animation stays clockwise and respects `prefers-reduced-motion` by
rendering a static product mark. It does not reintroduce breathing, size
changes, hover expansion, percent text, or close chrome.

## Settings and Storage Migration

New settings are small typed substructures rather than an unrelated second
store:

```text
PresentationIdentityPreferences
TaskbarPresentationPreferences
FloatBallPresentationPreferences
PanelPreferences
PricingPreferences
```

Migration rules:

1. missing fields receive documented defaults;
2. legacy 0..80 visual values scale to 0..100 without perceptible change;
3. obsolete tray/menu fields load but are ignored by the fixed tray;
4. unknown enum values and invalid quick-action layouts normalize safely;
5. Refresh is restored if absent/hidden from a panel layout;
6. unrelated settings survive every partial settings patch;
7. bridge DTOs, TypeScript types, settings tab whitelist, and persisted Rust
   settings change together.

The app uses the existing secure file pattern for identity metadata and atomic
writes for non-secret avatar/catalog/FX cache data. A failed migration must
preserve the original file and use the existing recovery path rather than
silently erase settings.

## Error States

| Situation | Required behavior |
| --- | --- |
| Account has no local name/avatar | Default product icon plus localized status; no email leak |
| Official avatar URL invalid/unavailable | Use default icon; no retry loop or raw URL diagnostic |
| Manual avatar invalid/oversized | Reject with localized inline message and retain current avatar |
| Slider save fails | Restore last confirmed value, show inline error, do not jitter during drag |
| Price source malformed/offline | Keep last valid snapshot with source age; no invented price |
| Unknown model | Keep token statistics, show `—` cost and include it in unpriced count |
| FX rate stale/missing | Show source currency or `—`; do not fabricate conversion |
| Start/Shell transient | Preserve enabled intent and geometry; recover automatically after close |
| Real full-screen | Hide only under the user-enabled preference; restore automatically on exit |
| Fast source unavailable | Retain last known state briefly, then Idle; never spin a console or spawn PowerShell |

## Delivery Stages

The release train is one `v1.1.0` goal but consists of independently reviewable
commits. No stage implies release publication.

1. **Foundation and proof:** create settings/DTO migration tests; add the
   in-memory Windows lifecycle trace; capture baseline Windows evidence.
2. **Identity and surfaces:** add presentation identity, protected avatar
   handling, synchronized panel/taskbar rendering, fixed tray, and panel
   personalization.
3. **Range-control consistency:** introduce the shared range field; migrate
   visual values to 0..100; remove all direct, bouncing range persistence.
4. **Pricing catalog:** implement source adapters, validator, cache, aliases,
   daily scheduling, USD/CNY conversion, local-cost UI, and failure states.
5. **Lifecycle and motion:** implement the trace-proven Shell state transition
   repair and event-driven Fast monitor; verify position preservation.
6. **Integration and release:** complete localization, migration cases,
   accessibility, Windows proof, installer smoke test, CI, release assets, and
   release notes.

## Verification

### Automated

- Rust tests for identity precedence, privacy filtering, avatar storage
  validation, settings migration, fixed-tray normalization, catalog schema
  validation, aliases, partial estimates, FX conversion, daily refresh/dedupe,
  Shell state transitions, and motion events.
- Sanitized source fixtures for each vendor; CI never depends on live pricing
  pages or current exchange rates.
- React/Vitest coverage for both locales, identity fallbacks, email-only
  selector visibility, taskbar/panel synchronization, every range field,
  settings migration outputs, panel layout controls, cost states, and motion
  event rendering.
- `cargo fmt --all -- --check`; clippy with warnings denied and tests for both
  Rust manifests; `pnpm test`; `pnpm run build`; and
  `scripts/local-check.ps1`.

### Windows proof

For every native/UI-affecting stage:

1. build a fresh Tauri binary and close old codex-barbar processes;
2. launch it in the relevant proof mode;
3. use CUA Driver on the real Windows desktop to exercise the setting and
   native surface; capture before/after screenshots;
4. restart and confirm persistence/migration;
5. prove taskbar width still follows only the hidden measurement window;
6. execute the Start/desktop/Explorer/Edge/full-screen matrix with the floating
   ball both on and off the taskbar;
7. confirm Fast changes are reflected within 500ms of a verified source change
   and do not reset the animation phase.

No unit test, mocked WebView, or successful command exit alone is accepted as
proof for native tray, Windows overlay, DWM, WebView2, or Shell behavior.

## Acceptance Criteria

1. A signed-in user sees their local official avatar and username in both the
   tray panel and taskbar; absence/failure renders the product icon safely.
2. Full email appears only in the tray-panel account selector.
3. Float ball remains an animated product mark with no text, box, hover
   expansion, or close control.
4. Every percentage slider is `0..100`, has correct semantic direction, does
   not bounce during drag, and preserves visual appearance through migration.
5. Settings no longer offer tray configuration; native tray behavior is fixed,
   compact, quota-coloured, and consistent.
6. Panel settings genuinely control density, optional detail lines, and
   eligible quick actions while Refresh remains available.
7. Cost table header is `Cost / 费用`; known direct/mapped models receive a
   source-labelled estimate, unknown models remain unpriced without zeroing
   known totals.
8. Price and FX sources refresh daily, survive offline operation using their
   latest valid cache, and never hard-code numeric model prices.
9. Clicking blank taskbar, opening Start/Explorer, minimizing Edge, or using
   `Win+D` never requires a Codex/Edge click to restore enabled surfaces.
10. Only verified real full-screen can hide surfaces, and only when the user
    enabled that preference.
11. Fast motion reacts within 500ms to a verified state change, stays smooth,
    and uses the approved 1x/2x/3x speeds.
12. About displays the installed backend/package version rather than a
    hard-coded UI value.
13. All automated gates, fresh Windows CUA proof, installer smoke test, and
    hosted CI succeed before `v1.1.0` is published.

## Non-goals

- Any action that redeems, purchases, changes plan/account state, or sends a
  model request.
- Browser-cookie import or webpage scraping for profile identity/avatar.
- A claim that local estimated cost is a provider invoice.
- User-created tray commands, scripts, executable paths, URLs, or native tray
  layouts.
- Cloud synchronization of avatars, settings, pricing cache, exchange rates,
  or local session logs.
- Support for ambiguous third-party model names through heuristic pricing.
- Replacing the existing provider refresh architecture or the independently
  measured taskbar width contract.
