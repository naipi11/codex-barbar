#!/usr/bin/env bash
# Regression checks for the Ubuntu Debian release scripts. Run on Ubuntu; the
# dpkg-deb integration checks are deliberately skipped when dpkg-deb is absent.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

require_file() {
  if [[ ! -f "$1" ]]; then
    printf 'missing required file: %s\n' "$1" >&2
    exit 1
  fi
}

# RED checks for Task 8. Keep these first so a missing Linux release surface is
# reported before any host-specific tooling is required.
require_file apps/desktop-tauri/src-tauri/tauri.linux.conf.json
require_file scripts/linux-release-build.sh
require_file scripts/verify-linux-release-artifacts.sh
grep -q 'const DESKTOP_FILE_NAME: &str = "com.naipi11.codexbarbar.desktop"' rust/src/platform/linux/autostart.rs

node <<'NODE'
const fs = require('fs');
const config = JSON.parse(fs.readFileSync('apps/desktop-tauri/src-tauri/tauri.linux.conf.json', 'utf8'));
const baseConfig = JSON.parse(fs.readFileSync('apps/desktop-tauri/src-tauri/tauri.conf.json', 'utf8'));
const packageJson = JSON.parse(fs.readFileSync('apps/desktop-tauri/package.json', 'utf8'));
const bundle = config.bundle || {};
if (!Array.isArray(bundle.targets) || bundle.targets[0] !== 'deb') throw new Error('Linux bundle target must be deb');
if (config.identifier !== 'com.naipi11.codexbarbar') throw new Error('Linux package identifier mismatch');
if (baseConfig.productName !== 'codex-barbar') throw new Error('Tauri Debian Package must derive from productName codex-barbar');
if (bundle.category !== 'Utility' || bundle.license !== 'MIT') throw new Error('Linux package metadata mismatch');
const depends = (((bundle.linux || {}).deb || {}).depends || []).join('\n');
for (const dependency of ['libwebkit2gtk-4.1-0', 'libgtk-3-0', 'libayatana-appindicator3-1', 'libsecret-1-0']) {
  if (!depends.includes(dependency)) throw new Error(`missing runtime dependency: ${dependency}`);
}
const releaseVersion = '1.1.0';
const cargoVersions = [
  fs.readFileSync('rust/Cargo.toml', 'utf8'),
  fs.readFileSync('apps/desktop-tauri/src-tauri/Cargo.toml', 'utf8')
].map((text) => /^version = "([^"]+)"/m.exec(text)?.[1]);
if (baseConfig.version !== releaseVersion || packageJson.version !== releaseVersion || cargoVersions.some((version) => version !== releaseVersion)) {
  throw new Error(`release manifests must all use ${releaseVersion}`);
}
const lock = fs.readFileSync('Cargo.lock', 'utf8');
for (const name of ['codexbar', 'codex-barbar-desktop']) {
  if (!new RegExp(`name = "${name}"\\nversion = "${releaseVersion}"`).test(lock)) {
    throw new Error(`Cargo.lock package version mismatch: ${name}`);
  }
}
for (const script of ['scripts/linux-release-build.sh', 'scripts/verify-linux-release-artifacts.sh']) {
  if (!fs.readFileSync(script, 'utf8').includes('Package)" == "codex-barbar"')) {
    throw new Error(`${script} must validate Tauri's codex-barbar Debian Package field`);
  }
}
const fixtureScript = fs.readFileSync('scripts/linux-release-build.test.sh', 'utf8');
const fixtureDesktop = fixtureScript.match(/\$fixture\/usr\/share\/applications\/([^"\n]+)/)?.[1];
if (fixtureDesktop !== 'codex-barbar.desktop') {
  throw new Error('dpkg-deb fixture must use Tauri generated codex-barbar.desktop');
}
NODE

grep -Eq 'codex-barbar_.*_amd64\.deb' scripts/verify-linux-release-artifacts.sh
bash -n scripts/linux-release-build.sh scripts/verify-linux-release-artifacts.sh scripts/linux-release-build.test.sh

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if bash scripts/linux-release-build.sh --version 1.1.0 --output "$tmp/missing" >"$tmp/missing.log" 2>&1; then
  printf 'staging unexpectedly succeeded without a Debian package\n' >&2
  exit 1
fi
grep -q 'Missing Debian bundle' "$tmp/missing.log"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  printf '[skip] dpkg-deb integration checks require Ubuntu/Debian\n'
  printf 'linux release script static tests passed\n'
  exit 0
fi

fixture="$tmp/fixture"
mkdir -p "$fixture/DEBIAN" "$fixture/usr/bin" \
  "$fixture/usr/share/applications" \
  "$fixture/usr/share/icons/hicolor/1024x1024/apps"
cat >"$fixture/DEBIAN/control" <<'CONTROL'
Package: codex-barbar
Version: 1.1.0
Architecture: amd64
Maintainer: codex-barbar
Description: fixture
Depends: libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1, libsecret-1-0
CONTROL
printf '#!/bin/sh\nexit 0\n' >"$fixture/usr/bin/codex-barbar"
chmod 0755 "$fixture/usr/bin/codex-barbar"
printf '[Desktop Entry]\nType=Application\nName=codex-barbar\n' >"$fixture/usr/share/applications/codex-barbar.desktop"
printf 'fixture icon\n' >"$fixture/usr/share/icons/hicolor/1024x1024/apps/codex-barbar.png"

mkdir -p "$tmp/target/release/bundle/deb"
dpkg-deb --build "$fixture" "$tmp/target/release/bundle/deb/codex-barbar_1.1.0_amd64.deb" >/dev/null
CARGO_TARGET_DIR="$tmp/target" bash scripts/linux-release-build.sh --version 1.1.0 --output "$tmp/assets"
bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets "$tmp/assets"

printf 'linux release script tests passed\n'
