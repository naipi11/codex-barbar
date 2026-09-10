# Troubleshooting

Error states shown in the tray panel or Settings map to the following
recovery steps. None of these steps require an administrator account.

| Error state | Meaning | What to do |
|---|---|---|
| Codex not found | No Codex CLI / App Server executable could be resolved | Install Codex, then use **Settings → Providers → Validate Codex executable** or restart codex-barbar |
| Unsupported Codex version | The resolved executable failed the fixed `--version` probe or the App Server wire contract | Check the diagnostic code, then update codex-barbar or Codex and retry; the tested matrix is evidence, not a hard-coded allowlist |
| Not signed in | The current CLI has no signed-in account | Run `codex login` in a terminal, then refresh |
| API key, no quota | The account authenticates with an API key, which exposes no quota data | Sign in with a ChatGPT/Codex account instead of an API key |
| Authentication expired | Stored or CLI-side authentication has expired | Re-authenticate with `codex login`, then refresh |
| Offline or timeout | Network failure, process hang, or RPC timeout | Check the network and Codex process, then refresh; codex-barbar keeps the last successful snapshot visible |
| Rate limited | The server asked the app to back off | Wait for the reset/backoff window; refresh is retried automatically with longer intervals |
| Protocol mismatch | The App Server wire protocol did not match the frozen schema | Export diagnostics, then update codex-barbar or Codex and report the redacted error code |
| Vault failure | DPAPI or vault read/write failed | Close codex-barbar and retry; if it persists, export a diagnostic and report the issue. Credentials are never written in plaintext |
| Storage failure | Local settings/database storage failed | Check that `%LOCALAPPDATA%\codex-barbar` is writable and not full; export diagnostics before deleting anything |

## Common questions

**Why does SmartScreen warn about the installer?**
Release binaries are unsigned until an Authenticode certificate is supplied.
Verify `SHA256SUMS.txt` from the release page before running the installer.

**Where is my data?**
`%LOCALAPPDATA%\codex-barbar`. The portable build uses the same location and
never writes next to the executable.

**How do I completely remove the app and its data?**
Uninstall codex-barbar. Uninstall keeps data by default; choose the explicit
“Delete local codex-barbar accounts and cache?” confirmation only when you
want the data removed.

**Why is the usage panel empty or stale?**
Refresh is manual during the first run and then automatic every 5 minutes by
default. If the App Server process cannot start, the panel shows a redacted
error and keeps the last successful snapshot.

**Why does using WSL affect the account shown?**
codex-barbar reads the Windows Codex installation and Windows-side sign-in
state. Running an agent or project inside WSL does not share WSL's
`~/.codex/auth.json`; it cannot supply the Windows account automatically.
Sign in with the Windows Codex CLI (`codex login` in PowerShell), then use
**Settings → Providers → Validate Codex executable** and refresh. If the app
was launched from the Start menu, validation also checks the standard
per-user OpenAI Codex install location when the terminal `PATH` is not
available.

**Can I use this on Windows 10 or ARM?**
No. V1 targets Windows 11 23H2 or newer on x64 only.

**Does the app update itself?**
No. Startup never checks for or downloads updates. Use the manual
**Check for updates** action in Settings, then install the new release.

If an issue persists, export a diagnostic from **Settings → Advanced** and
attach it to the bug report. Diagnostics are redacted and contain no
credentials.
