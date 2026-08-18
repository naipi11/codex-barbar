# Codex App Server compatibility

codex-barbar V1 talks to the official but experimental `codex app-server`
stdio JSONL process. `experimentalApi` is always `false`, and the private
`/wham/*` HTTP path was removed without a release fallback.

## Tested version matrix

Only rows with local Windows evidence belong in this table. No untested
combination is marked compatible.

| Codex version | Installation form | Account store | initialize | account/read | rateLimits/read | Date | Notes |
|---|---|---|---|---|---|---|---|
| 0.146.0 | verifiedNpmLayout | not captured | ok | ok | ok | 2026-08-06 | Windows 11 x64; node.exe + official npm layout; fixed `\\?\` extended-prefix launch args; `experimentalApi: false` |

## How to add a row

1. Build the smoke example and run the local-only probe:

```powershell
cargo build --manifest-path rust/Cargo.toml --example codex_app_server_smoke
.\scripts\codex-app-server-smoke.ps1
```

2. Record only the redacted JSON summary and the exact version/installation
   form observed. Never record email, account id, quota values, tokens,
   full paths, raw RPC lines, or environment variables.
3. Add the schema capture under `rust/src/providers/codex/app_server/schema/<slug>/`
   and update this table with the version, installation form, account store
   mode, read results, date, and any issue note.

## Failure modes

- `CodexNotFound`: no safe `codex.exe` / verified npm layout was found. The
  Store alias is only accepted after a direct fixed-argument `--version`
  probe succeeds.
- `UnsupportedCodexVersion`: the resolved program failed a fixed-argument
  launch or returned a wire shape outside the frozen client contract.
- `NotSignedIn`: the current CLI has no ChatGPT account.
- `ApiKeyNoQuota`: the account authenticates with an API key and exposes no
  ChatGPT plan quota.

These are recorded honestly in diagnostics; the product does not fall back to
a private endpoint.
