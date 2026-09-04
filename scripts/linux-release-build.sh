#!/usr/bin/env bash
# Stage a pre-built Ubuntu amd64 Debian package with deterministic release
# metadata. This script deliberately does not run a Tauri build: a missing
# bundle is an error, never a successful empty release.
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/linux-release-build.sh --version VERSION --output DIRECTORY

Stages codex-barbar_VERSION_amd64.deb, SHA256SUMS.txt, an SPDX 2.3 SBOM, and
artifact-manifest.json from the exact Tauri Debian bundle already built under
$CARGO_TARGET_DIR (or target/ when it is unset).
USAGE
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

version=""
output=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || die '--version requires a value'
      version="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || die '--output requires a value'
      output="$2"
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
[[ -n "$output" ]] || die '--output is required'

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_root="${CARGO_TARGET_DIR:-$repo_root/target}"
deb_name="codex-barbar_${version}_amd64.deb"
deb_source="$target_root/release/bundle/deb/$deb_name"

[[ -f "$deb_source" ]] || die "Missing Debian bundle: $deb_source. Run pnpm --dir apps/desktop-tauri run tauri:build:linux first."
command -v dpkg-deb >/dev/null 2>&1 || die 'dpkg-deb is required to stage a Debian release'
command -v node >/dev/null 2>&1 || die 'node is required to write the SPDX SBOM and manifest'
command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required to stage a Debian release'
[[ "$(dpkg-deb --field "$deb_source" Package)" == "codex-barbar" ]] || die 'Debian bundle Package field is not codex-barbar'
[[ "$(dpkg-deb --field "$deb_source" Version)" == "$version" ]] || die "Debian bundle Version does not match $version"
[[ "$(dpkg-deb --field "$deb_source" Architecture)" == "amd64" ]] || die 'Debian bundle Architecture is not amd64'

mkdir -p "$output"
output="$(cd "$output" && pwd)"
deb_asset="$output/$deb_name"
sbom_asset="$output/codex-barbar_${version}_sbom.spdx.json"
manifest_asset="$output/artifact-manifest.json"
sums_asset="$output/SHA256SUMS.txt"

# Only replace the four assets this script owns. It does not clean the output
# directory, so unrelated files remain visible to the verifier instead of
# being silently discarded.
rm -f "$deb_asset" "$sbom_asset" "$manifest_asset" "$sums_asset"
cp "$deb_source" "$deb_asset"
deb_hash="$(sha256sum "$deb_asset" | awk '{print $1}')"
printf '%s  %s\n' "$deb_hash" "$deb_name" >"$sums_asset"

commit="$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf '%040d' 0)"
commit_date="$(git -C "$repo_root" show -s --format=%cI HEAD 2>/dev/null || printf '1970-01-01T00:00:00Z')"

node - "$version" "$commit" "$commit_date" "$deb_name" "$deb_hash" "$deb_asset" "$sbom_asset" "$manifest_asset" <<'NODE'
const fs = require('fs');
const [version, commit, created, debName, debHash, debPath, sbomPath, manifestPath] = process.argv.slice(2);
const rootId = 'SPDXRef-Package-codex-barbar';
const sbom = {
  spdxVersion: 'SPDX-2.3',
  dataLicense: 'CC0-1.0',
  SPDXID: 'SPDXRef-DOCUMENT',
  name: 'codex-barbar',
  documentNamespace: `https://github.com/naipi11/codex-barbar/sbom/${version}/${commit}`,
  creationInfo: { created, creators: ['Tool: codex-barbar-linux-release'] },
  packages: [{
    name: 'codex-barbar', SPDXID: rootId, versionInfo: version,
    downloadLocation: 'NOASSERTION', filesAnalyzed: false,
    licenseConcluded: 'MIT', licenseDeclared: 'MIT', copyrightText: 'NOASSERTION',
    checksums: [{ algorithm: 'SHA256', checksumValue: debHash }]
  }]
};
const manifest = {
  version,
  commit,
  target: 'x86_64-unknown-linux-gnu',
  package: 'codex-barbar',
  files: [{ name: debName, size: fs.statSync(debPath).size, sha256: debHash }]
};
fs.writeFileSync(sbomPath, `${JSON.stringify(sbom, null, 2)}\n`);
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
NODE

printf 'Staged Linux release assets in %s\n' "$output"
