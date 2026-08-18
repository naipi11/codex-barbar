# Building from Source

## Prerequisites

- Windows 11 x64 host (native Windows is required for tray, DPAPI, and
  installer behavior)
- Rust stable (edition 2024) with the `x86_64-pc-windows-msvc` target,
  `rustfmt`, and `clippy`
- Microsoft Visual Studio Build Tools with the **Desktop development with
  C++** workload
- Node.js 20 and pnpm 10.18.1 (`corepack enable`)

## Frontend dependencies

```powershell
corepack pnpm@10.18.1 --dir apps/desktop-tauri install --frozen-lockfile
```

Use the hoisted node_modules layout on hardened hosts:
`--config.node-linker=hoisted`.

## Build the desktop app

```powershell
corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:build
```

Release binary: `target/release/codex-barbar.exe`; NSIS installer:
`target/release/bundle/nsis/codex-barbar_<version>_x64-setup.exe`.

Debug build (faster, no optimization):

```powershell
corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:build:debug
```

## Build the CLI

```powershell
cargo build -p codexbar --release
# Binary at target/release/codexbar.exe
```

## Dev mode (hot reload)

```powershell
.\scripts\dev.ps1            # default debug build + launch
.\scripts\dev.ps1 -Release   # optimized build
.\scripts\dev.ps1 -SkipBuild # run the last build without rebuilding
```

Or directly: `corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:dev`.

## Tests and checks

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
corepack pnpm@10.18.1 --dir apps/desktop-tauri test
corepack pnpm@10.18.1 --dir apps/desktop-tauri run build
```

The local CI mirror is `.\scripts\local-check.ps1`; `-All` also runs the
production dependency audit, license audit, and Tauri x64 production build.

## Release build

```powershell
.\scripts\windows-release-build.ps1 -Ref HEAD -Version 1.0.0 -OutputDirectory .\artifacts\release
.\scripts\verify-release-artifacts.ps1 -Version 1.0.0 -AssetsDirectory .\artifacts\release
```

The build script requires a clean worktree and exact HEAD/ref equality
(`-AllowDirty` is development-only), checks every product manifest version,
runs frozen install/tests/build, then stages the NSIS setup, portable ZIP,
`SHA256SUMS.txt`, SPDX SBOM, and artifact manifest. See
[RELEASING.md](./RELEASING.md) for the full flow.

## Project structure

```text
apps/desktop-tauri/            Tauri desktop shell
  src/                         React frontend (TypeScript)
  src-tauri/                   Tauri/Rust backend (main, tray, commands)
rust/                          Shared backend crate + CLI
  src/core/                    ProviderId, Provider trait, factory
  src/providers/codex/         Codex App Server client (V1 provider)
  src/accounts/                SQLite store, DPAPI vault, managed runtimes
  src/platform/windows/        Autostart, purge, locale
  src/tray/                    Tray icon rendering
  src/cli/                     CLI subcommands
docs/                          Documentation
scripts/                       Dev/release helper scripts
```

## Documentation map

| Doc | Contents |
|-----|----------|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Modules, entry points, data flow |
| [RELEASING.md](./RELEASING.md) | Release workflow and artifact verification |
| [WINDOWS_PROOF.md](./WINDOWS_PROOF.md) | Windows proof checklist |
| [TESTED_CODEX_VERSIONS.md](./TESTED_CODEX_VERSIONS.md) | Tested Codex versions |
| [../AGENTS.md](../AGENTS.md) | Agent/contributor guidelines |

Upstream macOS documentation is a read-only concept source; do not copy
Swift/Keychain/Sparkle instructions here without a Windows rewrite.
