#Requires -Version 5.1
<#
.SYNOPSIS
    Smoke-test the codex-barbar NSIS installer on a Windows host.

.DESCRIPTION
    Installs to a temp directory (or an explicit -InstallDir), verifies HKCU
    install scope, Start Menu shortcut, x64 GUI binary, displayed version,
    running tray process, upgrade preserves data, default uninstall preserves
    data, and (only with -PurgeData on a machine whose data root is empty or
    missing) that --purge-user-data exits 0. The real user data root at
    %LOCALAPPDATA%\codex-barbar is never touched unless -PurgeData is
    explicitly supplied and the root is empty/missing.

.PARAMETER InstallerPath
    Path to codex-barbar_<version>_x64-setup.exe.

.PARAMETER ExpectedVersion
    Version that must match the installed registry DisplayVersion.

.PARAMETER InstallDir
    Directory for the smoke install. Defaults to a unique temp directory.

.PARAMETER LeaveInstalled
    Keep the installation after the smoke run.

.PARAMETER PurgeData
    Also exercise --purge-user-data. Refuses to run when
    %LOCALAPPDATA%\codex-barbar contains data.

.PARAMETER SelfTest
    Validate script plumbing with synthetic PE files; no install is run.
#>
param(
    [string]$InstallerPath,

    [string]$ExpectedVersion = "",

    [string]$InstallDir = "",

    [switch]$LeaveInstalled,

    [switch]$PurgeData,

    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ProductName = "codex-barbar"
$DataRoot = Join-Path $env:LOCALAPPDATA $ProductName
$UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$ProductName"
$StartMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"

function Write-Step {
    param([string]$Message)
    Write-Host "[smoke-install] $Message"
}

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Message
    )
    if (-not $Condition) {
        throw $Message
    }
    Write-Step $Message
}

function Stop-TrayProcess {
    Get-Process -Name $ProductName -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

function Get-PeMachine {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64) {
        return -1
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
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
    $subsystemOffset = if ($optMagic -eq 0x20B) { 68 } else { 64 }
    return [BitConverter]::ToUInt16($bytes, $peOffset + 24 + $subsystemOffset)
}

if ($SelfTest) {
    $temp = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-install-selftest-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $temp | Out-Null
    try {
        $fake = Join-Path $temp "fake.exe"
        $pe = New-Object byte[] 512
        $pe[0] = 0x4D; $pe[1] = 0x5A
        $pe[0x3C] = 0x80
        $pe[0x80] = 0x50; $pe[0x81] = 0x45; $pe[0x82] = 0x00; $pe[0x83] = 0x00
        $pe[0x84] = 0x64; $pe[0x85] = 0x86
        $pe[0x98] = 0x0B; $pe[0x99] = 0x02
        $pe[0xDC] = 0x02; $pe[0xDD] = 0x00
        [IO.File]::WriteAllBytes($fake, $pe)
        if ((Get-PeMachine $fake) -ne 0x8664 -or (Get-PeSubsystem $fake) -ne 2) {
            throw "SelfTest PE helpers failed"
        }
        Write-Host "[selftest] windows-smoke-install.ps1 OK"
        exit 0
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if (-not $InstallerPath) {
    throw "InstallerPath is required."
}

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) {
    throw "This smoke test must run on Windows."
}

$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
if ([IO.Path]::GetExtension($installer).ToLowerInvariant() -ne ".exe") {
    throw "Expected an NSIS .exe installer, got: $installer"
}

if (-not $InstallDir) {
    $InstallDir = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-install-smoke-" + [guid]::NewGuid().ToString("N"))
}
$InstallDir = (New-Item -ItemType Directory -Force -Path $InstallDir).FullName

$dataExists = Test-Path -LiteralPath $DataRoot
$dataEmpty = $true
if ($dataExists) {
    $dataEmpty = @(Get-ChildItem -LiteralPath $DataRoot -Force -ErrorAction SilentlyContinue).Count -eq 0
}
if ($PurgeData -and $dataExists -and -not $dataEmpty) {
    throw "Refusing -PurgeData: $DataRoot is not empty. Back up or remove it manually first."
}

Stop-TrayProcess
Write-Step "installer: $installer"
Write-Step "install dir: $InstallDir"

$installedExe = Join-Path $InstallDir "$ProductName.exe"
$uninstaller = Join-Path $InstallDir "uninstall.exe"

try {
    # Fresh silent install into a dedicated directory. NSIS accepts /D= only
    # as the final switch and without quotes around the value.
    Write-Step "running silent install"
    $install = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$InstallDir") -Wait -PassThru
    Assert-True ($install.ExitCode -eq 0) "installer exited $($install.ExitCode), expected 0"

    Assert-True (Test-Path -LiteralPath $installedExe) "installed exe exists: $installedExe"
    Assert-True ((Get-PeMachine $installedExe) -eq 0x8664) "installed exe is x64"
    Assert-True ((Get-PeSubsystem $installedExe) -eq 2) "installed exe is GUI subsystem"

    $versionInfo = (Get-Item -LiteralPath $installedExe).VersionInfo
    if ($ExpectedVersion) {
        Assert-True ($versionInfo.FileVersion -eq $ExpectedVersion) "installed exe FileVersion $($versionInfo.FileVersion) matches $ExpectedVersion"
    }

    Assert-True (Test-Path -LiteralPath $uninstaller) "uninstaller exists: $uninstaller"

    Assert-True (Test-Path -LiteralPath $UninstallKey) "HKCU uninstall key exists (currentUser scope)"
    $uninstallEntry = Get-ItemProperty -LiteralPath $UninstallKey
    Assert-True ($uninstallEntry.DisplayName -eq $ProductName) "registry DisplayName is $ProductName"
    if ($ExpectedVersion) {
        Assert-True ($uninstallEntry.DisplayVersion -eq $ExpectedVersion) "registry DisplayVersion $($uninstallEntry.DisplayVersion) matches $ExpectedVersion"
    }

    $shortcut = Join-Path $StartMenu "$ProductName.lnk"
    Assert-True (Test-Path -LiteralPath $shortcut) "Start Menu shortcut exists: $shortcut"

    # Running tray process smoke.
    Write-Step "launching installed app"
    $app = Start-Process -FilePath $installedExe -PassThru
    Start-Sleep -Seconds 5
    $running = Get-Process -Name $ProductName -ErrorAction SilentlyContinue
    Assert-True ($null -ne $running) "installed app is running after launch"
    Stop-TrayProcess

    # Upgrade install preserves program files and user data.
    Write-Step "running upgrade install"
    $upgrade = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$InstallDir") -Wait -PassThru
    Assert-True ($upgrade.ExitCode -eq 0) "upgrade installer exited $($upgrade.ExitCode), expected 0"
    Assert-True (Test-Path -LiteralPath $installedExe) "upgrade preserved installed exe"
    Assert-True ((Test-Path -LiteralPath $DataRoot) -eq $dataExists) "upgrade preserved user data root state"

    # Explicit purge runs while the installed exe still exists (the NSIS
    # uninstaller hook invokes it before deleting app files). Only runs when
    # the caller asked for it and the data root is missing/empty, so the
    # developer machine is never damaged.
    if ($PurgeData) {
        $purge = Start-Process -FilePath $installedExe -ArgumentList @("--purge-user-data") -Wait -PassThru
        Assert-True ($purge.ExitCode -eq 0) "--purge-user-data exited $($purge.ExitCode), expected 0"
        Assert-True (-not (Test-Path -LiteralPath $DataRoot)) "purge removed $DataRoot"
    }

    # Default uninstall preserves user data (or preserves its prior absence).
    $dataBeforeUninstall = Test-Path -LiteralPath $DataRoot
    if (-not $LeaveInstalled) {
        Write-Step "running silent uninstall"
        $uninstall = Start-Process -FilePath $uninstaller -ArgumentList @("/S") -Wait -PassThru
        Assert-True ($uninstall.ExitCode -eq 0) "uninstaller exited $($uninstall.ExitCode), expected 0"
        Assert-True (-not (Test-Path -LiteralPath $installedExe)) "uninstall removed installed exe"
        Assert-True ((Test-Path -LiteralPath $DataRoot) -eq $dataBeforeUninstall) "default uninstall preserved user data root state"
    }
} finally {
    Stop-TrayProcess
    if (-not $LeaveInstalled) {
        if (Test-Path -LiteralPath $uninstaller) {
            & $uninstaller /S | Out-Null
        }
        if (Test-Path -LiteralPath $InstallDir) {
            Remove-Item -LiteralPath $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Write-Step "ok"
