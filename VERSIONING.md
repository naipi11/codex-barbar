# Versioning Policy

codex-barbar follows [Semantic Versioning 2.0.0](https://semver.org/).

## Version Format

```
MAJOR.MINOR.PATCH[-PRERELEASE]
```

**Examples:** `1.0.0`, `1.2.3`, `2.0.0-beta.1`

---

## Version Increments

### PATCH (x.x.X) — Bug Fixes
Increment for backwards-compatible bug fixes and minor improvements.

**Examples:**
- Fix crash when provider API is unreachable
- Fix incorrect usage percentage calculation
- Fix UI rendering glitch on high-DPI displays
- Rename "Zed AI" to "Zai" (cosmetic fix)
- Update error messages for clarity
- Performance optimizations (no API changes)

**Release:** `1.0.4` → `1.0.5`

---

### MINOR (x.X.0) — New Features
Increment for new features that are backwards-compatible.

**Examples:**
- Add new AI provider (e.g., Amp, JetBrains AI)
- Add new animation type (e.g., Unbraid, Tilt)
- Add new CLI command or flag
- Add new preferences option
- Add new chart visualization
- Add keyboard shortcut support
- Add a supported desktop-platform release target

**Release:** `1.0.4` → `1.1.0`

---

### MAJOR (X.0.0) — Breaking Changes
Increment for incompatible API changes or major rewrites.

**Examples:**
- Change settings file format (breaks existing configs)
- Remove deprecated providers
- Change CLI command syntax
- Change credential storage format
- Major UI redesign
- Minimum Windows version requirement change

**Release:** `1.0.4` → `2.0.0`

---

## Pre-release Versions

Use pre-release tags for testing before stable release:

| Tag | Purpose | Example |
|-----|---------|---------|
| `alpha` | Early development, unstable | `2.0.0-alpha.1` |
| `beta` | Feature complete, testing | `2.0.0-beta.1` |
| `rc` | Release candidate, final testing | `2.0.0-rc.1` |

---

## Release Checklist

### Before Release

1. **Update every product manifest and CHANGELOG.md**, then commit the exact
   candidate. Keep the changelog entry `Unreleased` until all release gates
   have passed.
2. **Validate that checked-out candidate before tagging.** Local release
   scripts must use `-Ref HEAD` (or an explicit ref that resolves to the
   checked-out candidate) because they require exact HEAD/ref equality.
3. **Run Windows and Ubuntu CI** for the exact candidate commit, including
   their artifact verifiers.
4. **Complete the platform acceptance records**. An Ubuntu Debian release
   requires [docs/verification/linux/ubuntu-24.04-acceptance.md](docs/verification/linux/ubuntu-24.04-acceptance.md)
   with no `PENDING`/`NOT RUN` release-blocking item, including the package
   SHA-256 and desktop/session evidence.
5. Only then create an annotated `vX.Y.Z` tag pointing to that exact candidate
   commit and push it to rerun/continue release aggregation. Run release
   doctor on the aggregate set, then inspect the draft assets and obtain
   explicit authorization to publish.

### Creating a Release

```bash
# 1. Commit the versioned candidate, then validate the checked-out commit
git add rust/Cargo.toml apps/desktop-tauri/src-tauri/Cargo.toml apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "chore: bump version to X.Y.Z"
./scripts/windows-release-build.ps1 -Ref HEAD -Version X.Y.Z -OutputDirectory ./artifacts/release

# 2. After exact-commit CI and acceptance records pass, tag that same commit
git tag -a vX.Y.Z -m "vX.Y.Z - Brief description" HEAD

# 3. Push the tag to run/continue dual-platform release aggregation
git push origin main --tags

# 4. After aggregation produces all Windows + Linux assets, run release doctor
./scripts/release-doctor.ps1 -Version X.Y.Z -AssetsDirectory ./artifacts/aggregate-release

# 5. Only after release doctor passes, inspect the draft assets; publish only with explicit authorization
```

The `v*` workflow path resolves the version from the tag; the
`workflow_dispatch` path checks its requested version against the committed
manifests. The hosted workflows deliberately use `pwsh` for PowerShell
policy/release steps, including where the Ubuntu runner supplies PowerShell
Core. These are execution details, not evidence that a Linux desktop
acceptance test ran.

---

## Changelog Format

Follow [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
## [X.Y.Z] — YYYY-MM-DD

### Added
- New features

### Changed
- Changes to existing functionality

### Fixed
- Bug fixes

### Removed
- Removed features

### Security
- Security fixes
```

---

## Version Locations

Update version in these files:

| File | Field |
|------|-------|
| `rust/Cargo.toml` | `version = "X.Y.Z"` |
| `apps/desktop-tauri/src-tauri/Cargo.toml` | `version = "X.Y.Z"` |
| `apps/desktop-tauri/package.json` | `version` |
| `apps/desktop-tauri/src-tauri/tauri.conf.json` | `version` |
| `CHANGELOG.md` | `## X.Y.Z - DATE` after release gates pass |

---

## Quick Reference

| Change Type | Version Bump | Example |
|-------------|--------------|---------|
| Bug fix | PATCH | `1.0.4` → `1.0.5` |
| New provider | MINOR | `1.0.4` → `1.1.0` |
| New feature | MINOR | `1.1.0` → `1.2.0` |
| Breaking change | MAJOR | `1.2.0` → `2.0.0` |
| Config format change | MAJOR | `1.2.0` → `2.0.0` |
