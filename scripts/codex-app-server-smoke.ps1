# Real-machine, read-only Codex App Server compatibility smoke.
#
# Resolves Codex through the Rust example, captures generated JSON schemas
# under rust/src/providers/codex/app_server/schema/<slug>/, and opens only a
# CurrentCli session (initialize -> account/read -> rateLimits/read ->
# shutdown). Output is redacted: no email, account id, quota value, token,
# full path, raw RPC line, or environment variable.

param(
    [switch] $SkipSchemaCapture
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $exampleCandidates = @(
        (Join-Path $repoRoot 'target/debug/examples/codex_app_server_smoke.exe'),
        (Join-Path $repoRoot 'rust/target/debug/examples/codex_app_server_smoke.exe')
    )
    $example = $exampleCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $example) {
        throw "Smoke example not built; run cargo build --manifest-path rust/Cargo.toml --example codex_app_server_smoke first."
    }

    $SchemaTemp = Join-Path ([IO.Path]::GetTempPath()) ('codex-barbar-schema-' + [guid]::NewGuid().ToString('N'))
    try {
        if (-not $SkipSchemaCapture) {
            New-Item -ItemType Directory -Path $SchemaTemp -Force | Out-Null
            & $example --generate-schema $SchemaTemp
            if ($LASTEXITCODE -ne 0) {
                throw "App Server schema generation failed (exit $LASTEXITCODE)."
            }
            $schemaFiles = Get-ChildItem -LiteralPath $SchemaTemp -Recurse -File -ErrorAction SilentlyContinue
            if ($schemaFiles.Count -eq 0) {
                throw "App Server schema generation produced no files."
            }
        }

        $summaryJson = & $example 2>$null
        if ($LASTEXITCODE -ne 0) {
            throw "Read-only App Server smoke failed (exit $LASTEXITCODE)."
        }
        $summary = $summaryJson | ConvertFrom-Json
        $summary | ConvertTo-Json -Depth 6

        if (-not $SkipSchemaCapture -and $schemaFiles) {
            $version = [string]$summary.codexVersion
            if ([string]::IsNullOrWhiteSpace($version)) {
                throw "Smoke summary did not include a Codex version for schema capture."
            }
            $slug = ($version.ToCharArray() | ForEach-Object {
                if ($_ -match '[A-Za-z0-9._-]') { $_ } else { '_' }
            }) -join ''
            $schemaDir = Join-Path $repoRoot ("rust/src/providers/codex/app_server/schema/$slug")
            New-Item -ItemType Directory -Path $schemaDir -Force | Out-Null
            $manifest = @{
                codexVersion = $version
                capturedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
                files = @()
            }
            foreach ($file in $schemaFiles) {
                $relative = $file.FullName.Substring($SchemaTemp.Length).TrimStart('\')
                $target = Join-Path $schemaDir $relative
                New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
                Copy-Item -LiteralPath $file.FullName -Destination $target -Force
                $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
                $manifest.files += @{
                    path = $relative
                    sha256 = $hash
                }
            }
            $manifestPath = Join-Path $schemaDir 'manifest.json'
            $manifestJson = $manifest | ConvertTo-Json -Depth 6
            [IO.File]::WriteAllText($manifestPath, $manifestJson, [Text.UTF8Encoding]::new($false))
        }
    } finally {
        if (-not $SkipSchemaCapture -and (Test-Path -LiteralPath $SchemaTemp)) {
            Remove-Item -LiteralPath $SchemaTemp -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
} finally {
    Pop-Location
}
