# Building from Source

## Prerequisites

### Windows 11 x64

- Windows 11 x64 host (native Windows is required for tray, DPAPI, and
  installer behavior)
- Rust stable (edition 2024) with the `x86_64-pc-windows-msvc` target,
  `rustfmt`, and `clippy`
- Microsoft Visual Studio Build Tools with the **Desktop development with
  C++** workload
- Node.js 20 and pnpm 10.18.1 (`corepack enable`)

### Ubuntu 24.04 amd64

Ubuntu builds must run on native Ubuntu 24.04 amd64. Install the desktop and
Debian packaging prerequisites before building:

```bash
sudo apt-get update
sudo apt-get install -y \
  curl \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  patchelf \
  libfuse2 \
  file \
  build-essential \
  jq \
  dpkg-dev
```

Use Rust stable with `rustfmt` and `clippy`, Node.js 20, and pnpm 10.18.1.
`libwebkit2gtk-4.1-dev`, GTK 3, and Ayatana AppIndicator are required for the
Tauri tray build; the Debian package declares the matching runtime libraries
plus `libsecret-1-0` for Secret Service credentials.

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

## Build the Ubuntu Debian package

```bash
export TAURI_LINUX_AYATANA_APPINDICATOR=1
corepack pnpm@10.18.1 --dir apps/desktop-tauri install --frozen-lockfile
corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:build:linux
bash scripts/linux-release-build.sh --version 1.1.0 --output artifacts/linux-release
bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release
```

The staged package is exactly `codex-barbar_1.1.0_amd64.deb`; its companion
assets are `SHA256SUMS.txt`, `codex-barbar_1.1.0_sbom.spdx.json`, and
`artifact-manifest.json`. The package hash is release evidence only after a
native Ubuntu build and verification. This Windows host cannot provide it.

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

On Ubuntu, run the same Rust and frontend commands with Bash paths, then the
Debian artifact verifier shown above. `pnpm run tauri:build:linux` and
`dpkg-deb` validate packaging, not GNOME/KDE/Wayland runtime behavior; record
that separately in [LINUX_ACCEPTANCE.md](./LINUX_ACCEPTANCE.md).

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
  src/platform/windows/        Windows autostart, purge, locale
  src/platform/linux/          XDG autostart, Linux locale
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
| [LINUX_ACCEPTANCE.md](./LINUX_ACCEPTANCE.md) | Ubuntu package and desktop acceptance checklist |
| [TESTED_CODEX_VERSIONS.md](./TESTED_CODEX_VERSIONS.md) | Tested Codex versions |
| [../AGENTS.md](../AGENTS.md) | Agent/contributor guidelines |

Upstream macOS documentation is a read-only concept source; do not copy
Swift/Keychain/Sparkle instructions here without a Windows rewrite.
