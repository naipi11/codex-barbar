# Upstream sources

| Role | Repository | Frozen baseline |
|---|---|---|
| Windows implementation base | https://github.com/Finesssee/Win-CodexBar | `b167e328147b93f997034a6b50c8b769d2a37f3b` / `upstream/win-codexbar-2026-08-03` |
| Behavior reference | https://github.com/steipete/CodexBar | Reference only; no Swift platform code is shipped |

codex-barbar preserves the imported Win-CodexBar Git history and MIT license. V1 reuses its Tauri, React, Rust, Windows tray, testing, and packaging foundations while replacing the Codex private HTTP integration and removing non-Codex product surfaces from the release graph.
