# Codex Windows launch compatibility

Probe date: 2026-08-06 (Asia/Shanghai), on a native Windows 11 x64 host.

The compatibility probe is read-only. It does not read `auth.json`, start an
App Server session, send account requests, execute arbitrary `.cmd` content,
or modify the Codex installation. Run it with:

```powershell
.\scripts\codex-app-server-smoke.ps1
```

The script runs the ignored resolver test
`real_machine_resolver_probe_is_read_only`, which supplies explicit `PATH` and
`PATHEXT` snapshots to `CodexCommandResolver`, resolves the selected command,
and performs a direct fixed-argument `--version` probe without a shell.

## Host and installed forms

| Item | Observation |
|---|---|
| OS | Windows 11 Professional, build 26200, x64 |
| Node | `v24.18.0` at `%ProgramFiles%\nodejs\node.exe` |
| npm package | `@openai/codex` version `0.146.0` |
| npm shim | `%APPDATA%\npm\codex.cmd`, 341 bytes |
| npm entry | `%APPDATA%\npm\node_modules\@openai\codex\bin\codex.js` |
| native user install roots | No readable `codex.exe` found under the two tested `%LOCALAPPDATA%\Programs\...` roots |
| WindowsApps alias directory | No `%LOCALAPPDATA%\Microsoft\WindowsApps\codex*` entry surfaced |
| Store package executable | `%ProgramFiles%\WindowsApps\...\resources\codex.exe` was found by `Get-Command`, but direct `--version` returned `Access is denied` |

## Direct-launch results

### Verified npm layout — supported

The resolver matched the checked-in official npm shim after CRLF
normalization. It selected:

```text
installation = VerifiedNpmLayout
program      = %ProgramFiles%\nodejs\node.exe
args_prefix  = %APPDATA%\npm\node_modules\@openai\codex\bin\codex.js
direct probe = codex-cli 0.146.0
```

The `.cmd` file was inspected for exact allow-listed content and was never
executed. The probe then launched `node.exe` directly with the absolute entry
path and `--version`.

### Windows Store form — unsupported on this host

PowerShell could discover a Store package path through `Get-Command`, but a
direct `codex.exe --version` attempt returned `Access is denied`. Discovery
therefore must not claim support merely because the command resolver finds a
path. The production resolver only selects the WindowsApps candidate after
the same regular/non-reparse checks plus a successful direct `--version`
probe; this host does not satisfy that contract.

### Native `%LOCALAPPDATA%\Programs` forms — not installed

Neither ordered native candidate was present/readable on this machine:

```text
%LOCALAPPDATA%\Programs\OpenAI Codex\codex.exe
%LOCALAPPDATA%\Programs\Codex\codex.exe
```

## Compatibility decision

V1 supports the verified npm layout observed above. The Store alias remains a
fail-closed, probe-gated candidate and is not reported as supported on this
host. Arbitrary batch wrappers, PowerShell shims, relative overrides, current
directory PATH segments, inaccessible files, and reparse-point candidates are
rejected.
