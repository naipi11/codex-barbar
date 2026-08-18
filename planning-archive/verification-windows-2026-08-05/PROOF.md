# codex-barbar V1 Phase 0 — Windows manual/CUA proof (2026-08-05)

Binary under test: `.worktrees/v1-implementation/target/debug/codex-barbar.exe`
(fresh `pnpm run tauri:build:debug` of HEAD = Task 6 commit + Task 7 boundary changes).

CUA driver (`cua-driver`) is NOT installed on this host (`%LOCALAPPDATA%\Programs\Cua`
missing, no `cua-driver` on PATH). Equivalent manual/UIA proof collected instead
per AGENTS.md fallback: UI Automation window enumeration + raw-mouse clicks +
pixel screenshots, run interactively on the real Windows 11 desktop (2560x1440).

## Environment note
`CODEXBAR_PROOF_MODE=settings:general` was set for automation (suppresses
blur-dismiss so windows stay open for capture). First launch under
`CODEXBAR_PROOF_MODE=tray` also shown working (flyout toggles from tray icon).

## Evidence checklist

| Requirement | Evidence | Result |
|---|---|---|
| One tray icon present | `tray-overflow.png` — `codex-barbar` button in the system-tray overflow panel (`UIA: 'codex-barbar' ControlType.Button` at overflow rect 2092,1162) | PASS |
| Left-click tray icon opens tray flyout | `flyout-final.png` — flyout window (Tauri Window, rect 38,38,514,733) shows "codex-barbar v0.1.0-alpha.1", "Codex connection is not configured yet", buttons Settings / Dismiss / Quit | PASS |
| Settings opens as a separate window | `settings-final.png` — second Tauri Window (rect 740,285,1080,870) "codex-barbar Settings / About / Close" | PASS |
| Only V1 surfaces exist (no FloatBar/PopOut/legacy windows) | UIA enumeration of pid windows: exactly 2 `Tauri Window` instances (flyout + settings) + single-instance helper + Tao thread panes; no other app windows | PASS |
| Flyout hides on Dismiss | `flyout-after-click.png` earlier sequence: after clicking Dismiss the Tauri flyout window was no longer enumerated by UIA | PASS |
| Proof harness opens settings tab directly | `settings-window.png` (first run with `CODEXBAR_PROOF_MODE=settings:general`) — settings window visible immediately at launch | PASS |
| Rebrand/identity | both windows titled `codex-barbar`; flyout header shows `v0.1.0-alpha.1`; single-instance class `com.naipi11.codexbarbar-siw` | PASS |

## Command log (abridged)

```
$env:CODEXBAR_PROOF_MODE='settings:general'
Start-Process .\target\debug\codex-barbar.exe
# UIA: Shell_TrayWnd > 显示隐藏的图标 (chevron) > TopLevelWindowForOverflowXamlIsland
#   > 'codex-barbar' Button — click => flyout Tauri Window appears
# UIA WindowPattern: both windows state=Normal offscreen=False
# Screenshots via System.Drawing CopyFromScreen of each window rect
```

Screenshots in this folder:
- `tray-overflow.png` — overflow panel with codex-barbar icon
- `tray-area.png`, `taskbar-before.png`, `bottom-right.png` — taskbar captures
- `flyout-final.png` — tray flyout surface
- `settings-final.png` — settings surface
- `settings-window.png`, `flyout-window.png`, `flyout-2.png`, `settings-2.png` — earlier captures