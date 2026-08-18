#Requires -Version 5.1
<#
.SYNOPSIS
    Verify codex-barbar release artifacts.

.DESCRIPTION
    Checks exact filenames/set, portable ZIP contents, PE architecture and
    subsystem, version consistency, SHA256SUMS, SPDX JSON parse, and absence
    of old CLI/legacy names.

.PARAMETER Version
    Version under test, for example 0.1.0-alpha.1.

.PARAMETER AssetsDirectory
    Directory containing the release artifacts.

.PARAMETER SelfTest
    Exercise the verifier with synthetic artifacts in a temp directory.

.PARAMETER InternalVerify
    Internal flag used by SelfTest to skip version-resource checks that the
    synthetic PE does not contain.
#>
param(
    [string]$Version = "",
    [string]$AssetsDirectory = ".\artifacts\release",
    [switch]$SelfTest,
    [switch]$InternalVerify
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Failures = New-Object System.Collections.Generic.List[string]

function Write-Ok {
    param([string]$Message)
    Write-Host "[ok] $Message"
}

function Write-Fail {
    param([string]$Message)
    $Failures.Add($Message)
    Write-Host "[fail] $Message" -ForegroundColor Red
}

function Assert-True {
    param(
        [bool]$Condition,
        [string]$FailureMessage,
        [string]$SuccessMessage = ""
    )
    if (-not $Condition) {
        Write-Fail $FailureMessage
    } else {
        Write-Ok $(if ($SuccessMessage) { $SuccessMessage } else { $FailureMessage })
    }
}

function Get-PeMachine {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64) {
        return -1
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    if ($peOffset -lt 0 -or $peOffset + 6 -ge $bytes.Length) {
        return -1
    }
    return [BitConverter]::ToUInt16($bytes, $peOffset + 4)
}

function Get-PeSubsystem {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64) {
        return -1
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    $optMagic = [BitConverter]::ToUInt16($bytes, $peOffset + 24)
    # The Subsystem field is at offset 68 from the optional header start for
    # both PE32 (0x10B) and PE32+ (0x20B).
    if ($peOffset + 24 + 68 + 2 -ge $bytes.Length) {
        return -1
    }
    return [BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68)
}

function Test-PeX64Gui {
    param([string]$Path)
    $machine = Get-PeMachine $Path
    $subsystem = Get-PeSubsystem $Path
    return ($machine -eq 0x8664 -and $subsystem -eq 2)
}

function Test-NsisSetupPe {
    param([string]$Path)
    # The NSIS installer stub is a 32-bit x86 GUI PE even for x64 payloads;
    # Tauri names the artifact *_x64-setup.exe when the app payload is x64.
    $machine = Get-PeMachine $Path
    $subsystem = Get-PeSubsystem $Path
    return ($machine -in @(0x14C, 0x8664) -and $subsystem -eq 2)
}

function Get-ManifestVersion {
    param([string]$Root)
    $rust = Get-Content -Raw (Join-Path $Root "rust\Cargo.toml")
    if ($rust -notmatch '(?m)^version = "([^"]+)"') {
        throw "Cannot parse rust/Cargo.toml version"
    }
    return $Matches[1]
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

if ($SelfTest) {
    $temp = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-verify-selftest-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $temp | Out-Null
    try {
        $setup = Join-Path $temp "codex-barbar_0.1.0-alpha.1_x64-setup.exe"
        $zip = Join-Path $temp "codex-barbar_0.1.0-alpha.1_x64-portable.zip"
        $sums = Join-Path $temp "SHA256SUMS.txt"
        $sbom = Join-Path $temp "codex-barbar_0.1.0-alpha.1_sbom.spdx.json"
        $manifest = Join-Path $temp "artifact-manifest.json"

        # Minimal PE64 GUI header: MZ + e_lfanew + PE signature + COFF machine
        # + optional magic + subsystem offset 68 (PE32+).
        $pe = New-Object byte[] 512
        $pe[0] = 0x4D; $pe[1] = 0x5A
        $pe[0x3C] = 0x80
        $pe[0x80] = 0x50; $pe[0x81] = 0x45; $pe[0x82] = 0x00; $pe[0x83] = 0x00
        $pe[0x84] = 0x64; $pe[0x85] = 0x86            # machine AMD64
        $pe[0x98] = 0x0B; $pe[0x99] = 0x02            # PE32+ magic
        $pe[0xDC] = 0x02; $pe[0xDD] = 0x00            # subsystem GUI (offset 68)
        [IO.File]::WriteAllBytes($setup, $pe)

        $zipStream = [System.IO.Compression.ZipFile]::Open($zip, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            foreach ($name in @("LICENSE", "UPSTREAMS.md", "README.md", "README.zh-CN.md", "PORTABLE.md")) {
                $entry = $zipStream.CreateEntry($name)
                $writer = New-Object System.IO.StreamWriter($entry.Open())
                $writer.Write("content")
                $writer.Close()
            }
            $exeEntry = $zipStream.CreateEntry("codex-barbar.exe")
            $exeStream = $exeEntry.Open()
            $exeStream.Write($pe, 0, $pe.Length)
            $exeStream.Close()
        } finally {
            $zipStream.Dispose()
        }

        $setupHash = (Get-FileHash -Algorithm SHA256 $setup).Hash.ToLowerInvariant()
        $zipHash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
        @(
            "$setupHash  codex-barbar_0.1.0-alpha.1_x64-setup.exe",
            "$zipHash  codex-barbar_0.1.0-alpha.1_x64-portable.zip"
        ) | Set-Content -Encoding ascii $sums

        '{"spdxVersion":"SPDX-2.3","name":"codex-barbar","SPDXID":"SPDXRef-DOCUMENT","dataLicense":"CC0-1.0","packages":[]}' |
            Set-Content -Encoding utf8 $sbom
        '{"version":"0.1.0-alpha.1","target":"x86_64-pc-windows-msvc"}' |
            Set-Content -Encoding utf8 $manifest

        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath -Version "0.1.0-alpha.1" -AssetsDirectory $temp -InternalVerify
        if ($LASTEXITCODE -ne 0) {
            throw "SelfTest verification failed"
        }
        Write-Host "[selftest] verify-release-artifacts.ps1 OK"
        exit 0
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if (-not $Version) {
    $Version = Get-ManifestVersion $RepoRoot
}
if ($Version -notmatch '^\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?$') {
    throw "Invalid version: $Version"
}

$AssetsDirectory = (Resolve-Path -LiteralPath $AssetsDirectory).Path
$expectedFiles = @(
    "codex-barbar_${Version}_x64-setup.exe",
    "codex-barbar_${Version}_x64-portable.zip",
    "SHA256SUMS.txt",
    "codex-barbar_${Version}_sbom.spdx.json",
    "artifact-manifest.json"
)

Write-Host "Verifying codex-barbar $Version in $AssetsDirectory"

foreach ($file in $expectedFiles) {
    Assert-True (Test-Path -LiteralPath (Join-Path $AssetsDirectory $file)) "missing expected file: $file" "expected file present: $file"
}

$actualFiles = Get-ChildItem -LiteralPath $AssetsDirectory -File | ForEach-Object { $_.Name }
$unexpected = @($actualFiles | Where-Object { $expectedFiles -notcontains $_ })
Assert-True ($unexpected.Count -eq 0) "unexpected files in assets directory: $($unexpected -join ', ')" "assets directory contains only expected files"

$setupPath = Join-Path $AssetsDirectory "codex-barbar_${Version}_x64-setup.exe"
$zipPath = Join-Path $AssetsDirectory "codex-barbar_${Version}_x64-portable.zip"

Assert-True (Test-NsisSetupPe $setupPath) "setup exe must be a valid NSIS PE (machine 0x14C/0x8664, subsystem 2)" "setup exe is a valid NSIS PE"

if (-not $InternalVerify) {
    $setupVersion = (Get-Item $setupPath).VersionInfo.FileVersion
    Assert-True ($setupVersion -eq $Version) "setup exe FileVersion is $setupVersion, expected $Version" "setup exe FileVersion matches $Version"
}

$zipArchive = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    $zipNames = @($zipArchive.Entries | ForEach-Object { $_.FullName } | Sort-Object)
    $expectedZipNames = @(
        "PORTABLE.md",
        "README.md",
        "README.zh-CN.md",
        "UPSTREAMS.md",
        "LICENSE",
        "codex-barbar.exe"
    ) | Sort-Object
    $diff = Compare-Object -ReferenceObject $expectedZipNames -DifferenceObject $zipNames
    Assert-True (@($diff).Count -eq 0) "portable zip contents mismatch: $($diff | ForEach-Object { $_.InputObject } | Out-String)" "portable zip contains the exact expected files"

    $portableExeEntry = $zipArchive.Entries | Where-Object { $_.FullName -eq "codex-barbar.exe" } | Select-Object -First 1
    $portableExe = Join-Path $AssetsDirectory "portable-extract-codex-barbar.exe"
    [System.IO.Compression.ZipFileExtensions]::ExtractToFile($portableExeEntry, $portableExe, $true)
    Assert-True (Test-PeX64Gui $portableExe) "portable exe must be x64 GUI" "portable exe is x64 GUI"
    Remove-Item -LiteralPath $portableExe -Force -ErrorAction SilentlyContinue
} finally {
    $zipArchive.Dispose()
}

$sumsPath = Join-Path $AssetsDirectory "SHA256SUMS.txt"
$sums = @{}
Get-Content -LiteralPath $sumsPath | Where-Object { $_ -match '^\s*([0-9a-fA-F]{64})\s+(.+)$' } | ForEach-Object {
    $sums[$Matches[2]] = $Matches[1].ToLowerInvariant()
}
Assert-True ($sums.Keys.Count -eq 2) "SHA256SUMS.txt entry count is $($sums.Keys.Count), expected 2" "SHA256SUMS.txt has exactly 2 entries"
foreach ($file in @("codex-barbar_${Version}_x64-setup.exe", "codex-barbar_${Version}_x64-portable.zip")) {
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $AssetsDirectory $file)).Hash.ToLowerInvariant()
    Assert-True ($sums.ContainsKey($file) -and $sums[$file] -eq $actualHash) "SHA256 mismatch for $file" "SHA256 matches for $file"
}

$sbomPath = Join-Path $AssetsDirectory "codex-barbar_${Version}_sbom.spdx.json"
try {
    $sbomJson = Get-Content -Raw -LiteralPath $sbomPath | ConvertFrom-Json
    Assert-True ($sbomJson.spdxVersion -eq "SPDX-2.3") "SBOM spdxVersion must be SPDX-2.3" "SBOM spdxVersion is SPDX-2.3"
    Assert-True ($sbomJson.name -eq "codex-barbar") "SBOM name must be codex-barbar" "SBOM name is codex-barbar"
} catch {
    Write-Fail "SBOM is not valid JSON: $($_.Exception.Message)"
}

$manifestPath = Join-Path $AssetsDirectory "artifact-manifest.json"
try {
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    Assert-True ($manifest.version -eq $Version) "manifest version mismatch" "manifest version matches $Version"
    Assert-True ($manifest.target -eq "x86_64-pc-windows-msvc") "manifest target mismatch" "manifest target is x86_64-pc-windows-msvc"
} catch {
    Write-Fail "artifact manifest is not valid JSON: $($_.Exception.Message)"
}

$forbiddenLegacy = @(
    "codexbar-cli.exe",
    "codexbar-desktop.exe",
    "codexbar-desktop-tauri.exe",
    "CodexBar.exe",
    "Win-CodexBar",
    "Inno",
    "Wix",
    "MSI"
)
foreach ($name in $actualFiles) {
    foreach ($needle in $forbiddenLegacy) {
        if ($name -like "*$needle*") {
            Write-Fail "legacy/forbidden name in assets: $name"
        }
    }
}

if ($Failures.Count -gt 0) {
    Write-Host ""
    Write-Host "$($Failures.Count) verification check(s) failed." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Release artifacts verified." -ForegroundColor Green
