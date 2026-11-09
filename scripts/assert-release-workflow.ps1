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

# Keep the two event routes for the same version in one non-cancelling group.
if (-not $workflow.Contains("format('v{0}', inputs.version)") -or
    -not $workflow.Contains('cancel-in-progress: false')) {
    throw 'Release publication must serialize manual and tag-triggered runs.'
}
if ($draftMatch.Value.Contains('--clobber') -or $draftMatch.Value.Contains('release upload')) {
    throw 'The publication step must not replace existing release assets.'
}

# Exercise the exact pure policy embedded in the workflow, without calling gh,
# accessing credentials, creating tags, or modifying any release.
$policyMatch = [regex]::Match($workflow, '(?ms)^          # BEGIN release-idempotency-policy\r?\n(.*?)^          # END release-idempotency-policy')
if (-not $policyMatch.Success) { throw 'Release idempotency policy is missing.' }
$policySource = $policyMatch.Groups[1].Value -replace '(?m)^ {10}', ''
. ([scriptblock]::Create($policySource))
$commit = 'a' * 40
$otherCommit = 'b' * 40
$tag = 'v1.2.3'
$draft = [pscustomobject]@{ tag_name = $tag; draft = $true; target_commitish = $commit }
$published = [pscustomobject]@{ tag_name = $tag; draft = $false; target_commitish = 'main' }
$wrongDraft = [pscustomobject]@{ tag_name = $tag; draft = $true; target_commitish = $otherCommit }
$movingDraft = [pscustomobject]@{ tag_name = $tag; draft = $true; target_commitish = 'main' }
$unrelated = [pscustomobject]@{ tag_name = 'v0.0.1'; draft = $true; target_commitish = $otherCommit }
$cases = @(
    @{ Name = 'new release'; Releases = @(); TagCommit = ''; Want = 'create'; Throws = $false }
    @{ Name = 'new release for existing matching tag'; Releases = @(); TagCommit = $commit; Want = 'create'; Throws = $false }
    @{ Name = 'existing exact draft'; Releases = @($draft); TagCommit = ''; Want = 'skip'; Throws = $false }
    @{ Name = 'existing tagged draft'; Releases = @($draft); TagCommit = $commit; Want = 'skip'; Throws = $false }
    @{ Name = 'published immutable release'; Releases = @($published); TagCommit = $commit; Want = 'skip'; Throws = $false }
    @{ Name = 'unrelated release is ignored'; Releases = @($unrelated); TagCommit = ''; Want = 'create'; Throws = $false }
    @{ Name = 'tag collision'; Releases = @(); TagCommit = $otherCommit; Want = ''; Throws = $true }
    @{ Name = 'published tag missing'; Releases = @($published); TagCommit = ''; Want = ''; Throws = $true }
    @{ Name = 'draft commit mismatch'; Releases = @($wrongDraft); TagCommit = ''; Want = ''; Throws = $true }
    @{ Name = 'moving draft target'; Releases = @($movingDraft); TagCommit = ''; Want = ''; Throws = $true }
    @{ Name = 'duplicate draft and published release'; Releases = @($draft, $published); TagCommit = $commit; Want = ''; Throws = $true }
    @{ Name = 'duplicate drafts'; Releases = @($draft, $draft); TagCommit = ''; Want = ''; Throws = $true }
)
foreach ($case in $cases) {
    $threw = $false
    $actual = $null
    try {
        $actual = Get-ReleaseDraftAction -Releases $case.Releases -Tag $tag -Commit $commit -TagCommit $case.TagCommit
    } catch { $threw = $true }
    if ($threw -ne $case.Throws -or (-not $threw -and $actual -cne $case.Want)) {
        throw "Release policy regression: $($case.Name)"
    }
}
$threw = $false
try { Get-ReleaseDraftAction -Releases @() -Tag $tag -Commit 'main' -TagCommit '' | Out-Null } catch { $threw = $true }
if (-not $threw) { throw 'Release policy must reject a moving source ref.' }
$rows = @(
    '{"tag_name":"v1.2.3","draft":false,"target_commitish":"main"}'
    '{"tag_name":"v0.0.1","draft":true,"target_commitish":"main"}'
)
$flattened = @($rows | ForEach-Object { $_ | ConvertFrom-Json })
if ($flattened.Count -ne 2 -or (Get-ReleaseDraftAction -Releases $flattened -Tag $tag -Commit $commit -TagCommit $commit) -ne 'skip') {
    throw 'Projected release rows from every page must be evaluated together.'
}
Write-Host '[assert-release-workflow] OK - credentials, commit binding, immutable assets, and 14 idempotency regressions passed'
