# Static policy guard for the release workflow.
#
# Dependabot's alerts endpoint is not reliably readable through the automatic
# Actions GITHUB_TOKEN. A dedicated repository secret is mandatory, otherwise
# tag-triggered releases would fail after all build checks had already passed.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$workflowPath = Join-Path $repoRoot '.github\workflows\release.yml'
if (-not (Test-Path -LiteralPath $workflowPath)) {
    throw "Release workflow is missing: $workflowPath"
}

$workflow = Get-Content -Raw -Encoding utf8 -LiteralPath $workflowPath
$gateMatch = [regex]::Match(
    $workflow,
    '(?ms)^      - name: Dependabot alert gate\r?\n.*?(?=^      - name:|\z)'
)
if (-not $gateMatch.Success) {
    throw 'Release workflow has no Dependabot alert gate.'
}

$gate = $gateMatch.Value
$required = @(
    'GH_TOKEN: ${{ secrets.DEPENDABOT_ALERTS_TOKEN }}',
    'DEPENDABOT_ALERTS_TOKEN is not configured',
    '/repos/$env:GITHUB_REPOSITORY/dependabot/alerts'
)
foreach ($fragment in $required) {
    if (-not $gate.Contains($fragment)) {
        throw "Dependabot gate must contain: $fragment"
    }
}

if ($gate.Contains('GITHUB_TOKEN')) {
    throw 'Dependabot gate must not use the automatic GITHUB_TOKEN.'
}
if ($workflow.Contains('allow_unreadable_dependabot')) {
    throw 'Release workflow must not allow an unreadable Dependabot bypass.'
}
if ($gate.Contains('manual override')) {
    throw 'Dependabot gate must not contain a manual override path.'
}

# A draft created by workflow_dispatch must bind to the checkout that was
# validated, not a default branch which may have advanced during the build.
$draftMatch = [regex]::Match($workflow, '(?ms)^      - name: Create draft release\r?\n.*?(?=^      - name:|\z)')
if (-not $draftMatch.Success -or -not $draftMatch.Value.Contains('--target $env:GITHUB_SHA')) {
    throw 'Draft releases must target the exact validated GITHUB_SHA.'
}
if (-not $workflow.Contains('pnpm --dir apps/desktop-tauri audit --audit-level high')) {
    throw 'Release dependency audit must include development dependencies.'
}

Write-Host '[assert-release-workflow] OK - credential, dependency, and release commit policies are enforced'
