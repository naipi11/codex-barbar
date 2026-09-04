# Regression coverage for the release workflow policy guard.
# It runs the real guard against a CRLF-encoded copy of the workflow, matching
# the line endings checked out by the Windows GitHub runner.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-barbar-release-policy-{0}" -f [guid]::NewGuid())

try {
    $workflowDirectory = Join-Path $temporaryRoot '.github\workflows'
    $scriptDirectory = Join-Path $temporaryRoot 'scripts'
    New-Item -ItemType Directory -Force -Path $workflowDirectory, $scriptDirectory | Out-Null

    Copy-Item -LiteralPath (Join-Path $repoRoot 'scripts\assert-release-workflow.ps1') -Destination $scriptDirectory
    foreach ($workflowName in @('release.yml', 'pr-check.yml')) {
        $source = Join-Path $repoRoot ".github\workflows\$workflowName"
        $destination = Join-Path $workflowDirectory $workflowName
        $content = [System.IO.File]::ReadAllText($source)
        $crlfContent = $content -replace "`r?`n", "`r`n"
        [System.IO.File]::WriteAllText($destination, $crlfContent, [System.Text.UTF8Encoding]::new($false))
    }

    & (Join-Path $scriptDirectory 'assert-release-workflow.ps1')

    $prWorkflowPath = Join-Path $workflowDirectory 'pr-check.yml'
    $prWorkflow = [System.IO.File]::ReadAllText($prWorkflowPath)
    $withoutPermissions = $prWorkflow -replace '(?m)^permissions:\r?\n  contents: read\r?\n\r?\n', ''
    [System.IO.File]::WriteAllText($prWorkflowPath, $withoutPermissions, [System.Text.UTF8Encoding]::new($false))

    $rejectedMissingPrPermissions = $false
    try {
        & (Join-Path $scriptDirectory 'assert-release-workflow.ps1')
    }
    catch {
        if ($_.Exception.Message -eq 'PR workflow default permissions must be contents: read.') {
            $rejectedMissingPrPermissions = $true
        }
        else {
            throw
        }
    }
    if (-not $rejectedMissingPrPermissions) {
        throw 'Policy guard accepted a PR workflow without explicit read-only permissions.'
    }

    Write-Host '[assert-release-workflow.test] OK - policy guard accepts CRLF workflows'
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
