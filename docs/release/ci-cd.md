# CI and release

## Hosted CI

`.github/workflows/pr-check.yml` runs the existing Windows V1 gate on the
official `windows-2025` runner and a `linux-check` job on `ubuntu-24.04` for
PRs and pushes to `main`/`master`. The Ubuntu job installs the documented
WebKitGTK/GTK/AppIndicator, packaging, and build dependencies; runs the Rust
and frontend checks; then builds, stages, and verifies the amd64 Debian
package. Its package control name and installed application launcher are
`codex-barbar` / `codex-barbar.desktop`; this is distinct from the XDG user
autostart filename `com.naipi11.codexbarbar.desktop`.

`.github/workflows/release.yml` runs only on `workflow_dispatch` (with a
`version` input) or a pushed `v*` tag. It resolves the version independently
in `windows-build` and `linux-build`, preserving the tag and manifest checks.
The Windows job retains the NSIS build/staging path; the Ubuntu job builds,
stages, and verifies the amd64 `.deb`. Each uploads a separate Actions
artifact group. `publish` needs both builds, downloads both groups, runs the
high/critical Dependabot gate once with `DEPENDABOT_ALERTS_TOKEN`, validates
and aggregates their manifests/SBOMs with
`aggregate-release-assets.mjs`. Both target manifests must name the exact
workflow commit. Before publishing, the remote `v<version>` tag is fetched,
dereferenced, and required to resolve to that same commit. `publish` is the
only job that can issue one draft `gh release create` command (on a tag or
when `publish_draft` is selected).

The final draft contains the Windows setup EXE and portable ZIP, the Linux
`codex-barbar_<version>_amd64.deb`, per-target SBOMs/manifests, and one
aggregate `SHA256SUMS.txt`. Target artifact groups each retain their own
checksum file only while moving through Actions; those files are not released
alongside the aggregate checksum list.

`.github/workflows/interaction-guard.yml` handles untrusted issue/PR authors
on `ubuntu-24.04`.

## Local mirror

```powershell
.\scripts\local-check.ps1            # default slice (boundary, rust, frontend)
.\scripts\local-check.ps1 -All       # adds audit, license, Tauri production build
.\scripts\release-doctor.ps1 -Version <version>
node .\scripts\aggregate-release-assets.test.mjs
```

Windows local checks cannot validate the Ubuntu build, Debian package, or
WebKitGTK runtime. The hosted `ubuntu-24.04` jobs remain the required release
evidence for those paths.

## Release flow

See [RELEASING.md](../RELEASING.md) for the artifact set, build/verify
commands, smoke requirements, and the explicit authorization gate before any
external publish.
