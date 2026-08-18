# Releasing

codex-barbar releases are built on native Windows 11 x64 and published as
GitHub Release assets. Nothing is pushed or published without explicit user
authorization.

## Artifacts

Every release produces exactly these files:

```text
codex-barbar_<version>_x64-setup.exe      NSIS per-user installer
codex-barbar_<version>_x64-portable.zip   Portable ZIP (same exe + docs)
SHA256SUMS.txt                            Hashes for setup and ZIP
codex-barbar_<version>_sbom.spdx.json     Deterministic SPDX 2.3 SBOM
artifact-manifest.json                    Version, commit, sizes, hashes
```

Binaries are unsigned until an Authenticode certificate is supplied;
SmartScreen warnings are expected.

## Local release build

1. Ensure all product manifests match the version:
   `rust/Cargo.toml`, `apps/desktop-tauri/src-tauri/Cargo.toml`,
   `apps/desktop-tauri/package.json`, and
   `apps/desktop-tauri/src-tauri/tauri.conf.json`.
2. Commit and tag: `git tag -a v<version> -m "codex-barbar <version>"`.
3. Build and verify from a clean worktree:

   ```powershell
   .\scripts\windows-release-build.ps1 -Ref v<version> -Version <version> -OutputDirectory .\artifacts\release
   .\scripts\verify-release-artifacts.ps1 -Version <version> -AssetsDirectory .\artifacts\release
   .\scripts\release-doctor.ps1 -Version <version> -AssetsDirectory .\artifacts\release
   ```

4. Run the installer and portable smoke tests on a clean machine (or a
   dedicated VM), then record evidence in `docs/WINDOWS_ACCEPTANCE.md`.

The build script refuses to run on a dirty worktree or when `-Ref` does not
resolve to HEAD. `-AllowDirty` exists only for development.

## Hosted release workflow

`.github/workflows/release.yml` repeats every PR gate, runs the release
build, verifies artifacts, and uploads them as an Actions artifact. A draft
GitHub Release is created only when `publish_draft` is true on
`workflow_dispatch` or when triggered by a `v*` tag; it uses only the
repository `GITHUB_TOKEN`.

Before a public release, the workflow queries enabled Dependabot alerts and
fails on open high/critical findings. If Dependabot/security-events read
access is not enabled, the release job fails with a clear external-gate
message. This is not a RustSec audit claim; RustSec tooling is not installed
without explicit approval.

## After release

- Distribution through the Windows package manager is a separate, manual
  step outside the release workflow. Every version needs its own immutable
  manifest folder with matching installer URL and SHA-256.
- Do not push, create a GitHub Release, or contact upstream repositories
  without explicit user authorization.
