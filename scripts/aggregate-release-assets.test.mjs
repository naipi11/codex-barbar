import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { createHash } from "node:crypto";

import { aggregateReleaseAssets } from "./aggregate-release-assets.mjs";

const VERSION = "1.1.0";

function sha256(content) {
  return createHash("sha256").update(content).digest("hex");
}

async function writeTargetFixture(root, { target, assetName, sbomName }) {
  await mkdir(root, { recursive: true });
  const assetContent = `${target}:${assetName}`;
  const assetHash = sha256(assetContent);
  await writeFile(join(root, assetName), assetContent);
  await writeFile(join(root, "SHA256SUMS.txt"), `${assetHash}  ${assetName}\n`);
  await writeFile(join(root, sbomName), `${JSON.stringify({ spdxVersion: "SPDX-2.3", name: "codex-barbar" })}\n`);
  await writeFile(
    join(root, "artifact-manifest.json"),
    `${JSON.stringify({
      version: VERSION,
      target,
      files: [{ name: assetName, size: Buffer.byteLength(assetContent), sha256: assetHash }],
    })}\n`,
  );
}

test("aggregates Windows and Linux assets into one checksum file", async () => {
  const temp = await mkdtemp(join(tmpdir(), "codex-barbar-aggregate-"));
  try {
    const windows = join(temp, "windows");
    const linux = join(temp, "linux");
    const output = join(temp, "aggregate");
    await writeTargetFixture(windows, {
      target: "x86_64-pc-windows-msvc",
      assetName: `codex-barbar_${VERSION}_x64-setup.exe`,
      sbomName: `codex-barbar_${VERSION}_sbom.spdx.json`,
    });
    await writeTargetFixture(linux, {
      target: "x86_64-unknown-linux-gnu",
      assetName: `codex-barbar_${VERSION}_amd64.deb`,
      sbomName: `codex-barbar_${VERSION}_sbom.spdx.json`,
    });

    await aggregateReleaseAssets({ version: VERSION, windows, linux, output });

    const sums = await readFile(join(output, "SHA256SUMS.txt"), "utf8");
    assert.match(sums, /codex-barbar_1\.1\.0_x64-setup\.exe/);
    assert.match(sums, /codex-barbar_1\.1\.0_amd64\.deb/);
    assert.equal((await readFile(join(output, "artifact-manifest-windows.json"), "utf8")).includes("windows-msvc"), true);
    assert.equal((await readFile(join(output, "artifact-manifest-linux.json"), "utf8")).includes("linux-gnu"), true);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

test("rejects a target manifest with the wrong target or version", async () => {
  const temp = await mkdtemp(join(tmpdir(), "codex-barbar-aggregate-"));
  try {
    const windows = join(temp, "windows");
    const linux = join(temp, "linux");
    await writeTargetFixture(windows, {
      target: "x86_64-pc-windows-msvc",
      assetName: `codex-barbar_${VERSION}_x64-setup.exe`,
      sbomName: `codex-barbar_${VERSION}_sbom.spdx.json`,
    });
    await writeTargetFixture(linux, {
      target: "x86_64-pc-windows-msvc",
      assetName: `codex-barbar_${VERSION}_amd64.deb`,
      sbomName: `codex-barbar_${VERSION}_sbom.spdx.json`,
    });

    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, windows, linux, output: join(temp, "aggregate") }),
      /linux target manifest target/i,
    );

    const manifestPath = join(linux, "artifact-manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    manifest.target = "x86_64-unknown-linux-gnu";
    manifest.version = "1.1.1";
    await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
    await assert.rejects(
      aggregateReleaseAssets({ version: VERSION, windows, linux, output: join(temp, "aggregate") }),
      /linux target manifest version/i,
    );
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});
