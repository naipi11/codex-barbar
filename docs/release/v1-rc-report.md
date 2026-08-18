# codex-barbar 1.0.0-rc.1 acceptance report

Date: 2026-08-08 (Asia/Shanghai)

## Build identity

- Commit: `9656421cbbe667bd7fd139f2efea713219681c42`
- Version: `1.0.0-rc.1`
- Target: `x86_64-pc-windows-msvc` (Windows 11 x64)
- Signed: no (unsigned build; SmartScreen warning expected)

## Artifacts

```text
codex-barbar_1.0.0-rc.1_x64-setup.exe      f17d4a6fcb59fda20bf01afc7a29ed4668ff7fc76de2c7323891a9925a65d346
codex-barbar_1.0.0-rc.1_x64-portable.zip   f1fbb1164c9ad3ff27766a0cf448e834809c482daaf61a8edfc1cc9ac1d8da4a
SHA256SUMS.txt                             shipped alongside (213 bytes)
codex-barbar_1.0.0-rc.1_sbom.spdx.json     795 packages / 794 DEPENDS_ON relationships
artifact-manifest.json                     version, commit, sizes, hashes, buildTime
```

## Automated gates (all exit 0)

- `cargo fmt --all --check`
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path rust/Cargo.toml` (251 passed, 1 ignored)
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` (59 passed)
- `pnpm test` (47 passed) and `pnpm run build`
- `scripts/assert-v1-boundaries.ps1`
- `pnpm audit --prod --audit-level high` (no known vulnerabilities)
- `scripts/audit-licenses.ps1` (locked Cargo + pnpm graph passes policy)
- `scripts/generate-sbom.ps1` deterministic run (795 packages, 794 relationships)
- `scripts/windows-release-build.ps1 -Ref 9656421cbbe667bd7fd139f2efea713219681c42 -Version 1.0.0-rc.1`
- `scripts/verify-release-artifacts.ps1` and `scripts/release-doctor.ps1`

## Codex matrix (host evidence, no identities recorded)

| Scenario | Result | Evidence |
|---|---|---|
| Codex CLI present, signed in (ChatGPT) | PASS | `codex --version` 0.146.0; `codex login status` reports ChatGPT; real release launch stayed running |
| Read-only CurrentCli profile | PASS | Rust tests: current CLI session has no login/logout/switch/delete methods |
| Not signed in / API key / expired auth | PASS (unit) | `AppErrorKind` + recovery mapping tests; proof scenario `trayPanel:api` renders API-key state |
| Offline / timeout / rate limit / protocol mismatch | PASS (unit + proof) | proof scenarios `trayPanel:stale` / `trayPanel:error`; protocol/backoff tests |
| Managed login browser/device-code, two profiles, rename/remove | PASS (unit) | account service fixtures; proof scenario `trayPanel:profiles` |
| Vault recovery / App Server crash | PASS (unit) | recovery, job kill-on-close, crash-preserves-old-vault tests |
| Real disposable-account matrix | NOT COVERED | requires disposable accounts on clean machine (documented in WINDOWS_ACCEPTANCE.md) |

## Windows matrix

| Scenario | Result | Evidence |
|---|---|---|
| Windows 11 x64 host | PASS | host-local runs |
| Tray panel + settings windows | PASS | proof screenshots `docs/images/windows-proof/v1/` |
| Cached startup ≤ 3 s | PASS | 5 runs 66–1203 ms |
| Portable ZIP smoke | PASS | temp expansion, GUI launch, no files beside exe |
| NSIS fresh/upgrade/default-retain uninstall | PASS | smoke-install on real installer |
| Single instance | PASS | real launch creates `com.naipi11.codexbarbar-sim`; second launch focuses existing |
| Four taskbar edges / two displays / 150–200% DPI / keyboard-reader / CUA | NOT COVERED | Cua Drivers unavailable on this host; geometry logic covered by unit tests |

## CUA / proof paths

CUA driver is not installed on this host
(`%LOCALAPPDATA%\Programs\Cua\cua-driver\bin\cua-driver.exe` missing), so
the documented equivalent path was used: Win32 `EnumWindows` /
`GetWindowRect` / pixel screenshots with `CODEXBAR_PROOF_MODE` synthetic
scenarios. Full CUA re-run is required before final 1.0.0.

## Known non-blocking limitations

- Binaries are unsigned; Authenticode signing remains an operator decision.
- The Codex App Server protocol is experimental; tested version is Codex CLI
  0.146.0.
- Legacy provider source directories remain in the repo for history but are
  outside the compiled graph; V1 locale keys no longer contain removed
  surface labels.

## Go / no-go

Automated gates, artifact verification, and host-level Windows/installer
smoke all pass.

**CONDITIONAL GO for 1.0.0-rc.1 as an internal candidate.** Final 1.0.0
requires the clean-machine CUA matrix and disposable-account matrix listed
above; any failure in those matrices flips this report to NO-GO until fixed.

## Resolution for 1.0.0

The 1.0.0 release commit carries the same code surface as this candidate.
The clean-machine CUA matrix and disposable-account matrix remain the
explicit acceptance gates for final publication; the release notes in
[v1.0.0-release-notes.md](./v1.0.0-release-notes.md) state tested versions,
unsigned-build behavior, privacy, and known limitations.
