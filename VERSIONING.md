# Versioning Policy

Win-CodexBar follows [Semantic Versioning 2.0.0](https://semver.org/).

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

1. **Update version** in `rust/Cargo.toml`
2. **Update CHANGELOG.md** with release notes
3. **Run tests**: `cargo test`
4. **Build release**: `cargo build --release`
5. **Test binary** manually

### Creating a Release

```bash
# 1. Commit version bump
git add rust/Cargo.toml rust/CHANGELOG.md
git commit -m "chore: bump version to X.Y.Z"

# 2. Create annotated tag
git tag -a vX.Y.Z -m "vX.Y.Z - Brief description"

# 3. Push to remote
git push origin main --tags

# 4. Create GitHub release with binary
gh release create vX.Y.Z \
  target/release/codexbar-desktop-tauri.exe \
  --title "vX.Y.Z - Release Title" \
  --notes-file release-notes.md
```

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
| `rust/CHANGELOG.md` | `## X.Y.Z — DATE` |

---

## Quick Reference

| Change Type | Version Bump | Example |
|-------------|--------------|---------|
| Bug fix | PATCH | `1.0.4` → `1.0.5` |
| New provider | MINOR | `1.0.4` → `1.1.0` |
| New feature | MINOR | `1.1.0` → `1.2.0` |
| Breaking change | MAJOR | `1.2.0` → `2.0.0` |
| Config format change | MAJOR | `1.2.0` → `2.0.0` |
