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

Write-Host '[assert-release-workflow] OK - Dependabot release credential policy is enforced'
