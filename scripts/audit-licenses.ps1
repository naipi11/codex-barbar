#Requires -Version 5.1
<#
.SYNOPSIS
    Fail-closed license audit for codex-barbar release dependencies.

.DESCRIPTION
    Reads the locked Cargo graph via `cargo metadata --locked` and the
    frontend graph via `pnpm licenses list --json`, then enforces
    scripts/license-policy.json: every SPDX token must be in the allowlist or
    match a reviewed package/version exception. Missing/unknown licenses and
    GPL/AGPL/SSPL expressions fail the run.

.PARAMETER SelfTest
    Validate the policy logic with synthetic packages; no repo scan is run.
#>
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$env:CI = "true"
$Failures = New-Object System.Collections.Generic.List[string]

function Get-NormalizedLicense {
    param([string]$License)
    if (-not $License) {
        return ""
    }
    $normalized = $License.Trim()
    $normalized = $normalized -replace '\s*/\s*', ' OR '
    $normalized = $normalized -replace '\s+', ' '
    return $normalized.Trim()
}

function Test-LicenseAllowed {
    param(
        [string]$PackageName,
        [string]$PackageVersion,
        [string]$License,
        [System.Collections.Generic.HashSet[string]]$Allowed,
        [object[]]$Exceptions
    )

    $normalized = Get-NormalizedLicense $License
    if (-not $normalized) {
        return $false
    }

    foreach ($exception in $Exceptions) {
        $exceptionKey = [string]$exception.package
        $packageKey = "$PackageName@$PackageVersion"
        if ($exceptionKey -eq $packageKey -and (Get-NormalizedLicense $exception.license) -eq $normalized) {
            return $true
        }
    }

    $tokens = $normalized -split '\s+(?:OR|AND)\s+'
    foreach ($token in $tokens) {
        $trimmed = $token.Trim('(', ')', ' ')
        if (-not $trimmed) {
            continue
        }
        if (-not $Allowed.Contains($trimmed)) {
            return $false
        }
    }
    return $true
}

function Assert-NoGplFamily {
    param(
        [string]$PackageName,
        [string]$PackageVersion,
        [string]$License,
        [string]$Source,
        [object[]]$Exceptions
    )

    $normalized = Get-NormalizedLicense $License
    foreach ($exception in $Exceptions) {
        $exceptionKey = [string]$exception.package
        $packageKey = "$PackageName@$PackageVersion"
        if ($exceptionKey -eq $packageKey -and (Get-NormalizedLicense $exception.license) -eq $normalized) {
            return $true
        }
    }
    if ($normalized -match 'GPL|AGPL|SSPL') {
        $Failures.Add("$PackageName@$PackageVersion ($Source) uses a GPL-family license: $License")
        return $false
    }
    return $true
}

function Read-Policy {
    param([string]$PolicyPath)

    $policy = Get-Content -Raw -LiteralPath $PolicyPath | ConvertFrom-Json
    $allowed = New-Object System.Collections.Generic.HashSet[string]
    foreach ($id in $policy.allowedSpdxIds) {
        [void]$allowed.Add($id)
    }
    $exceptions = @($policy.exceptions)
    return @{ Allowed = $allowed; Exceptions = $exceptions }
}

function Invoke-CargoAudit {
    param([hashtable]$Policy)

    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $metadataJson = & cargo metadata --manifest-path (Join-Path $RepoRoot "rust\Cargo.toml") --locked --format-version 1 2>$null
        $cargoExit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($cargoExit -ne 0) {
        throw "cargo metadata exited with $cargoExit"
    }
    $metadata = $metadataJson | Out-String | ConvertFrom-Json
    if (-not $metadata) {
        throw "cargo metadata returned no data"
    }
    foreach ($package in $metadata.packages) {
        if (-not $package.source) {
            continue
        }
        if ($package.name -in @("codexbar", "codex-barbar-desktop")) {
            continue
        }
        $key = "$($package.name)@$($package.version)"
        if (-not (Test-LicenseAllowed `
                -PackageName $package.name `
                -PackageVersion $package.version `
                -License $package.license `
                -Allowed $Policy.Allowed `
                -Exceptions $Policy.Exceptions)) {
            $Failures.Add("$key (cargo) license not allowed: $($package.license)")
        }
        [void](Assert-NoGplFamily $package.name $package.version $package.license "cargo" $Policy.Exceptions)
    }
}

function Invoke-PnpmAudit {
    param([hashtable]$Policy)

    $licensesJson = & corepack pnpm@10.18.1 --config.node-linker=hoisted --dir (Join-Path $RepoRoot "apps\desktop-tauri") licenses list --json 2>$null
    $licensesText = $licensesJson | Out-String
    $licenses = $null
    try {
        $licenses = $licensesText | ConvertFrom-Json
    } catch {
        throw "pnpm licenses list returned no parseable JSON: $($_.Exception.Message)"
    }
    if (-not $licenses) {
        throw "pnpm licenses list returned no data"
    }

    foreach ($property in $licenses.PSObject.Properties) {
        $license = $property.Name
        foreach ($package in $property.Value) {
            $version = @($package.versions)[0]
            $key = "$($package.name)@$version"
            if (-not (Test-LicenseAllowed `
                    -PackageName $package.name `
                    -PackageVersion $version `
                    -License $license `
                    -Allowed $Policy.Allowed `
                    -Exceptions $Policy.Exceptions)) {
                $Failures.Add("$key (npm) license not allowed: $license")
            }
            [void](Assert-NoGplFamily $package.name $version $license "npm" $Policy.Exceptions)
        }
    }
}

if ($SelfTest) {
    $policyPath = Join-Path $RepoRoot "scripts\license-policy.json"
    $policy = Read-Policy $policyPath

    if (-not (Test-LicenseAllowed "a" "1.0.0" "MIT" $policy.Allowed $policy.Exceptions)) {
        throw "SelfTest: MIT should be allowed"
    }
    if (-not (Test-LicenseAllowed "b" "1.0.0" "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT" $policy.Allowed $policy.Exceptions)) {
        throw "SelfTest: Apache-2.0 WITH LLVM-exception expression should be allowed"
    }
    if (Test-LicenseAllowed "c" "1.0.0" "GPL-3.0-only" $policy.Allowed $policy.Exceptions) {
        throw "SelfTest: GPL-3.0-only must not be allowed"
    }
    if (Test-LicenseAllowed "d" "1.0.0" "" $policy.Allowed $policy.Exceptions) {
        throw "SelfTest: missing license must not be allowed"
    }
    if (-not (Test-LicenseAllowed "self_cell" "1.2.2" "Apache-2.0 OR GPL-2.0-only" $policy.Allowed $policy.Exceptions)) {
        throw "SelfTest: reviewed exception self_cell@1.2.2 should be allowed"
    }
    Write-Host "[selftest] audit-licenses.ps1 OK"
    exit 0
}

$policy = Read-Policy (Join-Path $RepoRoot "scripts\license-policy.json")
Invoke-CargoAudit $policy
Invoke-PnpmAudit $policy

if ($Failures.Count -gt 0) {
    Write-Host ""
    Write-Host "$($Failures.Count) license check(s) failed:" -ForegroundColor Red
    $Failures | ForEach-Object { Write-Host "  - $_" }
    exit 1
}

Write-Host "[audit-licenses] OK - all locked dependencies pass the license policy"
