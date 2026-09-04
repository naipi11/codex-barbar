#!/usr/bin/env node
import { createHash } from "node:crypto";
import { cp, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const TARGETS = {
  windows: "x86_64-pc-windows-msvc",
  linux: "x86_64-unknown-linux-gnu",
};

function fail(message) {
  throw new Error(message);
}

function assertSafeFileName(name, label) {
  if (typeof name !== "string" || !name || basename(name) !== name) {
    fail(`${label} must be a file name`);
  }
}

function parseChecksums(text, label) {
  const entries = new Map();
  for (const line of text.trim().split(/\r?\n/)) {
    const match = /^([a-f0-9]{64})\s{2}(.+)$/.exec(line);
    if (!match) fail(`${label} SHA256SUMS.txt has an invalid entry`);
    const [, hash, name] = match;
    assertSafeFileName(name, `${label} SHA256SUMS entry`);
    if (entries.has(name)) fail(`${label} SHA256SUMS.txt has a duplicate entry for ${name}`);
    entries.set(name, hash);
  }
  if (entries.size === 0) fail(`${label} SHA256SUMS.txt is empty`);
  return entries;
}

async function readJson(path, label) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
}

async function sha256File(path) {
  return createHash("sha256").update(await readFile(path)).digest("hex");
}

async function loadTarget({ label, directory, version }) {
  const manifestPath = join(directory, "artifact-manifest.json");
  const manifest = await readJson(manifestPath, `${label} target manifest`);
  if (manifest.version !== version) {
    fail(`${label} target manifest version is ${manifest.version}, expected ${version}`);
  }
  if (manifest.target !== TARGETS[label]) {
    fail(`${label} target manifest target is ${manifest.target}, expected ${TARGETS[label]}`);
  }
  if (!Array.isArray(manifest.files) || manifest.files.length === 0) {
    fail(`${label} target manifest has no files`);
  }

  const checksums = parseChecksums(
    await readFile(join(directory, "SHA256SUMS.txt"), "utf8"),
    label,
  );
  const assets = [];
  const names = new Set();
  for (const entry of manifest.files) {
    const { name, sha256, size } = entry ?? {};
    assertSafeFileName(name, `${label} target manifest file`);
    if (names.has(name)) fail(`${label} target manifest has a duplicate file: ${name}`);
    names.add(name);
    if (!/^[a-f0-9]{64}$/.test(sha256 ?? "")) {
      fail(`${label} target manifest has an invalid SHA-256 for ${name}`);
    }
    if (!Number.isInteger(size) || size < 0) {
      fail(`${label} target manifest has an invalid size for ${name}`);
    }
    const path = join(directory, name);
    const info = await stat(path).catch(() => null);
    if (!info?.isFile()) fail(`${label} target asset is missing: ${name}`);
    if (info.size !== size) fail(`${label} target asset size mismatch: ${name}`);
    const actualHash = await sha256File(path);
    if (actualHash !== sha256) fail(`${label} target asset hash mismatch: ${name}`);
    if (checksums.get(name) !== actualHash) {
      fail(`${label} SHA256SUMS.txt does not match ${name}`);
    }
    assets.push({ name, path, sha256: actualHash });
  }

  const sbomName = `codex-barbar_${version}_sbom.spdx.json`;
  const sbomPath = join(directory, sbomName);
  const sbom = await readJson(sbomPath, `${label} SBOM`);
  if (sbom.spdxVersion !== "SPDX-2.3" || sbom.name !== "codex-barbar") {
    fail(`${label} SBOM is not a codex-barbar SPDX 2.3 document`);
  }

  return { label, manifestPath, sbomPath, assets };
}

async function assertEmptyDirectory(path) {
  await mkdir(path, { recursive: true });
  if ((await readdir(path)).length !== 0) {
    fail(`output directory must be empty: ${path}`);
  }
}

/**
 * Validates independent Windows/Linux release artifact sets, copies their
 * payloads and provenance files to one directory, and writes the one checksum
 * file intended for a GitHub Release.
 */
export async function aggregateReleaseAssets({ version, windows, linux, output }) {
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?$/.test(version)) {
    fail(`invalid version: ${version}`);
  }
  for (const [label, directory] of Object.entries({ windows, linux, output })) {
    if (typeof directory !== "string" || !directory) fail(`${label} directory is required`);
  }

  const [windowsTarget, linuxTarget] = await Promise.all([
    loadTarget({ label: "windows", directory: windows, version }),
    loadTarget({ label: "linux", directory: linux, version }),
  ]);
  await assertEmptyDirectory(output);

  const allAssets = [...windowsTarget.assets, ...linuxTarget.assets];
  const names = new Set();
  for (const asset of allAssets) {
    if (names.has(asset.name)) fail(`target artifact names collide: ${asset.name}`);
    names.add(asset.name);
    await cp(asset.path, join(output, asset.name));
  }
  for (const target of [windowsTarget, linuxTarget]) {
    await cp(target.manifestPath, join(output, `artifact-manifest-${target.label}.json`));
    await cp(target.sbomPath, join(output, `codex-barbar_${version}_${target.label}_sbom.spdx.json`));
  }

  const sums = allAssets
    .sort((left, right) => left.name.localeCompare(right.name))
    .map(({ name, sha256 }) => `${sha256}  ${name}`)
    .join("\n");
  await writeFile(join(output, "SHA256SUMS.txt"), `${sums}\n`, "utf8");
}

function parseCli(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (!Object.hasOwn({ "--version": true, "--windows": true, "--linux": true, "--output": true }, flag)) {
      fail(`unknown argument: ${flag}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) fail(`${flag} requires a value`);
    options[flag.slice(2)] = value;
    index += 1;
  }
  return options;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = parseCli(process.argv.slice(2));
  aggregateReleaseAssets(options)
    .then(() => console.log(`Aggregated release assets in ${options.output}`))
    .catch((error) => {
      console.error(`error: ${error.message}`);
      process.exitCode = 1;
    });
}
