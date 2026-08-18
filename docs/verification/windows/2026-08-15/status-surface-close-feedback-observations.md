# Status-Surface Close Feedback - Windows Verification Observations

Date: 2026-08-16
Branch: codex/status-surface-accuracy-opacity
Result: **PARTIAL - decisive locked-persistence retry not proven on the live host**

## Tool status

CUA Driver was not installable: the GitHub release asset fetch failed
repeatedly with TLS/SSL connection errors. The documented Win32 fallback was
used instead: window enumeration, extended-style inspection,
screen-composited captures, and WebView2 CDP DOM reads. CUA-only
click/type/UIA automation was not available.

## Source and build gates (PASS)

- HEAD before verification: a3a2723394b31a946b897bd7b65343077b7b9ec1
- Plus test-stability commit 9b933341b2a7bba5b0b6146b8950b3c3a60bc015
- Focused Rust: app_server 72, settings_repository 9, proof_harness 21,
  taskbar_overlay 32, status_surfaces 29 - all PASS.
- Full frontend: 26 files / 197 tests PASS (after stabilizing async waits).
- local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy: PASS.
- Production frontend build (tsc --noEmit && vite build): PASS.
- Fresh debug binary SHA-256:
  197F86FC465CA8A69BC96F38E9AFF83AE1A2EEB237CFFEFD5105084D23E1940E.
- The DOM proof used the dev-URL debug build with the Vite dev server; the
  asset-embedded debug build intermittently failed to create the first
  WebView window in this session (see Limitations).

## taskbar-status:weekly proof (PASS)

Fresh WebView2 user-data folder, dev-URL debug build:

- Visible taskbar window: 318x40 logical, WS_EX_LAYERED | WS_EX_TOOLWINDOW |
  WS_EX_NOACTIVATE | WS_EX_TOPMOST, inside the bottom taskbar at logical
  (843,779)-(1161,819).
- Measurement helper: hidden 318x40 window, label taskbar-status-measure.
- No float-ball window (proof projection disables it).
- DOM innerText of the visible surface, exact:
  P / ProofU / 周 98% / 8/20 / x
- No 5H cell present.
- Screen-composited capture: screenshots/close-feedback-taskbar-weekly.png.

## Opacity proof (PASS)

Settings toggled through the typed command via CDP while the surface kept the
same HWND and geometry (233x40):

- taskbarStatusOpacity 0 -> computed --surface-bg-alpha 0, capture hash
  B4C842B8EA16DD63396D97294B80A73D69EC993A8895EC964918D0DD575E8142.
- taskbarStatusOpacity 80 -> computed --surface-bg-alpha 0.8, capture hash
  4F61E130360FA556295D80716D94A5DBB60EC5E20F806698A6EF1DE062A65A0A.
- Hashes differ; screen-composited alpha changed materially.
- Settings persisted for both values.

## Unlocked close proofs (PASS)

- Frontend close: CDP click on the real close button destroyed the visible and
  measurement windows and persisted taskbarStatusEnabled=false.
- Native close: WM_CLOSE posted to the exact root HWND of the visible taskbar
  window destroyed both windows and persisted taskbarStatusEnabled=false.
- Windows recreated by re-enabling through the typed command before the next
  proof.

## Locked persistence retry (NOT PROVEN on live host)

The exclusive-writer sequence was attempted with several independent
file-lock strategies against the WAL database (FileShare.None, PENDING byte
lock, WAL file lock, WAL write lock, shm lock range). Every attempt still
persisted false, so no live rollback to true and no live red retry state
could be exercised. The controller rollback path is covered by Rust unit
tests (status_surfaces 29 passing, including persistence-save-failure
rollback and feedback emission). Live locked proof remains open.

## Limitations (recorded, not hidden)

- The asset-embedded debug build intermittently failed to create the first
  WebView window (style application on the first window did not apply; the
  window stayed hidden). The dev-URL build with a fresh WebView2 profile
  created windows reliably. This is a session-level WebView2 timing anomaly,
  not a source change in Tasks 1-4; the same window lifecycle code was used
  for every launch.
- Forced process termination during diagnosis left the rolling log unflushed
  (0 bytes), so tracing evidence is unavailable for failed launches.
- CUA driver unavailable (GitHub asset download blocked by TLS).
- No float-ball or Settings live proof was completed in this run.
- User settings restored to original values (taskbar disabled, both
  opacities 20, float ball enabled).

## Evidence files

- screenshots/close-feedback-taskbar-weekly.png
- screenshots/close-feedback-taskbar-opacity-0.png
- screenshots/close-feedback-taskbar-opacity-80.png
