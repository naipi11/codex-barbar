#Requires -Version 5.1
<#
.SYNOPSIS
    Verify Authenticode signatures on the Windows release executables.

.PARAMETER AssetsDirectory
    Directory containing the release artifacts.

.PARAMETER Version
    Release version used to locate the setup executable and portable ZIP.

.PARAMETER ExpectedSubject
    Optional certificate subject fragment that must match the signer.

.PARAMETER RequireSignature
    Fail when any executable is unsigned or has an invalid signature.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AssetsDirectory,

    [Parameter(Mandatory = $true)]
    [string]$Version,

    [string]$ExpectedSubject,

    [switch]$RequireSignature
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$AssetsDirectory = (Resolve-Path -LiteralPath $AssetsDirectory).Path
$setupPath = Join-Path $AssetsDirectory "codex-barbar_${Version}_x64-setup.exe"
$portablePath = Join-Path $AssetsDirectory "codex-barbar_${Version}_x64-portable.zip"

if (-not (Test-Path -LiteralPath $setupPath -PathType Leaf)) {
    throw "Missing setup executable: $setupPath"
}
if (-not (Test-Path -LiteralPath $portablePath -PathType Leaf)) {
    throw "Missing portable archive: $portablePath"
}

$temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-barbar-signature-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
try {
    Expand-Archive -LiteralPath $portablePath -DestinationPath $temporaryDirectory -Force
    $portableExe = Join-Path $temporaryDirectory "codex-barbar.exe"
    if (-not (Test-Path -LiteralPath $portableExe -PathType Leaf)) {
        throw "Portable archive does not contain codex-barbar.exe"
    }

    $failures = New-Object System.Collections.Generic.List[string]
    foreach ($path in @($setupPath, $portableExe)) {
        $signature = Get-AuthenticodeSignature -FilePath $path
        $name = Split-Path -Leaf $path
        $subject = if ($signature.SignerCertificate) {
            $signature.SignerCertificate.Subject
        } else {
            ""
        }

        if ($signature.Status -eq [System.Management.Automation.SignatureStatus]::Valid) {
            if ($ExpectedSubject -and $subject -notlike "*${ExpectedSubject}*") {
                $failures.Add("$name signer subject does not contain '$ExpectedSubject': $subject")
                Write-Host "[fail] $name signer subject: $subject" -ForegroundColor Red
            } else {
                Write-Host "[ok] $name signed by $subject"
            }
        } else {
            $message = "$name signature status: $($signature.Status)"
            if ($RequireSignature) {
                $failures.Add($message)
                Write-Host "[fail] $message" -ForegroundColor Red
            } else {
                Write-Host "[skip] $message (unsigned builds are currently allowed)" -ForegroundColor Yellow
            }
        }
    }

    if ($failures.Count -gt 0) {
        throw ($failures -join [Environment]::NewLine)
    }
} finally {
    Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Authenticode signature checks completed." -ForegroundColor Green
