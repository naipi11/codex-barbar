# codex-barbar Portable Build

The portable ZIP contains only the files needed to run `codex-barbar.exe`:

```text
codex-barbar.exe
LICENSE
UPSTREAMS.md
README.md
README.zh-CN.md
PORTABLE.md
```

No installer or registry change is made. The executable is the same
Windows x64 desktop binary that the NSIS installer deploys.

## Data location

The portable build writes all accounts, cache, logs, and settings to
`%LOCALAPPDATA%\codex-barbar`, exactly like the installed build. It never
writes data beside the executable. To remove the app's data, close
codex-barbar and delete `%LOCALAPPDATA%\codex-barbar`.

## First run

Unblock the downloaded ZIP before extraction (right-click the ZIP, then
Properties, and check Unblock). Windows SmartScreen may warn because the
unsigned portable binary is downloaded from the internet.

## Updates

The portable build does not update itself. Replace the extracted files with
the new release and keep the data directory untouched.
