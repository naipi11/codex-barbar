# Graphite Knot Icon Family Design

## Objective

Replace the unrelated blue code-glyph application icon and boxed numeric tray icon with one cohesive codex-barbar identity that stays legible from a 16 px Windows tray slot through a 1024 px installer asset.

## Approved Direction

The approved direction is **Graphite Knot**:

- Use the existing ChatGPT-style interlocking knot geometry already rendered by `ChatGptMark.tsx`.
- Application icon: a graphite rounded-square tile, a centered white knot, and a restrained emerald accent/glow.
- Tray icon: the simplified knot only on a fully transparent canvas; no number, text, progress bar, opaque square, or closing control.
- The exact percentage remains available in the taskbar status surface and tray tooltip. The tray icon communicates only the quota band.

## Visual System

### Application icon

- Canvas: 1024 × 1024 SVG source and PNG output.
- Container: rounded square with a large corner radius, dark graphite base, subtle center lift, and a thin low-contrast border.
- Mark: centered white knot with enough clear space to remain recognizable at 16–32 px.
- Accent: restrained emerald halo behind the mark. It must not blur the white silhouette or resemble a glossy blue mobile-app template.
- No letters, `</>` glyph, bars, percentage, account initials, or user-specific information.

### Tray icon

- Runtime canvas remains 32 × 32 RGBA.
- All four corners and the area outside the knot are transparent.
- Render one shared anti-aliased knot alpha mask with a one-pixel graphite keyline for light-taskbar contrast, then tint the inner mark by state:
  - Normal/high quota: rounded universal-weekly remaining `67..=100`, phosphor green `#56D98A`.
  - Warning/medium quota: rounded universal-weekly remaining `34..=66`, amber `#E2A33A`.
  - Danger/low quota: rounded universal-weekly remaining `0..=33`, red `#E24B55`.
  - Stale/API/unavailable: neutral slate `#9AA3B2` so missing data is not misrepresented as zero quota.
- The native tray and React status surfaces select the same universal weekly window from `primary` then `secondary`; 5-hour and model-specific additional windows never influence the band.
- The keyline follows the knot and its negative spaces; it must not become an opaque background rectangle. No embedded percentage digits.

### Small-size behavior

- The ICO must contain 16, 20, 24, 32, 48, 64, 128, and 256 px frames.
- The 16–32 px frames use the same silhouette with adequate padding and alpha edges.
- The center negative space must remain open at tray size.
- Windows light and dark taskbar backgrounds must both preserve recognizable edges.

## Architecture

- `rust/icons/codex-barbar.svg` becomes the editable canonical application-icon source.
- `rust/icons/codex-barbar.png` and `rust/icons/codex-barbar.ico` are generated bundle assets derived from that source.
- `rust/src/tray/render.rs` owns the deterministic 32 px tray alpha mask and state tinting. Existing `TrayVisualState` selection and tray tooltip semantics remain unchanged.
- `apps/desktop-tauri/src-tauri/tauri.conf.json` continues referencing the same bundle asset paths, avoiding installer/config churn.

## Non-goals

- Do not redesign the tray panel, taskbar status surface, floating ball, settings, or usage model.
- Do not change quota thresholds or provider selection.
- Do not add image libraries, build dependencies, or runtime SVG decoding.
- Do not touch the existing uncommitted universal-quota files.
- Do not commit, push, tag, or release without a separate user instruction.

## Acceptance Criteria

1. The tray output has transparent corners and no rectangular background.
2. Normal, warning, and danger icons share one knot silhouette and use the approved green/amber/red palette.
3. Stale/API/unavailable output uses neutral slate and cannot be confused with a danger quota.
4. The tray output contains no numeric/text glyphs or progress bar.
5. The application PNG is 1024 × 1024 and the ICO contains all required frames.
6. The SVG, PNG, ICO, and runtime tray icon clearly belong to the same Graphite Knot family.
7. Focused tests, both Rust manifests, frontend checks, formatting, and clippy pass.
8. A fresh Tauri debug build is launched after closing older instances; the application icon and tray icon are visually checked on Windows with CUA or equivalent DPI-aware screenshots.
