# Tested Codex Versions

codex-barbar V1 reads usage through the official Codex App Server protocol.
The compatibility matrix below records native Windows acceptance evidence;
versions outside it are unverified, not automatically rejected.

## Compatibility semantics

The executable check verifies that the resolved Codex command can be launched
with the fixed `--version` probe. A detected version is not, by itself, proof
that every App Server method is compatible; the refresh handshake remains the
runtime compatibility check and reports a redacted protocol error when it
fails.

The matrix below is acceptance evidence, not a hard-coded version allowlist.
When a new Codex version is verified on native Windows, add a row with the
exact codex-barbar version, Codex version, OS build, and result.

| codex-barbar | Codex version tested | Date | Notes |
|---|---|---|---|
| 1.1.5 (local, unpublished) | Codex CLI 0.167.0 | 2026-11-08 | Windows 11 x64, OS 10.0.26200.0; nativeExe; real initialize/account/read/account/rateLimits/read succeeded for the signed-in Current CLI. Separate empty CODEX_HOME returned signedOut as expected. |
| 1.0.0 | Codex CLI 0.146.0 | 2026-08-08 | Windows 11 23H2 x64, current CLI profile (ChatGPT) |
| 1.0.0-rc.1 | Codex CLI 0.146.0 | 2026-08-07 | Windows 11 23H2 x64, current CLI profile (ChatGPT) |

## 0.167.0 verification scope

- Latest stable checked on 2026-11-08: [rust-v0.167.0](https://github.com/openai/codex/releases/tag/rust-v0.167.0), published 2026-11-06T01:40:00Z. This is dated evidence, not a guarantee for future releases.
- Real read-only smoke: codexVersion=0.167.0, initialized=true, accountState=signedIn, rateLimitsMethod=available, experimentalApi=false. No raw account, token, quota or RPC payload was retained.
- Isolated signed-out smoke: accountState=signedOut, rateLimitsMethod=notChecked. This is transport coverage, not authenticated quota coverage.
- Relevant official schemas and source/SHA-256 provenance: [0.167.0 manifest](../rust/src/providers/codex/app_server/schema/0.167.0/manifest.json). Synthetic fixtures cover nullable windows, multi-bucket quota, credits and unknown fields. Historical 0.146.0 fixtures remain; that CLI version was not reinstalled or re-run on this host.
- Real managed browser/device login, keyring variants, GCP/Bedrock quota support and the full installer/multi-monitor matrix are not established by these smoke checks. Unsupported account variants fail closed.

## How to test a new Codex version

1. Install the Codex version and sign in with `codex login`.
2. Launch a fresh codex-barbar build and confirm the tray panel shows the
   signed-in profile, quota windows, and reset times.
3. Exercise offline, timeout, and rate-limit states.
4. Record the exact version (`codex --version`), date, OS build, and result
   in this table, then update `docs/WINDOWS_ACCEPTANCE.md` if needed.

## Boundary

- `UnsupportedCodexVersion` means the fixed executable probe or the runtime
  App Server wire contract failed; being absent from this matrix alone does
  not reject a version.
- Experimental API capabilities remain disabled. Protocol mismatches surface
  as redacted errors with a recovery hint, never raw protocol text. Pinning
  a schema documents a contract; runtime reads remain required for acceptance.
