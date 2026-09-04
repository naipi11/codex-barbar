#Requires -Version 5.1
<#
.SYNOPSIS
    Build codex-barbar V1 release artifacts from the current checkout.

.DESCRIPTION
    Verifies the checkout is clean (unless -AllowDirty is supplied for
    development), checks every product manifest matches -Version, runs frozen
    pnpm install, frontend tests/build and the Tauri NSIS bundle, then stages
    the setup EXE and portable ZIP, hashes, SPDX document, and artifact
    manifest into -OutputDirectory. It never resets or cleans the user's
    checkout and never touches %LOCALAPPDATA%\codex-barbar.

.PARAMETER Ref
    Git ref that must equal HEAD. Defaults to HEAD.

.PARAMETER Version
    Version for the artifacts, matching ^\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?$.

.PARAMETER OutputDirectory
    Directory for the generated artifacts. Defaults to .\artifacts\release.

.PARAMETER AllowDirty
    Allow an uncommitted worktree. Development-only; release builds must be clean.
#>
param(
    [string]$Ref = "HEAD",
    [string]$Version = "",
    [string]$OutputDirectory = ".\artifacts\release",
    [switch]$AllowDirty
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$env:CARGO_TERM_COLOR = "never"
$env:CARGO_TERM_PROGRESS_WHEN = "never"
$env:NO_COLOR = "1"
$env:CI = "true"

$RepoRoot = (Resolve-Path (Split-Path -Parent $PSScriptRoot)).Path
$FrontendDir = Join-Path $RepoRoot "apps\desktop-tauri"

function Invoke-Native {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList,
        [string]$Label
    )
    Write-Host "==> $Label"
    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

function Get-ManifestVersion {
    param([string]$Path)
    if ([IO.Path]::GetExtension($Path) -eq ".json") {
        return ((Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json).version)
    }
    $text = Get-Content -Raw -LiteralPath $Path
    if ($text -notmatch '(?m)^version = "([^"]+)"') {
        throw "Cannot parse version from $Path"
    }
    return $Matches[1]
}

function Write-JsonFile {
    param(
        [hashtable]$Data,
        [string]$Path
    )
    Write-Utf8NoBom -Path $Path -Content ($Data | ConvertTo-Json -Depth 6)
}

function Write-Utf8NoBom {
    param(
        [string]$Path,
        [string]$Content
    )
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content + [Environment]::NewLine, $utf8)
}

if (-not $Version) {
    throw "Version is required, for example -Version 0.1.0-alpha.1"
}
if ($Version -notmatch '^\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?$') {
    throw "Invalid version: $Version"
}

$git = Get-Command git -ErrorAction SilentlyContinue
if (-not $git) {
    throw "git is required."
}

Push-Location $RepoRoot
try {
    $head = (& $git.Source rev-parse HEAD).Trim()
    if ($Ref -ne "HEAD") {
        $resolved = (& $git.Source rev-parse --verify "$Ref^{commit}").Trim()
        if ($resolved -ne $head) {
            throw "Ref $Ref resolves to $resolved but HEAD is $head. Release builds require exact HEAD/ref equality."
        }
    }

    $status = (& $git.Source status --porcelain) | Where-Object { $_ -notmatch '^\?\?\s+target/' }
    if ($status -and -not $AllowDirty) {
        Write-Host "Dirty files:" -ForegroundColor Red
        $status | ForEach-Object { Write-Host $_ }
        throw "Worktree is not clean. Commit or stash changes first (or pass -AllowDirty for development)."
    }
    if ($status -and $AllowDirty) {
        Write-Host "[warn] Worktree is dirty; -AllowDirty is for development only." -ForegroundColor Yellow
    }

    foreach ($manifest in @(
        (Join-Path $RepoRoot "rust\Cargo.toml"),
        (Join-Path $RepoRoot "apps\desktop-tauri\src-tauri\Cargo.toml"),
        (Join-Path $RepoRoot "apps\desktop-tauri\package.json"),
        (Join-Path $RepoRoot "apps\desktop-tauri\src-tauri\tauri.conf.json")
    )) {
        $actual = Get-ManifestVersion $manifest
        if ($actual -ne $Version) {
            throw "$manifest has version $actual, expected $Version"
        }
    }
    Write-Host "All product manifests match $Version"

    # hoisted node_modules layout: pnpm's default junction-based layout can be
    # rejected on hardened Windows hosts ("untrusted mount point"), and the
    # layout does not affect the compiled frontend artifacts.
    Invoke-Native "corepack" @("pnpm@10.18.1", "--dir", $FrontendDir, "install", "--frozen-lockfile", "--config.node-linker=hoisted") "frozen pnpm install"
    Invoke-Native "corepack" @("pnpm@10.18.1", "--dir", $FrontendDir, "test") "frontend tests"
    Invoke-Native "corepack" @("pnpm@10.18.1", "--dir", $FrontendDir, "run", "build") "frontend build"
    Invoke-Native "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", (Join-Path $RepoRoot "scripts\audit-licenses.ps1")) "license audit"
    Invoke-Native "corepack" @("pnpm@10.18.1", "--dir", $FrontendDir, "run", "tauri:build") "Tauri NSIS bundle"

    $setupName = "codex-barbar_${Version}_x64-setup.exe"
    $setupSource = Join-Path $RepoRoot "target\release\bundle\nsis\$setupName"
    if (-not (Test-Path -LiteralPath $setupSource)) {
        throw "Missing NSIS bundle: $setupSource"
    }

    $outputSpec = $OutputDirectory
    $resolvedOutput = Resolve-Path -LiteralPath $outputSpec -ErrorAction SilentlyContinue
    if ($resolvedOutput) {
        $OutputDirectory = $resolvedOutput.Path
    } else {
        if (-not [IO.Path]::IsPathRooted($outputSpec)) {
            $outputSpec = Join-Path $RepoRoot $outputSpec
        }
        New-Item -ItemType Directory -Force -Path $outputSpec | Out-Null
        $OutputDirectory = (Resolve-Path -LiteralPath $outputSpec).Path
    }

    $setupAsset = Join-Path $OutputDirectory $setupName
    $zipName = "codex-barbar_${Version}_x64-portable.zip"
    $zipAsset = Join-Path $OutputDirectory $zipName
    $sumsAsset = Join-Path $OutputDirectory "SHA256SUMS.txt"
    $sbomAsset = Join-Path $OutputDirectory "codex-barbar_${Version}_sbom.spdx.json"
    $manifestAsset = Join-Path $OutputDirectory "artifact-manifest.json"

    Copy-Item -LiteralPath $setupSource -Destination $setupAsset -Force

    $portableStage = Join-Path $OutputDirectory ("portable-stage-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $portableStage | Out-Null
    try {
        Copy-Item -LiteralPath (Join-Path $RepoRoot "target\release\codex-barbar.exe") -Destination (Join-Path $portableStage "codex-barbar.exe")
        foreach ($doc in @("LICENSE", "UPSTREAMS.md", "README.md", "README.zh-CN.md", "PORTABLE.md")) {
            Copy-Item -LiteralPath (Join-Path $RepoRoot $doc) -Destination (Join-Path $portableStage $doc)
        }
        Compress-Archive -Path (Join-Path $portableStage "*") -DestinationPath $zipAsset -CompressionLevel Optimal -Force
    } finally {
        Remove-Item -LiteralPath $portableStage -Recurse -Force -ErrorAction SilentlyContinue
    }

    $setupHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $setupAsset).Hash.ToLowerInvariant()
    $zipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipAsset).Hash.ToLowerInvariant()
    @(
        "$setupHash  $setupName",
        "$zipHash  $zipName"
    ) | Set-Content -LiteralPath $sumsAsset -Encoding ascii

    Invoke-Native "powershell.exe" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", (Join-Path $RepoRoot "scripts\generate-sbom.ps1"), "-Version", $Version, "-Commit", $head, "-OutputPath", $sbomAsset) "SPDX SBOM generation"

    $sbom = Get-Content -Raw -LiteralPath $sbomAsset | ConvertFrom-Json
    $product = @($sbom.packages | Where-Object { $_.SPDXID -eq "SPDXRef-Package-codex-barbar" })[0]
    if ($null -eq $product) {
        throw "SPDX SBOM has no codex-barbar root package."
    }
    $product | Add-Member -NotePropertyName checksums -NotePropertyValue @(
        [pscustomobject]@{ algorithm = "SHA256"; checksumValue = $setupHash }
        [pscustomobject]@{ algorithm = "SHA256"; checksumValue = $zipHash }
    ) -Force
    $sbom | Add-Member -NotePropertyName target -NotePropertyValue "x86_64-pc-windows-msvc" -Force
    Write-Utf8NoBom -Path $sbomAsset -Content ($sbom | ConvertTo-Json -Depth 12)

    $sumsHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sumsAsset).Hash.ToLowerInvariant()
    $sbomHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $sbomAsset).Hash.ToLowerInvariant()

    $files = @(
        @{ name = $setupName; size = (Get-Item -LiteralPath $setupAsset).Length; sha256 = $setupHash }
        @{ name = $zipName; size = (Get-Item -LiteralPath $zipAsset).Length; sha256 = $zipHash }
        @{ name = "SHA256SUMS.txt"; size = (Get-Item -LiteralPath $sumsAsset).Length; sha256 = $sumsHash }
        @{ name = (Split-Path $sbomAsset -Leaf); size = (Get-Item -LiteralPath $sbomAsset).Length; sha256 = $sbomHash }
    )
    $manifest = @{
        version = $Version
        commit = $head
        target = "x86_64-pc-windows-msvc"
        buildTime = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        signed = $false
        files = $files
    }
    Write-JsonFile $manifest $manifestAsset

    Write-Host ""
    Write-Host "Release artifacts:"
    Get-ChildItem -LiteralPath $OutputDirectory -File | Sort-Object Name | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize
} finally {
    Pop-Location
}
