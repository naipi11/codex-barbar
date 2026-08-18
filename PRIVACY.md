# Privacy

codex-barbar is a local Windows tray application. It does not phone home,
does not collect telemetry, and does not share your account data with any
server other than the Codex App Server you already use.

## What is stored and where

All application data lives under `%LOCALAPPDATA%\codex-barbar`:

| Path | Content |
|---|---|
| `data\codex-barbar.db` | SQLite database: profiles, usage snapshots, refresh state |
| `vault\` | DPAPI-protected credential envelopes for managed profiles |
| `runtime\` | Isolated per-profile `CODEX_HOME` runtimes (temporary) |
| `logs\` | Rotating redacted logs, 5 MiB per segment, 14-day retention |
| `diagnostics\` | Redacted diagnostic exports you create from Settings |

The installed build is a per-user NSIS install under
`%LOCALAPPDATA%\Programs\codex-barbar`; the portable ZIP writes nothing
beside the executable. Both store data in the same `%LOCALAPPDATA%`
location.

## Credentials

- The current Codex CLI profile is read-only. codex-barbar never writes,
  rotates, or removes your CLI authentication.
- Managed profiles run with an isolated `CODEX_HOME` and force
  `cli_auth_credentials_store = "file"`. Idle credentials are encrypted with
  Windows DPAPI using Current User scope only; there is no Local Machine
  fallback and no plaintext fallback.
- The React WebView never receives OAuth tokens, refresh tokens, raw
  `auth.json`, or arbitrary filesystem/process/network capability.

## Logs and diagnostics

- Logs are line-oriented, redacted before writing, rotate at 5 MiB, and are
  removed after 14 days.
- Diagnostics are written only when you export them from Settings. The
  export path is fixed, the payload is scanned before and after
  serialization for secret patterns, and a failed final scan discards the
  temporary file and preserves the previous export.

## Network boundary

- Account and quota traffic goes only through the official Codex App Server
  process using its stdio protocol. There is no direct HTTP call to private
  Codex endpoints.
- Startup performs no update check, download, or apply. A manual action may
  query the public GitHub Releases feed or open the fixed Releases page.
- No PAT or API key is embedded or requested.

## Deletion

Close the app and delete `%LOCALAPPDATA%\codex-barbar`, or choose the
explicit “Delete local codex-barbar accounts and cache?” confirmation in the
uninstaller. The uninstaller never deletes data without that explicit
confirmation.
