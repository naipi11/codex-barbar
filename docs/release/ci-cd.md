# CI and release

## Hosted CI

`.github/workflows/pr-check.yml` runs on the official `windows-2025` runner
for PRs and pushes to `main`/`master`. It runs fmt, clippy and tests on both
Rust crates, frontend tests/build, the V1 boundary scan, production
dependency audit, license audit, and a Tauri x64 production build.

`.github/workflows/release.yml` runs only on `workflow_dispatch` (with a
`version` input) or a pushed `v*` tag. It repeats every PR gate, runs
`windows-release-build.ps1`, verifies artifacts, uploads them to an Actions
artifact, and optionally creates a draft release (gated by `publish_draft`
or a tag event). It uses only `GITHUB_TOKEN`.

`.github/workflows/interaction-guard.yml` handles untrusted issue/PR authors
on `ubuntu-24.04`.

## Local mirror

```powershell
.\scripts\local-check.ps1            # default slice (boundary, rust, frontend)
.\scripts\local-check.ps1 -All       # adds audit, license, Tauri production build
.\scripts\release-doctor.ps1 -Version <version>
```

## Release flow

See [RELEASING.md](../RELEASING.md) for the artifact set, build/verify
commands, smoke requirements, and the explicit authorization gate before any
external publish.
