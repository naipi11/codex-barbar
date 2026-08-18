#Requires -Version 5.1
<#
.SYNOPSIS
    Generate a deterministic SPDX 2.3 JSON SBOM for codex-barbar.

.DESCRIPTION
    Reads the locked Cargo graph via `cargo metadata --locked` plus
    Cargo.lock checksums, and the frontend graph via `pnpm licenses list
    --json`. Packages and DEPENDS_ON relationships are sorted before
    serialization; the document namespace and creation timestamp are derived
    from the Git commit so repeated runs on the same commit are byte-identical.

.PARAMETER Version
    Product version. Defaults to the rust/Cargo.toml version.

.PARAMETER Commit
    Git commit for the document namespace. Defaults to HEAD.

.PARAMETER OutputPath
    Output JSON path (required unless -SelfTest).

.PARAMETER SelfTest
    Generate twice to temp paths and assert byte-identical output.
#>
param(
    [string]$Version = "",
    [string]$Commit = "",
    [string]$OutputPath = "",
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$env:CI = "true"

function Get-RustVersion {
    $text = Get-Content -Raw -LiteralPath (Join-Path $RepoRoot "rust\Cargo.toml")
    if ($text -notmatch '(?m)^version = "([^"]+)"') {
        throw "Cannot parse rust/Cargo.toml version"
    }
    return $Matches[1]
}

function Get-GitCommit {
    if ($Commit) {
        return $Commit
    }
    $git = Get-Command git -ErrorAction SilentlyContinue
    if (-not $git) {
        return "0" * 40
    }
    Push-Location $RepoRoot
    try {
        $head = (& $git.Source rev-parse HEAD).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $head) {
            return "0" * 40
        }
        return $head
    } finally {
        Pop-Location
    }
}

function Get-GitCommitDate {
    $git = Get-Command git -ErrorAction SilentlyContinue
    if (-not $git) {
        return "1970-01-01T00:00:00Z"
    }
    Push-Location $RepoRoot
    try {
        $date = (& $git.Source show -s --format=%cI HEAD).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $date) {
            return "1970-01-01T00:00:00Z"
        }
        return $date
    } finally {
        Pop-Location
    }
}

function Get-NormalizedLicense {
    param([string]$License)
    if (-not $License) {
        return ""
    }
    return (($License.Trim() -replace '\s*/\s*', ' OR ' -replace '\s+', ' ')).Trim()
}

function ConvertTo-SpdxId {
    param([string]$Prefix, [string]$Value)
    $sanitized = ($Value -replace '[^A-Za-z0-9.\-]', '-')
    return "SPDXRef-$Prefix-$sanitized"
}

function Get-Purl {
    param(
        [string]$Ecosystem,
        [string]$Name,
        [string]$PackageVersion
    )
    if ($Ecosystem -eq "npm") {
        $encoded = $Name
        if ($Name.StartsWith("@")) {
            $encoded = "%40" + $Name.Substring(1)
        }
        return "pkg:npm/$encoded@$PackageVersion"
    }
    return "pkg:cargo/$($Name.ToLowerInvariant())@$PackageVersion"
}

function Get-DownloadLocation {
    param(
        [string]$Ecosystem,
        [string]$Name,
        [string]$PackageVersion
    )
    if ($Ecosystem -eq "npm") {
        return "https://registry.npmjs.org/$Name/-/$PackageVersion"
    }
    return "https://crates.io/crates/$Name/$PackageVersion"
}

function Get-CargoLockChecksums {
    $lockPath = Join-Path $RepoRoot "Cargo.lock"
    if (-not (Test-Path -LiteralPath $lockPath)) {
        return @{}
    }
    $checksums = @{}
    $current = $null
    foreach ($line in Get-Content -LiteralPath $lockPath) {
        if ($line -match '^\[\[package\]\]$') {
            $current = @{ name = ""; version = ""; checksum = "" }
            continue
        }
        if ($null -ne $current) {
            if ($line -match '^name = "(.+)"$') {
                $current.name = $Matches[1]
            } elseif ($line -match '^version = "(.+)"$') {
                $current.version = $Matches[1]
            } elseif ($line -match '^checksum = "(.+)"$') {
                $current.checksum = $Matches[1]
            }
        }
        if ($null -ne $current -and $current.name -and $current.version -and $current.checksum) {
            $checksums["$($current.name)@$($current.version)"] = $current.checksum
            $current = $null
        }
    }
    return $checksums
}

function Get-CargoPackages {
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
    $checksums = Get-CargoLockChecksums
    $packages = New-Object System.Collections.Generic.List[object]
    foreach ($package in $metadata.packages) {
        if (-not $package.source) {
            continue
        }
        if ($package.name -in @("codexbar", "codex-barbar-desktop")) {
            continue
        }
        $key = "$($package.name)@$($package.version)"
        $spdxId = ConvertTo-SpdxId "Cargo" "$($package.name)-$($package.version)"
        $entry = [ordered]@{
            name = $package.name
            SPDXID = $spdxId
            versionInfo = $package.version
            downloadLocation = Get-DownloadLocation "cargo" $package.name $package.version
            filesAnalyzed = $false
            licenseConcluded = "NOASSERTION"
            licenseDeclared = $(if ($package.license) { Get-NormalizedLicense $package.license } else { "NOASSERTION" })
            copyrightText = "NOASSERTION"
            externalRefs = @(@{
                referenceCategory = "PACKAGE-MANAGER"
                referenceType = "purl"
                referenceLocator = Get-Purl "cargo" $package.name $package.version
            })
        }
        if ($checksums.ContainsKey($key)) {
            $entry.checksums = @(@{
                algorithm = "SHA256"
                checksumValue = $checksums[$key]
            })
        }
        if ($package.repository) {
            $entry.sourceInfo = [string]$package.repository
        } elseif ($package.homepage) {
            $entry.sourceInfo = [string]$package.homepage
        }
        $packages.Add([pscustomobject]$entry)
    }
    return $packages
}

function Get-NpmPackages {
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $licensesJson = & corepack pnpm@10.18.1 --config.node-linker=hoisted --dir (Join-Path $RepoRoot "apps\desktop-tauri") licenses list --json 2>$null
        $pnpmExit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($pnpmExit -ne 0) {
        throw "pnpm licenses list exited with $pnpmExit"
    }
    $licenses = $licensesJson | Out-String | ConvertFrom-Json
    if (-not $licenses) {
        throw "pnpm licenses list returned no data"
    }
    $packages = New-Object System.Collections.Generic.List[object]
    foreach ($property in $licenses.PSObject.Properties) {
        $license = $property.Name
        foreach ($package in $property.Value) {
            $packageVersion = @($package.versions)[0]
            $spdxId = ConvertTo-SpdxId "Npm" "$($package.name)-$packageVersion"
            $entry = [ordered]@{
                name = $package.name
                SPDXID = $spdxId
                versionInfo = $packageVersion
                downloadLocation = Get-DownloadLocation "npm" $package.name $packageVersion
                filesAnalyzed = $false
                licenseConcluded = "NOASSERTION"
                licenseDeclared = $(if ($license) { Get-NormalizedLicense $license } else { "NOASSERTION" })
                copyrightText = "NOASSERTION"
                externalRefs = @(@{
                    referenceCategory = "PACKAGE-MANAGER"
                    referenceType = "purl"
                    referenceLocator = Get-Purl "npm" $package.name $packageVersion
                })
            }
            if ($package.homepage) {
                $entry.sourceInfo = [string]$package.homepage
            }
            $packages.Add([pscustomobject]$entry)
        }
    }
    return $packages
}

function New-Sbom {
    param(
        [string]$ResolvedVersion,
        [string]$ResolvedCommit
    )

    $cargoPackages = Get-CargoPackages
    $npmPackages = Get-NpmPackages
    $allPackages = New-Object System.Collections.Generic.List[object]
    foreach ($entry in $cargoPackages) {
        $allPackages.Add($entry)
    }
    foreach ($entry in $npmPackages) {
        $allPackages.Add($entry)
    }
    $sortedPackages = @($allPackages | Sort-Object name, versionInfo, SPDXID)

    $rootId = "SPDXRef-Package-codex-barbar"
    $root = [ordered]@{
        name = "codex-barbar"
        SPDXID = $rootId
        versionInfo = $ResolvedVersion
        downloadLocation = "NOASSERTION"
        filesAnalyzed = $false
        licenseConcluded = "MIT"
        licenseDeclared = "MIT"
        copyrightText = "NOASSERTION"
    }

    $relationships = New-Object System.Collections.Generic.List[object]
    foreach ($package in $sortedPackages) {
        $relationships.Add([pscustomobject][ordered]@{
            spdxElementId = $rootId
            relatedSpdxElement = $package.SPDXID
            relationshipType = "DEPENDS_ON"
        })
    }
    $sortedRelationships = @($relationships | Sort-Object relatedSpdxElement)

    $packageObjects = @($root) + $sortedPackages
    $document = [ordered]@{
        spdxVersion = "SPDX-2.3"
        dataLicense = "CC0-1.0"
        SPDXID = "SPDXRef-DOCUMENT"
        name = "codex-barbar"
        documentNamespace = "https://github.com/naipi11/codex-barbar/sbom/$ResolvedVersion/$ResolvedCommit"
        creationInfo = [ordered]@{
            created = Get-GitCommitDate
            creators = @("Tool: codex-barbar-sbom")
        }
        packages = $packageObjects
        relationships = $sortedRelationships
    }
    return $document
}

function Write-SbomFile {
    param(
        [hashtable]$Document,
        [string]$Path
    )
    $json = $Document | ConvertTo-Json -Depth 12
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $json, $utf8)
}

if ($SelfTest) {
    $temp = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-sbom-selftest-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $temp | Out-Null
    try {
        $resolvedVersion = $(if ($Version) { $Version } else { Get-RustVersion })
        $a = Join-Path $temp "sbom-a.json"
        $b = Join-Path $temp "sbom-b.json"
        $docA = New-Sbom $resolvedVersion (Get-GitCommit)
        $docB = New-Sbom $resolvedVersion (Get-GitCommit)
        Write-SbomFile $docA $a
        Write-SbomFile $docB $b
        $hashA = (Get-FileHash -Algorithm SHA256 -LiteralPath $a).Hash
        $hashB = (Get-FileHash -Algorithm SHA256 -LiteralPath $b).Hash
        if ($hashA -ne $hashB) {
            throw "SelfTest: SBOM is not deterministic"
        }
        $parsed = Get-Content -Raw -LiteralPath $a | ConvertFrom-Json
        if ($parsed.spdxVersion -ne "SPDX-2.3" -or $parsed.name -ne "codex-barbar") {
            throw "SelfTest: SBOM shape mismatch"
        }
        Write-Host "[selftest] generate-sbom.ps1 OK"
        exit 0
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if (-not $OutputPath) {
    throw "OutputPath is required (or use -SelfTest)."
}
if (-not $Version) {
    $Version = Get-RustVersion
}
if (-not $Commit) {
    $Commit = Get-GitCommit
}

$document = New-Sbom $Version $Commit
Write-SbomFile $document $OutputPath
Write-Host "[generate-sbom] wrote $OutputPath ($($document.packages.Count) packages, $($document.relationships.Count) relationships)"
