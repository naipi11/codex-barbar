# Task 8 report: Ubuntu Debian packaging

## Implemented

- Added `apps/desktop-tauri/src-tauri/tauri.linux.conf.json`, an Ubuntu Debian
  overlay with the package identifier `com.naipi11.codexbarbar`, `deb` as the
  only target, the Utility category, MIT license metadata, repository metadata,
  a generated Tauri desktop entry and icon, and runtime dependencies for
  WebKitGTK 4.1, GTK 3, Ayatana AppIndicator, and Secret Service.
- Kept the existing `tauri:build` NSIS command byte-for-byte and added explicit
  `tauri:build:windows` and `tauri:build:linux` commands. The Linux command is
  `tauri build --config src-tauri/tauri.linux.conf.json --bundles deb`.
- Added executable Linux scripts for deterministic staging and verification.
  Staging requires an already-built exact-version amd64 Debian package, writes
  the `.deb`, one-entry `SHA256SUMS.txt`, SPDX 2.3 JSON, and target manifest,
  and exits before claiming success if the bundle is absent. Verification checks
  asset set, package id/version/amd64 metadata, runtime dependencies, generated
  executable/desktop/icon paths, relative non-traversing package paths, hashes,
  SPDX MIT metadata, and target manifest.
- Added `scripts/linux-release-build.test.sh`. It covers the missing-bundle
  failure path, configuration metadata, shell syntax, and (on Ubuntu/Debian)
  builds a fixture package with `dpkg-deb`, then stages and verifies it. The
  fixed user autostart filename remains
  `com.naipi11.codexbarbar.desktop`; it is distinct from Tauri's generated
  installed application desktop entry `codex-barbar.desktop`.
- Updated the desktop crate metadata to be platform-neutral and declare MIT;
  no Rust dependency was added or changed.

## RED then GREEN evidence

- RED: before implementation, the equivalent presence checks confirmed that
  `tauri.linux.conf.json`, `linux-release-build.sh`, and
  `verify-linux-release-artifacts.sh` were absent. The requested `bash` command
  first failed because the Windows `bash.exe` points to a WSL installation with
  no Linux distribution. Git Bash was then used for the GREEN static test run.
- GREEN: `C:\Program Files\Git\bin\bash.exe scripts/linux-release-build.test.sh`
  passed configuration and syntax checks, verified that an absent bundle fails
  closed, and printed `[skip] dpkg-deb integration checks require Ubuntu/Debian`.

## Verification completed on Windows

- `corepack pnpm@10.18.1 --dir apps/desktop-tauri test` — 39 files, 300 tests
  passed.
- `corepack pnpm@10.18.1 --dir apps/desktop-tauri run build` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` —
  passed.
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings`
  — passed.
- `cargo test --manifest-path rust/Cargo.toml` — 522 passed, 1 ignored.
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` — 260
  passed.
- `git diff --check` — passed.

## Not performed: Ubuntu-only build/package proof

This Windows host has neither a runnable WSL distribution nor `dpkg-deb`, and
cannot build or inspect a Linux Tauri Debian package. No `.deb` was produced,
so there is intentionally no release hash to report. Run the following on
Ubuntu 24.04 amd64 CI before treating the package as verified:

```bash
sudo apt-get update
sudo apt-get install -y curl libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev patchelf libfuse2 file build-essential jq dpkg-dev
export TAURI_LINUX_AYATANA_APPINDICATOR=1
corepack pnpm@10.18.1 --dir apps/desktop-tauri install --frozen-lockfile
corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:build:linux
bash scripts/linux-release-build.test.sh
bash scripts/linux-release-build.sh --version 1.1.0 --output artifacts/linux-release
bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release
sha256sum artifacts/linux-release/codex-barbar_1.1.0_amd64.deb
```

## Fix round 1: review corrections

- The reverse-DNS Tauri `identifier` remains `com.naipi11.codexbarbar`, but the
  Debian bundler derives the Debian control `Package` field from the active
  `productName`. The active product name is `codex-barbar`, so staging,
  verification, the target manifest, and the fixture now all require
  `Package: codex-barbar` while retaining the exact
  `codex-barbar_<version>_amd64.deb` asset name. This removes the old
  hand-authored fixture mismatch that would have hidden a real Tauri failure.
- Synchronized the active core Cargo manifest, desktop Cargo manifest,
  frontend package manifest, base Tauri config, and both root Cargo.lock local
  package entries to release version `1.1.0`. The shell regression now rejects
  any mismatch before host-specific package tooling is required.
- The verifier now compares target-manifest `files[0].size` to the actual
  staged Debian archive byte size, in addition to its SHA-256 check.
- Re-ran the Git Bash shell regression after the correction; static checks
  passed and the `dpkg-deb` fixture segment remained correctly skipped on this
  non-Debian host. Ubuntu package-build evidence remains unperformed as above.

## Fix round 2: generated desktop-entry fixture

- Corrected the `dpkg-deb` fixture application entry to
  `usr/share/applications/codex-barbar.desktop`, matching the Tauri Debian
  bundler's product-name-derived output and the verifier. A static fixture-path
  assertion now runs before the Windows `dpkg-deb` skip, so this mismatch cannot
  be hidden by the host limitation again.
- Left the separate XDG autostart contract unchanged:
  `~/.config/autostart/com.naipi11.codexbarbar.desktop` remains the sole
  user-owned autostart filename.
- Re-ran shell syntax/static tests, 300 frontend tests, the frontend build,
  desktop Rust tests (260), and formatting check; all completed successfully.
