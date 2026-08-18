#Requires -Version 5.1
<#
.SYNOPSIS
    Check whether a codex-barbar V1 release is ready or complete.

.DESCRIPTION
    Verifies version-file consistency, artifact presence/hashes/names, the
    artifact manifest, and (optionally) the local Git tag. It never queries
    GitHub or writes external state.

.PARAMETER Version
    Version under test. Defaults to the rust/Cargo.toml version.

.PARAMETER AssetsDirectory
    Directory containing the release artifacts.

.PARAMETER SkipGitTag
    Skip the local Git tag check.
#>
param(
    [string]$Version = "",
    [string]$AssetsDirectory = ".\artifacts\release",
    [switch]$SkipGitTag
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Failures = New-Object System.Collections.Generic.List[string]
$Warnings = New-Object System.Collections.Generic.List[string]

function Write-Ok {
    param([string]$Message)
    Write-Host "[ok] $Message"
}

function Write-Warn {
    param([string]$Message)
    $Warnings.Add($Message)
    Write-Host "[warn] $Message" -ForegroundColor Yellow
}

function Write-Fail {
    param([string]$Message)
    $Failures.Add($Message)
    Write-Host "[fail] $Message" -ForegroundColor Red
}

function Assert-Version {
    param(
        [string]$Label,
        [string]$Actual,
        [string]$Expected
    )
    if ($Actual -eq $Expected) {
        Write-Ok "$Label version is $Actual"
    } else {
        Write-Fail "$Label version is $Actual, expected $Expected"
    }
}

function Get-CargoVersion {
    param([string]$Path)
    $text = Get-Content -Raw -LiteralPath $Path
    if ($text -match '(?m)^version = "([^"]+)"') {
        return $Matches[1]
    }
    return ""
}

if (-not $Version) {
    $Version = Get-CargoVersion (Join-Path $RepoRoot "rust\Cargo.toml")
}
if (-not $Version) {
    throw "Could not determine release version."
}

Write-Host "Release doctor: codex-barbar $Version"
Write-Host ""

Assert-Version "rust/Cargo.toml" (Get-CargoVersion (Join-Path $RepoRoot "rust\Cargo.toml")) $Version
Assert-Version "apps/desktop-tauri/src-tauri/Cargo.toml" (Get-CargoVersion (Join-Path $RepoRoot "apps\desktop-tauri\src-tauri\Cargo.toml")) $Version

$packageJsonPath = Join-Path $RepoRoot "apps\desktop-tauri\package.json"
Assert-Version "apps/desktop-tauri/package.json" (((Get-Content -Raw -LiteralPath $packageJsonPath) | ConvertFrom-Json).version) $Version

$tauriConfigPath = Join-Path $RepoRoot "apps\desktop-tauri\src-tauri\tauri.conf.json"
Assert-Version "tauri.conf.json" (((Get-Content -Raw -LiteralPath $tauriConfigPath) | ConvertFrom-Json).version) $Version

if (-not $SkipGitTag) {
    $tag = "v$Version"
    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($git) {
        Push-Location $RepoRoot
        try {
            & $git.Source rev-parse --verify --quiet "$tag^{commit}" *> $null
            if ($LASTEXITCODE -eq 0) {
                Write-Ok "Git tag exists: $tag"
            } else {
                Write-Warn "Git tag not found locally: $tag"
            }
        } finally {
            Pop-Location
        }
    } else {
        Write-Warn "git not found; skipped local tag check"
    }
}

if (Test-Path -LiteralPath $AssetsDirectory) {
    $expected = @(
        "codex-barbar_${Version}_x64-setup.exe",
        "codex-barbar_${Version}_x64-portable.zip",
        "SHA256SUMS.txt",
        "codex-barbar_${Version}_sbom.spdx.json",
        "artifact-manifest.json"
    )
    foreach ($name in $expected) {
        if (Test-Path -LiteralPath (Join-Path $AssetsDirectory $name)) {
            Write-Ok "asset present: $name"
        } else {
            Write-Fail "missing asset: $name"
        }
    }

    $setupPath = Join-Path $AssetsDirectory "codex-barbar_${Version}_x64-setup.exe"
    $zipPath = Join-Path $AssetsDirectory "codex-barbar_${Version}_x64-portable.zip"
    $sumsPath = Join-Path $AssetsDirectory "SHA256SUMS.txt"

    if ((Test-Path -LiteralPath $setupPath) -and (Test-Path -LiteralPath $zipPath)) {
        $sums = @{}
        Get-Content -LiteralPath $sumsPath | Where-Object { $_ -match '^\s*([0-9a-fA-F]{64})\s+(.+)$' } | ForEach-Object {
            $sums[$Matches[2]] = $Matches[1].ToLowerInvariant()
        }
        foreach ($name in @("codex-barbar_${Version}_x64-setup.exe", "codex-barbar_${Version}_x64-portable.zip")) {
            $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $AssetsDirectory $name)).Hash.ToLowerInvariant()
            if ($sums.ContainsKey($name) -and $sums[$name] -eq $actualHash) {
                Write-Ok "SHA256 matches for $name"
            } else {
                Write-Fail "SHA256 mismatch for $name"
            }
        }
    }

    $manifestPath = Join-Path $AssetsDirectory "artifact-manifest.json"
    if (Test-Path -LiteralPath $manifestPath) {
        try {
            $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
            if ($manifest.version -eq $Version) {
                Write-Ok "artifact manifest version matches $Version"
            } else {
                Write-Fail "artifact manifest version is $($manifest.version), expected $Version"
            }
            if ($manifest.target -eq "x86_64-pc-windows-msvc") {
                Write-Ok "artifact manifest target is x86_64-pc-windows-msvc"
            } else {
                Write-Fail "artifact manifest target is $($manifest.target)"
            }
        } catch {
            Write-Fail "artifact manifest is not valid JSON: $($_.Exception.Message)"
        }
    }
} else {
    Write-Warn "local assets directory not found: $AssetsDirectory"
}

if ($Failures.Count -gt 0) {
    Write-Host ""
    Write-Host "$($Failures.Count) release doctor check(s) failed." -ForegroundColor Red
    exit 1
}

if ($Warnings.Count -gt 0) {
    Write-Host ""
    Write-Host "$($Warnings.Count) warning(s)." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Release doctor passed."
