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
| 1.0.0 | Codex CLI 0.146.0 | 2026-08-08 | Windows 11 23H2 x64, current CLI profile (ChatGPT) |
| 1.0.0-rc.1 | Codex CLI 0.146.0 | 2026-08-07 | Windows 11 23H2 x64, current CLI profile (ChatGPT) |

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
- The App Server protocol is experimental; protocol mismatches surface as
  redacted errors with a recovery hint, never raw protocol text.
