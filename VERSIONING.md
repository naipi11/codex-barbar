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

1. **Update every product manifest** to the exact version.
2. **Update CHANGELOG.md** with release notes; label it `Unreleased` until
   all release gates have passed.
3. **Run Windows and Ubuntu CI** for the exact commit, including their
   artifact verifiers.
4. **Complete the platform acceptance records**. An Ubuntu Debian release
   requires [docs/verification/linux/ubuntu-24.04-acceptance.md](docs/verification/linux/ubuntu-24.04-acceptance.md)
   with no `PENDING`/`NOT RUN` release-blocking item, including the package
   SHA-256 and desktop/session evidence.
5. Only then commit the version bump, tag the exact commit, inspect the draft
   assets, and obtain explicit authorization to publish.

### Creating a Release

```bash
# 1. Commit the already-validated version bump
git add rust/Cargo.toml apps/desktop-tauri/src-tauri/Cargo.toml apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "chore: bump version to X.Y.Z"

# 2. Create annotated tag
git tag -a vX.Y.Z -m "vX.Y.Z - Brief description"

# 3. Push the validated tag to run the dual-platform workflow
git push origin main --tags

# 4. Inspect the draft assets; publish only with explicit authorization
```

The hosted workflows deliberately use `pwsh` for PowerShell policy/release
steps, including where the Ubuntu runner supplies PowerShell Core. This is an
execution detail, not evidence that a Linux desktop acceptance test ran.

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
