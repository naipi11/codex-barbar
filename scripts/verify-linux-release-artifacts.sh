#!/usr/bin/env bash
# Verify the exact Ubuntu amd64 release asset set, its Debian control fields,
# installed paths, SPDX metadata, manifest, and SHA-256 checksum.
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/verify-linux-release-artifacts.sh --version VERSION --assets DIRECTORY
USAGE
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

version=""
assets=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || die '--version requires a value'
      version="$2"
      shift 2
      ;;
    --assets)
      [[ $# -ge 2 ]] || die '--assets requires a value'
      assets="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1" ;;
  esac
done

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-((alpha|beta|rc)\.[0-9]+))?$ ]] || die "invalid version: $version"
[[ -n "$assets" && -d "$assets" ]] || die '--assets must be an existing directory'
command -v dpkg-deb >/dev/null 2>&1 || die 'dpkg-deb is required to verify Debian release artifacts'
command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required to verify release hashes'
command -v tar >/dev/null 2>&1 || die 'tar is required to inspect Debian package paths'
command -v node >/dev/null 2>&1 || die 'node is required to validate JSON artifacts'

assets="$(cd "$assets" && pwd)"
deb_name="codex-barbar_${version}_amd64.deb"
sbom_name="codex-barbar_${version}_sbom.spdx.json"
expected=("$deb_name" "SHA256SUMS.txt" "$sbom_name" "artifact-manifest.json")

for name in "${expected[@]}"; do
  [[ -f "$assets/$name" ]] || die "missing expected asset: $name"
done

mapfile -t actual < <(find "$assets" -maxdepth 1 -type f -printf '%f\n' | LC_ALL=C sort)
[[ ${#actual[@]} -eq ${#expected[@]} ]] || die "unexpected asset count in $assets"
for name in "${expected[@]}"; do
  [[ " ${actual[*]} " == *" $name "* ]] || die "unexpected release asset set: missing $name"
done

deb="$assets/$deb_name"
[[ "$(dpkg-deb --field "$deb" Package)" == "com.naipi11.codexbarbar" ]] || die 'Debian Package field mismatch'
[[ "$(dpkg-deb --field "$deb" Version)" == "$version" ]] || die 'Debian Version field mismatch'
[[ "$(dpkg-deb --field "$deb" Architecture)" == "amd64" ]] || die 'Debian Architecture field mismatch'
depends="$(dpkg-deb --field "$deb" Depends)"
for dependency in libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1 libsecret-1-0; do
  [[ "$depends" == *"$dependency"* ]] || die "Debian dependency missing: $dependency"
done

mapfile -t package_paths < <(dpkg-deb --fsys-tarfile "$deb" | tar -tf -)
normalized_paths=()
for entry in "${package_paths[@]}"; do
  [[ "$entry" != /* ]] || die "package entry is absolute: $entry"
  relative="${entry#./}"
  [[ -n "$relative" ]] || continue
  [[ "$relative" != /* && "$relative" != *'//' && "/$relative/" != *'/../'* ]] || die "unsafe package path: $entry"
  normalized_paths+=("$relative")
done
for path in \
  'usr/bin/codex-barbar' \
  'usr/share/applications/codex-barbar.desktop' \
  'usr/share/icons/hicolor/1024x1024/apps/codex-barbar.png'; do
  [[ " ${normalized_paths[*]} " == *" $path "* ]] || die "missing expected Debian package entry: $path"
done

sums="$assets/SHA256SUMS.txt"
[[ $(wc -l <"$sums") -eq 1 ]] || die 'SHA256SUMS.txt must contain exactly one entry'
expected_hash="$(awk -v name="$deb_name" '$2 == name && $1 ~ /^[0-9a-f]{64}$/ { print $1 }' "$sums")"
[[ -n "$expected_hash" ]] || die "SHA256SUMS.txt has no valid entry for $deb_name"
actual_hash="$(sha256sum "$deb" | awk '{print $1}')"
[[ "$expected_hash" == "$actual_hash" ]] || die "SHA256 mismatch for $deb_name"

node - "$version" "$deb_name" "$actual_hash" "$assets/$sbom_name" "$assets/artifact-manifest.json" <<'NODE'
const fs = require('fs');
const [version, debName, debHash, sbomPath, manifestPath] = process.argv.slice(2);
const sbom = JSON.parse(fs.readFileSync(sbomPath, 'utf8'));
if (sbom.spdxVersion !== 'SPDX-2.3' || sbom.name !== 'codex-barbar') throw new Error('invalid SPDX document');
const root = (sbom.packages || []).find((entry) => entry.SPDXID === 'SPDXRef-Package-codex-barbar');
if (!root || root.versionInfo !== version || root.licenseDeclared !== 'MIT' || root.licenseConcluded !== 'MIT') throw new Error('invalid SPDX root package');
if (!root.checksums?.some((entry) => entry.algorithm === 'SHA256' && entry.checksumValue === debHash)) throw new Error('SPDX package hash mismatch');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
if (manifest.version !== version || manifest.target !== 'x86_64-unknown-linux-gnu' || manifest.package !== 'com.naipi11.codexbarbar') throw new Error('invalid target manifest');
const asset = (manifest.files || []).find((entry) => entry.name === debName);
if (!asset || asset.sha256 !== debHash || !Number.isInteger(asset.size) || asset.size < 1) throw new Error('invalid target manifest asset');
NODE

printf 'Linux release artifacts verified: %s\n' "$assets"
