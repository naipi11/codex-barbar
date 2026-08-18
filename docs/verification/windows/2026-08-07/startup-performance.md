# codex-barbar cached-start performance — 2026-08-07

- OS: Windows 11 (host-local, no device identifiers recorded)
- Build: fresh `target/debug/codex-barbar.exe` from `codex/v1-implementation`
- Mode: `CODEXBAR_PROOF_MODE=trayPanel:ready`; time from process start to visible main window
- Measurement: PowerShell Stopwatch + Win32 IsWindowVisible polling (CUA driver unavailable on this host)

| Run | Startup (ms) | Visible window |
|---|---|---|
| 1 | 1203 | True |
| 2 | 67 | True |
| 3 | 69 | True |
| 4 | 67 | True |
| 5 | 66 | True |

All runs must be ≤ 3000 ms per the V1 cached-tray budget.
