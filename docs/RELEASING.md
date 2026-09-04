# Releasing

codex-barbar releases are built on native Windows 11 x64 and Ubuntu 24.04
amd64, then published as GitHub Release assets. Nothing is pushed or published
without explicit user authorization. A Debian package is not release-ready
until both platform CI jobs pass and the Ubuntu desktop acceptance record is
complete for the exact candidate commit.

## Artifacts

Every release produces exactly these files:

```text
codex-barbar_<version>_x64-setup.exe      NSIS per-user installer
codex-barbar_<version>_x64-portable.zip   Portable ZIP (same exe + docs)
codex-barbar_<version>_amd64.deb          Ubuntu 24.04 Debian package
SHA256SUMS.txt                            Aggregate hashes for Windows + Debian payloads
codex-barbar_<version>_windows-sbom.spdx.json  Windows SPDX 2.3 SBOM
codex-barbar_<version>_linux-sbom.spdx.json    Linux SPDX 2.3 SBOM
artifact-manifest-windows.json            Windows target manifest
artifact-manifest-linux.json              Linux target manifest
```

The aggregate step does not create an aggregate `artifact-manifest.json`; it
copies the two renamed target manifests above and writes the combined
`SHA256SUMS.txt`.

Binaries are unsigned until an Authenticode certificate is supplied;
SmartScreen warnings are expected.

## Local release build

1. Commit the versioned candidate and ensure all product manifests match its
   version:
   `rust/Cargo.toml`, `apps/desktop-tauri/src-tauri/Cargo.toml`,
   `apps/desktop-tauri/package.json`, and
   `apps/desktop-tauri/src-tauri/tauri.conf.json`.
2. Check out that exact candidate commit. Before a tag exists, build and
   verify the Windows artifacts from its clean worktree with `HEAD`:

   ```powershell
   .\scripts\windows-release-build.ps1 -Ref HEAD -Version <version> -OutputDirectory .\artifacts\release
   .\scripts\verify-release-artifacts.ps1 -Version <version> -AssetsDirectory .\artifacts\release
   .\scripts\release-doctor.ps1 -Version <version> -AssetsDirectory .\artifacts\release
   ```

3. Run the installer and portable smoke tests on a clean machine (or a
   dedicated VM), then record evidence in `docs/WINDOWS_ACCEPTANCE.md`.
4. Build and verify the Ubuntu artifacts on native Ubuntu 24.04 amd64:

   ```bash
   export TAURI_LINUX_AYATANA_APPINDICATOR=1
   corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:build:linux
   bash scripts/linux-release-build.sh --version <version> --output artifacts/linux-release
   bash scripts/verify-linux-release-artifacts.sh --version <version> --assets artifacts/linux-release
   ```

5. Install the exact `codex-barbar_<version>_amd64.deb` in real Ubuntu
   GNOME Wayland and GNOME X11 when available, and record KDE best-effort
   results when available. Complete
   [LINUX_ACCEPTANCE.md](./LINUX_ACCEPTANCE.md) and
   [verification/linux/ubuntu-24.04-acceptance.md](./verification/linux/ubuntu-24.04-acceptance.md),
   including the package SHA-256, tray/panel/settings/Current CLI refresh,
   float-ball, notification, XDG autostart, Secret Service, and unsupported
   taskbar behavior.
6. Wait for green Windows and Ubuntu CI for the exact candidate commit. After
   both CI jobs and both platform acceptance records are complete, create an
   annotated `v<version>` tag that points to that same candidate commit, then
   push the tag to rerun/continue release aggregation. Inspect the resulting
   draft assets and request authorization before publishing.

The build script refuses to run on a dirty worktree or when `-Ref` does not
resolve to HEAD. `-AllowDirty` exists only for development.

## Hosted release workflow

`.github/workflows/release.yml` has `windows-build`, `linux-build`, and
`publish` jobs. `publish` requires both build jobs, validates that they
resolved the same version, aggregates the target artifacts, and uploads a
draft only when `publish_draft` is true on `workflow_dispatch` or when
triggered by a `v*` tag; it uses only the repository `GITHUB_TOKEN`. The
workflow deliberately uses `pwsh` for PowerShell policy/release steps,
including where the Ubuntu runner provides PowerShell Core. A green build job
does not replace real GNOME/KDE/Wayland/X11/D-Bus/Secret Service acceptance.

For a `v*` invocation, the workflow resolves the version from the tag and
rejects a tag that does not start with `v`. For `workflow_dispatch`, provide
the requested version that matches the committed product manifests; dispatch
does not make an unaccepted Ubuntu desktop result pass.

Before a public release, the workflow queries enabled Dependabot alerts and
fails on open high/critical findings. If Dependabot/security-events read
access is not enabled, the release job fails with a clear external-gate
message. This is not a RustSec audit claim; RustSec tooling is not installed
without explicit approval.

## After release

- Distribution through the Windows package manager is a separate, manual
  step outside the release workflow. Every version needs its own immutable
  manifest folder with matching installer URL and SHA-256.
- Ubuntu release publication is blocked while any row in
  `docs/verification/linux/ubuntu-24.04-acceptance.md` remains `PENDING` or
  `NOT RUN`; do not claim a package hash, tag, or release before that record
  and both platform CI jobs are complete.
- Do not push, create a GitHub Release, or contact upstream repositories
  without explicit user authorization.
