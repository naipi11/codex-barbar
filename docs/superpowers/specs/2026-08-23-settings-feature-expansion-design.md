# Settings Feature Expansion Design

## Status

Approved by the project owner on 2026-08-23.

## Objective

Complete the currently placeholder V1 settings categories without reviving the
legacy multi-provider settings stack:

- show the real installed application version in About;
- add reliable, opt-in Windows notifications;
- turn the inherited "Menu bar" category into Windows-specific taskbar and
  tray presentation settings;
- let users configure the built-in tray menu and tray-panel quick actions;
- add a read-only usage-and-spend view backed by official Codex quota data and
  local Codex session logs.

The work must preserve the current V1 architecture, keep provider and account
data correctly siloed, and remain strictly observational. codex-barbar must not
redeem reset credits, purchase usage, change an OpenAI account, or expose any
other action that can consume a user's entitlement or money.

## Approved Product Boundaries

### Read-only Codex integration

codex-barbar may:

- read the universal weekly Codex allowance;
- read the weekly reset time and snapshot freshness;
- read the number of available banked reset credits when the Codex app-server
  returns it;
- scan local Codex session logs for token counts and local cost estimates;
- notify the user about observed state changes.

codex-barbar must not:

- register or expose the app-server reset-credit consumption method;
- provide buttons for applying a reset, buying credits, changing a plan, or
  modifying official Codex account state;
- present a local cost estimate as an OpenAI invoice or authoritative bill;
- guess missing quota, reset-credit, model-pricing, or account-attribution
  data.

### Windows terminology

The V1 settings tab identifiers remain unchanged for bridge compatibility, but
the user-facing `menuBar` title changes:

- English: **Taskbar & Tray**
- Simplified Chinese: **任务栏与托盘**

The `menu` tab remains **Menu / 菜单** and owns only built-in tray-menu and
tray-panel action configuration.

## Architecture Decision

Use the active V1 settings and bridge architecture rather than one of these
rejected alternatives:

1. **Frontend-only settings:** fast to draw but cannot provide reliable
   background notifications or native tray-menu updates while the settings
   window is closed.
2. **Wholesale legacy-settings port:** feature-rich but would create two
   competing settings stores and import unrelated multi-provider behavior.

The approved architecture extends `storage::AppSettings`, adds focused Rust
services and DTOs, and reuses existing domain modules only through Codex-specific
adapters.

```text
Codex app-server / local Codex logs
                 |
                 v
       Rust parsing and aggregation
                 |
                 v
 cache + notification decisions + cost estimation
                 |
                 v
        typed Tauri DTOs and events
                 |
                 v
 settings / taskbar / tray / tray panel
```

Rust remains the source of truth. React surfaces render typed state and send
validated settings patches; they do not independently derive notification,
quota, pricing, or menu-normalization behavior.

## Information Architecture

The settings navigation keeps the existing stable tab IDs:

| Tab ID | User-facing title | Scope |
| --- | --- | --- |
| `general` | General / 通用 | Startup, refresh, language, theme, floating-ball controls |
| `providers` | Accounts / 账户 | Existing Codex account management |
| `notifications` | Notifications / 通知 | Opt-in toast behavior and thresholds |
| `menuBar` | Taskbar & Tray / 任务栏与托盘 | Status overlay and native tray presentation |
| `menu` | Menu / 菜单 | Built-in menu and quick-action visibility/order |
| `usageSpend` | Usage & Spend / 用量与费用 | Read-only quota, reset credits, tokens, and estimates |
| `advanced` | Advanced / 高级 | Existing executable and diagnostics controls |
| `about` | About / 关于 | Version, update state, license, and project links |

Existing taskbar-related controls currently shown in General move to Taskbar &
Tray. The underlying persisted fields may be retained for compatibility, but
there must be one user-facing control for each behavior.

## About

### Required UI

- Product name and a dedicated current-version row.
- The version is the live `BootstrapDto.version` value supplied from
  `CARGO_PKG_VERSION`; it is never duplicated in localized copy.
- "Check for updates" retains an inline state:
  - checking;
  - already current;
  - update available;
  - check failed with a retry action.
- Release-page, project-homepage, and MIT-license links.
- Platform label for the current Windows build.

The version remains visible offline and when the update check fails.

## Notifications

### Defaults and master control

- Add a prominent `notificationsEnabled` master switch.
- The master switch defaults to **off** for new and upgraded installations so
  an update never introduces unsolicited popups.
- Per-event preferences are retained while the master switch is off.
- Enabling notifications establishes a baseline from the current snapshot; it
  must not replay historical threshold or reset-credit events.

### Event controls

Each event has an independent switch:

1. remaining quota enters the warning band;
2. remaining quota enters the danger band;
3. the universal weekly allowance completes a reset;
4. available banked reset-credit count increases;
5. refresh fails three consecutive times, followed by a single recovery
   notification;
6. a newer codex-barbar release is detected.

A separate sound switch controls notification sounds, and a safe "Send test
notification" button proves Windows toast availability without changing any
Codex state.

### Thresholds

Thresholds use **remaining percentage**, matching the status surfaces:

- `67..=100`: normal/green;
- `34..=66`: warning/amber;
- `0..=33`: danger/red.

The default warning and danger entry thresholds are therefore 66 and 33.
Users may adjust both thresholds. Validation requires:

```text
0 <= danger < warning <= 100
```

Invalid patches are rejected by Rust and explained inline; they are not silently
rewritten into a different user choice.

### Dedupe and lifecycle

- The first valid snapshot arms state but does not notify.
- One provider/account/window/band transition produces at most one toast.
- Remaining in the same band across refreshes produces no further toast.
- Recovery to a healthier band, a new weekly window, or a changed account
  re-arms the relevant transition.
- Reset-credit increase is computed only between two known counts. `null -> N`
  establishes a baseline; `N -> greater N` notifies once; decreases do not
  notify.
- Refresh-failure notification fires on the third consecutive failure and
  remains deduped until a successful refresh. Recovery fires once.
- Dedupe state is stored in a small non-secret runtime-state file so restarting
  the app does not immediately repeat a toast.

Clicking a toast may open the relevant codex-barbar surface. It must not invoke
an OpenAI or Codex mutation.

## Taskbar & Tray

### Taskbar status overlay

Provide controls for:

- show/hide the taskbar status overlay;
- show/hide the product icon;
- show/hide the account short name;
- show/hide the weekly label;
- show/hide the numeric remaining percentage;
- show/hide the weekly reset date;
- compact or standard density;
- existing taskbar opacity;
- hide status overlays during a detected full-screen application.

At least one meaningful status element must remain enabled when the overlay is
enabled. The UI explains and prevents an all-empty layout.

The account short name has no length setting. When enabled, it is exactly the
first six Unicode grapheme clusters of the account display name, or of the
email local part before `@` when no display name is available. If neither is
available, the account element is omitted rather than replaced with a false
identity.

The full-screen preference applies to taskbar and floating status overlays but
does not remove the native tray icon. It uses the existing Windows full-screen
detection path and defaults to on.

### Native tray

Provide controls for:

- dynamic quota-band icon or neutral monochrome icon;
- which tooltip rows appear: account short name, weekly remaining, reset date,
  and last update time.

Dynamic icon color continues to use the same universal-weekly quota selector
and shared 67/34/33 band semantics as the floating ball and taskbar status.
Model-specific 5-hour windows must not affect it.

All presentation logic lives in shared pure Rust/TypeScript view models so the
tray, taskbar, floating ball, and settings preview cannot disagree about the
selected universal window or status band.

## Menu

### Configurable surfaces

Configure these independently:

1. native tray right-click menu;
2. tray-panel quick actions.

Each surface uses a registry of stable built-in item IDs. Users may:

- show or hide eligible items;
- reorder items by drag and drop;
- reorder with keyboard-accessible up/down controls;
- restore the surface's default layout.

Users may not add commands, scripts, arguments, URLs, or executable paths.

### Safety normalization

- Settings and Quit remain mandatory in the native tray menu.
- Unknown IDs from a newer/older version are ignored safely.
- Duplicate IDs are collapsed deterministically.
- Missing mandatory items are restored when settings load.
- Separators are generated from visible groups rather than stored as arbitrary
  user items, preventing empty or repeated separators.
- An invalid layout falls back to defaults without crashing startup.

Saving a valid layout rebuilds the native tray menu and updates the tray panel
immediately; no process restart is required.

## Usage & Spend

This entire tab is read-only.

### Official allowance card

Show only the universal weekly Codex allowance:

- rounded remaining percentage;
- weekly reset time in the selected locale/time zone;
- snapshot freshness and last successful update time;
- normalized data state;
- available banked reset-credit count when supplied.

Do not surface a model-specific 5-hour allowance as the primary or universal
quota. The reset-credit field distinguishes:

- known count, including zero;
- unsupported/not returned;
- stale because the provider refresh failed.

### Reset-credit DTO

Extend the Codex app-server response parser to read the existing
`rateLimitResetCredits` summary. Expose only non-sensitive read-only data needed
by the product:

```text
availableCount: integer >= 0
observedAt: timestamp
state: available | unsupported | stale
```

Opaque credit identifiers are not forwarded to React or persisted. The
app-server consume method is not wrapped by a Tauri command.

### Local usage ranges

Offer these views:

- Today;
- Last 7 days;
- Last 30 days;
- Current weekly allowance period, when a trustworthy reset time is available.

Aggregate:

- uncached input tokens;
- cached input tokens;
- output tokens;
- total tokens;
- local session count;
- daily totals;
- totals grouped by model.

Reuse the existing Codex JSONL/cost scanner through a V1 read-only service. The
scan is incremental and cached, runs off the UI thread, and can be cancelled
when the window closes. Opening the tab requests a refresh subject to the
existing debounce rather than rescanning all files for every React render.

### Account attribution

Attribute a local session only when its runtime home or durable metadata proves
the selected account. If local logs cannot be separated reliably, label the
result **This device combined / 此设备合计**. Never display device-wide totals as
though they belong to the selected OpenAI account.

### Cost estimates

- Resolve known model prices with the existing pricing infrastructure and
  cached price metadata.
- Display the pricing source and its last update time when available.
- Show a persistent **Local estimate, not an OpenAI bill / 本地估算，并非
  OpenAI 账单** badge.
- Unknown models still contribute tokens but contribute no guessed cost.
- List models whose price could not be resolved.
- If pricing is unavailable, keep the usage view functional and mark cost as
  unavailable.

The first release includes a daily usage/cost trend and a per-model table. It
does not export or upload logs.

## Settings Storage and Migration

Extend `rust/src/storage/settings_repository.rs::AppSettings` with small typed
substructures rather than a flat collection of loosely related flags:

```text
NotificationPreferences
TaskbarTrayPreferences
MenuPreferences
```

The existing taskbar enabled/opacity values remain backward compatible. New
fields use `serde` defaults and normalize on load. The migration rules are:

1. missing notification preferences produce a disabled master switch;
2. existing taskbar and tray behavior remains visually unchanged unless the
   user changes a new setting;
3. missing menu layouts produce the current built-in order;
4. unknown enum values and invalid menu entries recover to documented defaults;
5. a settings save preserves fields unrelated to the active tab.

Rust DTOs, `types/bridge.ts`, runtime validators, fixtures, and tests must change
together. The existing case-sensitive tab IDs remain synchronized between
`SettingsTabId`, `TAB_META`, and `surface_target.rs`.

## UI and Interaction

- Replace placeholder copy with compact, grouped setting cards; do not leave
  the current mostly empty content canvas.
- Save ordinary changes immediately and show a short localized saved state.
- Keep the current settings window responsive at its supported minimum size.
- English and Simplified Chinese ship together for every label, helper, empty
  state, error, toast, tooltip, and test fixture.
- Controls have visible focus, labels, disabled explanations, and keyboard
  operation.
- Menu reordering has non-drag keyboard controls.
- Loading, empty, stale, unsupported, corrupted-log, and refresh-failure states
  are distinct. Do not collapse them into the old generic protocol anomaly
  message.

## Error Handling

- **Windows notifications unavailable:** keep settings usable, show an inline
  diagnostic, and let the test-notification action retry.
- **Codex app-server omits reset credits:** render Unsupported/Not returned, not
  zero.
- **Usage refresh stale:** preserve the last known value with an explicit stale
  timestamp.
- **Logs missing:** show a local empty state with the expected non-secret log
  location category, not an error toast loop.
- **Partial/corrupt JSONL:** skip malformed records, report a sanitized skipped
  count, and continue aggregating valid records.
- **Pricing unavailable:** show tokens and suppress only the cost total.
- **Menu rebuild fails:** retain the last working native menu, report the save
  failure, and do not persist a layout that was not applied.
- **Settings file invalid:** use the existing safe recovery path and never erase
  a recoverable user configuration silently.

No error or diagnostic output may contain tokens, cookies, raw API keys, reset
credit identifiers, or unredacted session contents.

## Delivery Plan

### Milestone 0 — About version

- pass `BootstrapDto.version` into About;
- remove hard-coded version copy;
- add localized version and update-result states;
- add focused frontend tests.

### Milestone 1 — Settings foundation and notifications

- add typed settings structures and migration defaults;
- add notification DTOs and UI;
- adapt the existing notification engine to V1 Codex snapshots;
- add persistent dedupe state and reset-credit increase handling;
- validate with real Windows toasts.

### Milestone 2 — Taskbar & Tray

- move existing presentation controls from General;
- add visibility, density, account, tooltip, icon-style, and full-screen fields;
- centralize presentation selection;
- rebuild and validate native tray/taskbar behavior on Windows.

### Milestone 3 — Menu configuration

- create stable registries and normalization;
- implement both layout editors;
- rebuild the tray menu and panel actions live;
- prove keyboard ordering and safe fallback.

### Milestone 4 — Usage & Spend

- parse read-only reset-credit summary;
- expose official weekly allowance and freshness;
- add incremental local token aggregation and pricing estimates;
- implement range selection, trend, model table, and all empty/error states.

### Milestone 5 — Integration polish

- complete English and Simplified Chinese coverage;
- run migration and corrupted-state matrices;
- check settings layout at supported sizes and DPIs;
- capture final Windows proof for all new tabs and native surfaces.

Each milestone is a scoped commit/review unit. A milestone does not imply a tag,
push, installer, GitHub release, or Winget submission without a separate user
instruction.

## Verification

### Automated

- Rust unit tests for settings defaults/migration/normalization.
- Notification transition, dedupe, restart, account, and weekly-window tests.
- Codex app-server fixtures for reset-credit known/zero/null/missing/malformed
  cases.
- Menu registry, mandatory-item, duplicate, unknown-ID, and rebuild rollback
  tests.
- JSONL aggregation, date-range, account-attribution, unknown-price, and partial
  corruption tests.
- Frontend tests for every tab, locale, loading/empty/error state, threshold
  validation, master switch, field visibility, and keyboard ordering.
- `cargo fmt --all -- --check`.
- Clippy and tests for both Rust manifests with warnings denied.
- Frontend Vitest and production build using pnpm.
- `scripts/local-check.ps1` before integration handoff.

### Windows UI proof

For each UI/native milestone:

1. build a fresh Tauri desktop binary;
2. close every older running instance;
3. launch the fresh binary in the relevant proof mode where available;
4. use CUA Driver to exercise controls and capture before/after screenshots;
5. restart the app and prove settings persistence;
6. verify the real tray menu, taskbar overlay, tooltip, toast, language switch,
   full-screen behavior, and empty/error presentation as applicable.

Frontend unit tests alone are not evidence for Windows toast, native menu,
taskbar, WebView2, or full-screen behavior.

## Non-goals

- Supporting providers other than Codex in these V1 tabs.
- Consuming banked resets or changing OpenAI/Codex account state.
- Purchasing credits or linking to a purchase action from notification toasts.
- Replacing OpenAI billing or analytics.
- Cloud-syncing settings or local usage history.
- Exporting raw session logs.
- User-defined scripts, commands, menu URLs, or executable launchers.
- Replacing the existing Accounts or Advanced implementation.
- Introducing a second settings store.

## Acceptance Criteria

1. About always shows the installed package version from the backend.
2. Notifications are disabled by default and no toast fires until the user
   enables the master switch.
3. Enabled notifications dedupe across refreshes and app restarts.
4. Reset-credit count is read-only, distinguishes zero from unavailable, and
   never exposes a redeem action or identifier.
5. Taskbar account text has only a visibility switch and, when shown, is fixed
   to six grapheme clusters.
6. Taskbar, tray, and floating surfaces use one universal-weekly quota/band
   definition and ignore model-specific 5-hour limits.
7. Built-in menu items can be hidden and reordered, mandatory native items are
   preserved, and malformed layouts recover safely.
8. Usage & Spend shows official weekly quota separately from local estimates,
   labels device-combined data honestly, and never guesses unknown prices.
9. All new settings persist through restart and migrate without changing the
   current user's existing presentation unexpectedly.
10. English and Simplified Chinese cover all new UI and notification text.
11. Automated checks pass and fresh Windows CUA proof verifies the native
    behaviors before release.
