# Task 10 report - Ubuntu documentation and acceptance

## Scope completed

- Added English and Simplified Chinese Ubuntu 24.04 amd64 installation guidance
  while preserving the existing Windows installer/portable instructions.
- Documented the exact target asset `codex-barbar_1.1.0_amd64.deb`, Debian
  runtime dependencies (WebKitGTK, GTK, Ayatana AppIndicator, and Secret
  Service), GNOME/KDE caveats, Wayland/X11 expectations, Linux's unsupported
  taskbar status, notification capability acceptance, float-ball fallback, and
  XDG autostart filename `com.naipi11.codexbarbar.desktop`.
- Added `docs/LINUX_ACCEPTANCE.md` and the per-candidate record at
  `docs/verification/linux/ubuntu-24.04-acceptance.md`.
- Made release publication explicitly require green Windows and Ubuntu CI plus
  a completed Ubuntu acceptance record for the exact candidate commit.
- Preserved the Task 9 workflow fact that CI uses `pwsh`; no PowerShell 5.1
  BOM concern is made release-blocking in these docs.

## Evidence boundary

This Windows host cannot run Ubuntu GNOME, GNOME Wayland/X11, KDE,
D-Bus/Secret Service, or `dpkg-deb`. The new acceptance record intentionally
labels every package hash, CI link, and desktop/session observation
`PENDING` or `NOT RUN`; it does not claim a tag, a release, or Ubuntu desktop
acceptance.

## Documentation hashes

Hashes are SHA-256 values after the documentation checks below and before the
documentation-only commit:

```text
DD16BF0FB5ABF91516157777E9EFEAC2D687C736C2E91D6D98600CEF9F5B1B15  README.md
F826E276052FE49801EC1C0670F994D1E864B3052104BD69B64A90516C161F25  README.zh-CN.md
7C100A6C3598850425323BFE4A1198AE854EC4ACCF418CAF9C7C88882C0A3A1D  docs/BUILDING.md
BC22D0CAE37564DCAC822762AC755A290272EFCFD00DFDE8E5D6C332A8810D25  docs/RELEASING.md
6782664AC6934A5EB7268D2C1DFB30F8CE3582F96425E31BD2C848592C5D82C7  docs/ARCHITECTURE.md
62CCB91EE6CCCD2C10E237CC7B8E67DAB50E53BD7341A5B46F207DEC28D422C5  CHANGELOG.md
7DBA8BA448915ADB6FCE8F818B3D8D12BE7CD01A5091A9593F040B196E816280  VERSIONING.md
1F7C39058D2363BFA113145F38E4B96060142284B2B2115B6D4A4162A9CB8085  docs/LINUX_ACCEPTANCE.md
C3A2AA4A4C6EE0ADC18FF62FF8E6E9552A01D7C76276962A2FC15123D68F0119  docs/verification/linux/ubuntu-24.04-acceptance.md
```

## Verification

- `rg -n "Ubuntu|\\.deb|amd64|Wayland|Secret Service|AppIndicator|com\\.naipi11\\.codexbarbar\\.desktop|PENDING|NOT RUN|pwsh" ...` — pass; required terms and pending evidence are present across the changed documentation.
- `rg -n "codex-barbar_\\$\\{version\\}_amd64\\.deb|codex-barbar_.*_amd64\\.deb|libwebkit2gtk-4\\.1-0|libayatana-appindicator3-1|libsecret-1-0" ...` — pass; documentation agrees with the Linux Tauri config and package scripts.
- Node local-Markdown-link checker for all nine changed/added documentation files — pass; all local links resolve.
- `cargo fmt --all --check` — pass.
- `git diff --check` — pass.

No package, frontend, or Rust test/build command was run because this task
changes documentation only. No Ubuntu package build or desktop acceptance was
attempted on this Windows host.

## Fix round 1 (review follow-up)

- Corrected the aggregate release asset contract: the aggregator emits the
  combined `SHA256SUMS.txt`, renamed per-target SBOMs, and
  `artifact-manifest-windows.json` / `artifact-manifest-linux.json`; it does
  not emit an aggregate `artifact-manifest.json`.
- Reordered release documentation around the exact candidate commit. Pre-tag
  local validation now uses `-Ref HEAD`; only after exact-commit Windows and
  Ubuntu CI plus acceptance records pass may `v<version>` be created at that
  commit and pushed to run/continue aggregation. The docs retain the `v*`
  resolution rule and workflow-dispatch manifest-version check.
- Replaced the stale `1.0.0` Windows build example with `<version>`.
- Reframed Ubuntu 24.04 amd64 in both READMEs as a preview release target
  pending desktop acceptance, not a supported release platform.

### Fix-round hashes

```text
FBA0EE40790C674FB4FAC4B532E21BCFDFC1217A302E31F47D6C38D6D7320728  README.md
2B0828F6C4238955B556AFF47DAA4D02B63F7F9219BE588DC6120BC35B408FDC  README.zh-CN.md
57D69DEB4CDE639B725D9A935E1A6E31B69A17649A800FD64C034539A9D0BF4E  docs/BUILDING.md
6AC4DF83E8636CDB6B48433395CC3ADFF2D1D047A61D50772AF437E1A94530B6  docs/RELEASING.md
4906435C6352FE6810B99BCFD4EB7D097F57B81E072BDA20BEEB9FA841C02CF9  VERSIONING.md
```

Ubuntu package hash, Ubuntu acceptance, tag, and release remain unclaimed.

## Fix round 2 (release-doctor aggregate path)

- Moved the documented `release-doctor.ps1` invocation out of the pre-tag,
  Windows-only `artifacts/release` build block.
- Documented it only after tagged dual-platform aggregation, with
  `-AssetsDirectory .\\artifacts\\aggregate-release`. This matches the
  script's required eight-file aggregate set: two payloads for Windows, one
  Debian payload, combined `SHA256SUMS.txt`, two renamed SBOMs, and two target
  manifests.
- Added the same post-aggregation release-doctor order to `VERSIONING.md`.

### Fix-round consistency assertion and hashes

The static check requires exactly one documented release-doctor invocation in
`docs/RELEASING.md`, requires that invocation to use
`.\\artifacts\\aggregate-release`, rejects a release-doctor invocation against
`.\\artifacts\\release`, and verifies that it follows the tag/aggregation
instruction. It also checks the matching `VERSIONING.md` order.

```text
8F80A2B7E6C51D5EF795BC2E2AA47113852E5F2D6162093D1D030FEA28D754D9  docs/RELEASING.md
98E19CC387586E9FD0844507236829CE4C36AF28CD6CC1137851D7FF33BB63FD  VERSIONING.md
```

Ubuntu package hash, Ubuntu acceptance, tag, and release remain unclaimed.
