# CI — codex-barbar

codex-barbar runs on official GitHub-hosted runners. The Windows V1 gates
run on `windows-2025`; the interaction guard runs on `ubuntu-24.04`. There is
no third-party hosted pool and no budget-mode conditional.

## Workflows

### PR check — `.github/workflows/pr-check.yml`

Runs on `pull_request`, on `push` to `main`/`master`, and on
`workflow_dispatch`. Runner: `windows-2025` (official GitHub Windows Server
2025 image with VS Build Tools).

Exact commands run, in order:

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
.\scripts\assert-v1-boundaries.ps1
pnpm --dir apps/desktop-tauri audit --prod --audit-level high
.\scripts\audit-licenses.ps1
pnpm --dir apps/desktop-tauri run tauri:build
```

Tooling is pinned: Node 20, pnpm 10.18.1, stable Rust with `rustfmt` and
`clippy`, and target `x86_64-pc-windows-msvc`. `concurrency.cancel-in-progress`
is on, keyed by ref, so superseded pushes cancel the in-flight run.

### Release — `.github/workflows/release.yml`

Runs on `workflow_dispatch` (requires `version` and an optional boolean
`publish_draft`) or on a pushed `v*` tag. It repeats every PR gate, then:

```powershell
.\scripts\windows-release-build.ps1 -Ref $env:GITHUB_SHA -Version <version> -OutputDirectory .\artifacts\release
.\scripts\verify-release-artifacts.ps1 -Version <version> -AssetsDirectory .\artifacts\release
```

Artifacts are uploaded to an Actions artifact named
`codex-barbar-<version>`. A draft GitHub Release is created only when the
dispatch input `publish_draft` is true or when the run was triggered by a
tag; it uses only the repository `GITHUB_TOKEN`, never a PAT. Release notes
mark the build as unsigned when no Authenticode certificate was supplied.
Winget submission is a separate, manual step outside this workflow.

The Dependabot release gate uses the repository secret
`DEPENDABOT_ALERTS_TOKEN`, not `GITHUB_TOKEN`: GitHub's automatic Actions
token cannot reliably read the Dependabot alerts REST endpoint. Configure it
as a fine-grained token restricted to this repository with **Dependabot
alerts: Read** permission, then rotate it using `gh secret set
DEPENDABOT_ALERTS_TOKEN -R naipi11/codex-barbar`. A missing or unreadable
secret fails the release before artifacts are built; there is no release-time
bypass for that security gate.

### Interaction guard — `.github/workflows/interaction-guard.yml`

Runs on `ubuntu-24.04` for untrusted issue/PR authors. Permissions are
unchanged (`contents: read`, `issues: write`, `pull-requests: write`).

## Local mirror

`.\scripts\local-check.ps1` mirrors the PR gate. `-All` adds the production
dependency audit and the Tauri x64 production build; packaging, installer
smoke, and uploads remain explicit release steps.
