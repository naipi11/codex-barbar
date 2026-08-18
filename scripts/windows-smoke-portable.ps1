#Requires -Version 5.1
<#
.SYNOPSIS
    Smoke-test the codex-barbar portable ZIP.

.DESCRIPTION
    Expands the portable ZIP to a temp directory, launches the GUI, verifies
    no file is created beside the executable, then stops the process. The
    app's data directory (%LOCALAPPDATA%\codex-barbar) is left untouched.

.PARAMETER ArchivePath
    Path to codex-barbar portable ZIP.

.PARAMETER SelfTest
    Exercise expansion and content checks with a synthetic ZIP and skip the
    GUI launch.
#>
param(
    [string]$ArchivePath = "",
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot

function Write-Step {
    param([string]$Message)
    Write-Host "[smoke-portable] $Message"
}

function Get-ZipFileNames {
    param([string]$ZipPath)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        return @($archive.Entries | ForEach-Object { $_.FullName } | Sort-Object)
    } finally {
        $archive.Dispose()
    }
}

if ($SelfTest) {
    $temp = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-portable-selftest-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $temp | Out-Null
    try {
        $zip = Join-Path $temp "portable.zip"
        $sourceDir = Join-Path $temp "source"
        New-Item -ItemType Directory -Force $sourceDir | Out-Null
        foreach ($name in @("codex-barbar.exe", "LICENSE", "UPSTREAMS.md", "README.md", "README.zh-CN.md", "PORTABLE.md")) {
            Set-Content -LiteralPath (Join-Path $sourceDir $name) -Value "fixture" -Encoding ascii
        }
        Compress-Archive -Path (Join-Path $sourceDir "*") -DestinationPath $zip

        $expected = @(
            "PORTABLE.md",
            "README.md",
            "README.zh-CN.md",
            "UPSTREAMS.md",
            "LICENSE",
            "codex-barbar.exe"
        ) | Sort-Object
        $names = Get-ZipFileNames $zip
        $diff = Compare-Object -ReferenceObject $expected -DifferenceObject $names
        if (@($diff).Count -ne 0) {
            throw "SelfTest zip content mismatch"
        }
        Write-Host "[selftest] windows-smoke-portable.ps1 OK"
        exit 0
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if (-not $ArchivePath) {
    throw "ArchivePath is required."
}
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) {
    throw "This smoke test must run on Windows."
}

$archive = (Resolve-Path -LiteralPath $ArchivePath).Path
if ([IO.Path]::GetExtension($archive).ToLowerInvariant() -ne ".zip") {
    throw "Expected a .zip archive, got: $archive"
}

$existing = Get-Process -Name "codex-barbar" -ErrorAction SilentlyContinue
if ($existing) {
    throw "codex-barbar is already running (PID $($existing.Id -join ', ')). Close it before portable smoke testing."
}

$work = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-portable-smoke-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $work | Out-Null
$startedProcess = $null
try {
    Write-Step "expanding $archive"
    Expand-Archive -LiteralPath $archive -DestinationPath $work

    $expected = @(
        "PORTABLE.md",
        "README.md",
        "README.zh-CN.md",
        "UPSTREAMS.md",
        "LICENSE",
        "codex-barbar.exe"
    ) | Sort-Object
    $actual = @(Get-ChildItem -LiteralPath $work -File | ForEach-Object { $_.Name } | Sort-Object)
    $diff = Compare-Object -ReferenceObject $expected -DifferenceObject $actual
    if (@($diff).Count -ne 0) {
        throw "Expanded contents mismatch: $($diff | ForEach-Object { $_.InputObject } | Out-String)"
    }
    Write-Step "expanded contents match"

    $exe = Join-Path $work "codex-barbar.exe"
    $before = @(Get-ChildItem -LiteralPath $work -File | ForEach-Object { $_.Name } | Sort-Object)
    Write-Step "launching $exe"
    $startedProcess = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds 5
    if ($startedProcess.HasExited) {
        throw "codex-barbar exited during portable smoke with code $($startedProcess.ExitCode)"
    }

    $after = @(Get-ChildItem -LiteralPath $work -File | ForEach-Object { $_.Name } | Sort-Object)
    $diff = Compare-Object -ReferenceObject $before -DifferenceObject $after
    if (@($diff).Count -ne 0) {
        throw "Portable executable wrote files beside itself: $($diff | ForEach-Object { $_.InputObject } | Out-String)"
    }
    Write-Step "no files created beside the executable"
} finally {
    if ($startedProcess -and -not $startedProcess.HasExited) {
        Stop-Process -Id $startedProcess.Id -Force -ErrorAction SilentlyContinue
        $startedProcess.WaitForExit(5000) | Out-Null
    }
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Step "ok"
