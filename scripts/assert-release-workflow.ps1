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
$prWorkflowPath = Join-Path $repoRoot '.github\workflows\pr-check.yml'
if (-not (Test-Path -LiteralPath $prWorkflowPath)) {
    throw "PR workflow is missing: $prWorkflowPath"
}
$prWorkflow = Get-Content -Raw -Encoding utf8 -LiteralPath $prWorkflowPath

if ($workflow -notmatch '(?ms)^permissions:\r?\n  contents: read\r?$') {
    throw 'Release workflow default permissions must be contents: read.'
}

foreach ($fragment in @('windows-build:', 'linux-build:', 'publish:')) {
    if (-not $workflow.Contains($fragment)) {
        throw "Release workflow must define job: $fragment"
    }
}
if (-not $workflow.Contains('runs-on: windows-2025')) {
    throw 'Release workflow must retain the Windows build runner.'
}
if (-not $workflow.Contains('runs-on: ubuntu-24.04')) {
    throw 'Release workflow must build Ubuntu artifacts on ubuntu-24.04.'
}
if (-not $prWorkflow.Contains('linux-check:') -or -not $prWorkflow.Contains('runs-on: ubuntu-24.04')) {
    throw 'PR workflow must include the ubuntu-24.04 linux-check job.'
}

$publishMatch = [regex]::Match($workflow, '(?ms)^  publish:\r?\n.*?(?=^  \S|\z)')
if (-not $publishMatch.Success) {
    throw 'Release workflow has no publish job.'
}
$publish = $publishMatch.Value
if ($publish -notmatch '(?ms)^  publish:\r?\n.*?^    permissions:\r?\n      contents: write\r?$') {
    throw 'Publish must be the only job granted contents: write.'
}
if ([regex]::Matches($workflow, '(?m)^\s+contents: write$').Count -ne 1) {
    throw 'No job other than publish may be granted contents: write.'
}
foreach ($buildJob in @('windows-build', 'linux-build')) {
    $buildMatch = [regex]::Match($workflow, "(?ms)^  $($buildJob):\r?\n.*?(?=^  \S|\z)")
    if (-not $buildMatch.Success -or -not $buildMatch.Value.Contains('persist-credentials: false')) {
        throw "$buildJob checkout must disable persisted credentials."
    }
}
if ($publish -notmatch '(?m)^    needs:\s*\[windows-build, linux-build\]$') {
    throw 'Publish must require both windows-build and linux-build.'
}
foreach ($fragment in @(
    'name: windows-release',
    'path: artifacts/windows-release',
    'name: linux-release',
    'path: artifacts/linux-release',
    'aggregate-release-assets.mjs --version',
    '--windows artifacts/windows-release',
    '--linux artifacts/linux-release',
    '--output artifacts/aggregate-release'
)) {
    if (-not $publish.Contains($fragment)) {
        throw "Publish job must contain: $fragment"
    }
}

$releaseCreates = [regex]::Matches($workflow, 'gh release create')
if ($releaseCreates.Count -ne 1 -or -not $publish.Contains('gh release create')) {
    throw 'Only the publish job may create exactly one GitHub release.'
}

if ($workflow -match '(?m)^\s*\$resolved\s*=\s*"?\$\{\{ inputs\.version \}\}"?$' -or $workflow -match "(?m)^\s*resolved='\$\{\{ inputs\.version \}\}'$") {
    throw 'Workflow-dispatch version must not be interpolated into shell source.'
}
foreach ($fragment in @(
    'REQUESTED_VERSION: ${{ inputs.version }}',
    '$resolved = $env:REQUESTED_VERSION',
    'resolved="$REQUESTED_VERSION"',
    "if (`$resolved -notmatch '^\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?`$')",
    '[[ "$resolved" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-((alpha|beta|rc)\.[0-9]+))?$ ]]'
)) {
    if (-not $workflow.Contains($fragment)) {
        throw "Version resolver must contain safe validation: $fragment"
    }
}

$gateMatch = [regex]::Match(
    $workflow,
    '(?ms)^      - name: Dependabot alert gate\r?\n.*?(?=^      - name:|\z)'
)
if (-not $gateMatch.Success) {
    throw 'Release workflow has no Dependabot alert gate.'
}

$gate = $gateMatch.Value
if ([regex]::Matches($workflow, '(?m)^      - name: Dependabot alert gate$').Count -ne 1 -or -not $publish.Contains($gate)) {
    throw 'Dependabot alert gate must run exactly once in publish.'
}
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

Write-Host '[assert-release-workflow] OK - dual-platform release and Dependabot credential policies are enforced'
