import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { aggregateReleaseAssets } from "./aggregate-release-assets.mjs";

const VERSION = "1.1.0";
const COMMIT = "a".repeat(40);

function sha256(content) {
  return createHash("sha256").update(content).digest("hex");
}

async function writeTargetFixture(root, { label, target, payloadNames }) {
  await mkdir(root, { recursive: true });
  const payloads = await Promise.all(payloadNames.map(async (name) => {
    const content = `${target}:${name}`;
    await writeFile(join(root, name), content);
    return { name, size: Buffer.byteLength(content), sha256: sha256(content) };
  }));
  const sumsName = "SHA256SUMS.txt";
  const sums = payloads.map(({ name, sha256: hash }) => `${hash}  ${name}`).join("\n") + "\n";
  await writeFile(join(root, sumsName), sums);
  const sbomName = `codex-barbar_${VERSION}_sbom.spdx.json`;
  const sbom = {
    spdxVersion: "SPDX-2.3",
    name: "codex-barbar",
    target,
    documentNamespace: `https://example.test/sbom/${VERSION}/fixture`,
    packages: [{
      SPDXID: "SPDXRef-Package-codex-barbar",
      name: "codex-barbar",
      versionInfo: VERSION,
      checksums: payloads.map(({ sha256: checksumValue }) => ({ algorithm: "SHA256", checksumValue })),
    }],
  };
  await writeFile(join(root, sbomName), `${JSON.stringify(sbom)}\n`);
  const supportFiles = await Promise.all([sumsName, sbomName].map(async (name) => {
    const path = join(root, name);
    return { name, size: (await stat(path)).size, sha256: await sha256File(path) };
  }));
  await writeFile(
    join(root, "artifact-manifest.json"),
    `${JSON.stringify({ version: VERSION, commit: COMMIT, target, files: [...payloads, ...supportFiles] })}\n`,
  );
}

async function sha256File(path) {
  return sha256(await readFile(path));
}

async function rewriteManifestSupportFile(root, name) {
  const manifestPath = join(root, "artifact-manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const file = manifest.files.find((entry) => entry.name === name);
  file.size = (await stat(join(root, name))).size;
  file.sha256 = await sha256File(join(root, name));
  await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
}

async function createFixtures(temp) {
  const windows = join(temp, "windows");
  const linux = join(temp, "linux");
  await writeTargetFixture(windows, {
    label: "windows",
    target: "x86_64-pc-windows-msvc",
    payloadNames: [
      `codex-barbar_${VERSION}_x64-setup.exe`,
      `codex-barbar_${VERSION}_x64-portable.zip`,
    ],
  });
  await writeTargetFixture(linux, {
    label: "linux",
    target: "x86_64-unknown-linux-gnu",
    payloadNames: [`codex-barbar_${VERSION}_amd64.deb`],
  });
  return { windows, linux };
}

test("aggregates staging-shaped Windows and Linux artifact manifests", async () => {
  const temp = await mkdtemp(join(tmpdir(), "codex-barbar-aggregate-"));
  try {
    const { windows, linux } = await createFixtures(temp);
    const output = join(temp, "aggregate");

    await aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output });

    const sums = await readFile(join(output, "SHA256SUMS.txt"), "utf8");
    assert.match(sums, /codex-barbar_1\.1\.0_x64-setup\.exe/);
    assert.match(sums, /codex-barbar_1\.1\.0_amd64\.deb/);
    assert.equal((await readFile(join(output, "artifact-manifest-windows.json"), "utf8")).includes("windows-msvc"), true);
    assert.equal((await readFile(join(output, "artifact-manifest-linux.json"), "utf8")).includes("linux-gnu"), true);
    await stat(join(output, `codex-barbar_${VERSION}_windows_sbom.spdx.json`));
    await stat(join(output, `codex-barbar_${VERSION}_linux_sbom.spdx.json`));
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

test("rejects a target manifest with the wrong target or version", async () => {
  const temp = await mkdtemp(join(tmpdir(), "codex-barbar-aggregate-"));
  try {
    const { windows, linux } = await createFixtures(temp);
    const manifestPath = join(linux, "artifact-manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    manifest.target = "x86_64-pc-windows-msvc";
    await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output: join(temp, "aggregate") }),
      /linux target manifest target/i,
    );

    manifest.target = "x86_64-unknown-linux-gnu";
    manifest.version = "1.1.1";
    await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output: join(temp, "aggregate") }),
      /linux target manifest version/i,
    );
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

test("rejects an SBOM with a mismatched product version or payload hash", async () => {
  const temp = await mkdtemp(join(tmpdir(), "codex-barbar-aggregate-"));
  try {
    const { windows, linux } = await createFixtures(temp);
    const sbomPath = join(linux, `codex-barbar_${VERSION}_sbom.spdx.json`);
    const sbom = JSON.parse(await readFile(sbomPath, "utf8"));
    sbom.packages[0].versionInfo = "1.1.1";
    await writeFile(sbomPath, `${JSON.stringify(sbom)}\n`);
    await rewriteManifestSupportFile(linux, `codex-barbar_${VERSION}_sbom.spdx.json`);
    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output: join(temp, "aggregate") }),
      /linux SBOM product version/i,
    );

    sbom.packages[0].versionInfo = VERSION;
    sbom.packages[0].checksums[0].checksumValue = "0".repeat(64);
    await writeFile(sbomPath, `${JSON.stringify(sbom)}\n`);
    await rewriteManifestSupportFile(linux, `codex-barbar_${VERSION}_sbom.spdx.json`);
    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output: join(temp, "aggregate") }),
      /linux SBOM does not reference payload checksum/i,
    );
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

test("rejects missing or mismatched target manifest commits", async () => {
  const temp = await mkdtemp(join(tmpdir(), "codex-barbar-aggregate-"));
  try {
    const { windows, linux } = await createFixtures(temp);
    const manifestPath = join(linux, "artifact-manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    manifest.commit = "b".repeat(40);
    await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);

    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output: join(temp, "mismatch") }),
      /linux target manifest commit/i,
    );

    delete manifest.commit;
    await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, commit: COMMIT, windows, linux, output: join(temp, "missing") }),
      /linux target manifest commit/i,
    );
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});
