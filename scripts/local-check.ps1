#Requires -Version 5.1
param(
    [switch]$Rust,
    [switch]$Tauri,
    [switch]$Frontend,
    [switch]$Format,
    [switch]$Clippy,
    [switch]$ReleaseDoctor,
    [switch]$All,
    [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$env:CI = "true"

function Invoke-Step {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$ArgumentList
    )

    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

function Invoke-PnpmStep {
    param(
        [string]$Name,
        [string[]]$PnpmArgs
    )

    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    # Pin the project's pnpm version and use the hoisted node_modules layout:
    # pnpm's default junction layout can be rejected on hardened Windows hosts.
    & corepack pnpm@10.18.1 --config.node-linker=hoisted --dir (Join-Path $RepoRoot "apps\desktop-tauri") @PnpmArgs
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

if (-not ($Rust -or $Tauri -or $Frontend -or $Format -or $Clippy -or $ReleaseDoctor -or $All)) {
    $Rust = $true
    $Tauri = $true
    $Frontend = $true
}

Push-Location $RepoRoot
try {
    Invoke-Step "V1 boundary guard" "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\assert-v1-boundaries.ps1")
    Invoke-Step "Release workflow policy guard" "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\assert-release-workflow.ps1")
    Invoke-Step "Release workflow CRLF regression" "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\assert-release-workflow.test.ps1")
    if ($All -or $Format) {
        Invoke-Step "Rust format" "cargo" @("fmt", "--all", "--check")
    }
    if ($All -or $Clippy) {
        Invoke-Step "Shared Rust clippy" "cargo" @("clippy", "--manifest-path", "rust\Cargo.toml", "--all-targets", "--", "-D", "warnings")
        Invoke-Step "Tauri Rust clippy" "cargo" @("clippy", "--manifest-path", "apps\desktop-tauri\src-tauri\Cargo.toml", "--all-targets", "--", "-D", "warnings")
    }
    if ($All -or $Rust) {
        Invoke-Step "Shared Rust tests" "cargo" @("test", "--manifest-path", "rust\Cargo.toml")
    }
    if ($All -or $Tauri) {
        Invoke-Step "Tauri Rust tests" "cargo" @("test", "--manifest-path", "apps\desktop-tauri\src-tauri\Cargo.toml")
    }
    if ($All -or $Frontend) {
        Invoke-PnpmStep "Frontend tests" @("test")
        Invoke-PnpmStep "Frontend build" @("run", "build")
    }
    if ($All) {
        Invoke-PnpmStep "Production dependency audit" @("audit", "--prod", "--audit-level", "high")
        $auditLicenses = Join-Path $RepoRoot "scripts\audit-licenses.ps1"
        if (-not (Test-Path -LiteralPath $auditLicenses)) {
            throw "Missing license audit script: $auditLicenses"
        }
        Invoke-Step "License audit" "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\audit-licenses.ps1")
        Invoke-PnpmStep "Tauri x64 production build" @("run", "tauri:build")
    }
    if ($All -or $ReleaseDoctor) {
        $args = @("-File", "scripts\release-doctor.ps1")
        if ($Version) {
            $args += @("-Version", $Version)
        }
        Invoke-Step "Release doctor" "powershell.exe" $args
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Local checks passed." -ForegroundColor Green
