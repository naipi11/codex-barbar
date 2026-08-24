# Static release-graph guard for the codex-barbar V1 Phase-0 boundary.
#
# Scans the active release files only and rejects forbidden invoke names,
# window labels, plugin permissions, network origins, and private Codex
# endpoints. Runs in scripts/local-check.ps1 and CI.

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
  $expectedCommands = @(
    'get_bootstrap_state','get_settings_snapshot','get_notification_capability','update_settings','apply_menu_preferences','get_usage_spend','send_test_notification','get_locale_strings',
    'select_profile','refresh_selected_profile','start_managed_login','cancel_managed_login',
    'rename_managed_profile','remove_managed_profile','save_profile_avatar','clear_profile_avatar','validate_codex_executable',
    'get_diagnostics_summary','export_diagnostics','check_for_updates','open_release_page',
    'open_codex_usage_page','open_windows_notification_settings','open_settings_window','close_settings_window','dismiss_tray_panel',
    'set_flyout_interacting','set_flyout_size','get_current_surface_state','open_tray_panel','quit_app',
    'set_status_surface_enabled','set_float_ball_expanded','set_taskbar_status_width'
  ) | Sort-Object

  $activeFiles = @(
    'apps/desktop-tauri/src-tauri/src/main.rs',
    'apps/desktop-tauri/src-tauri/src/commands/mod.rs',
    'apps/desktop-tauri/src/lib/tauri.ts',
    'apps/desktop-tauri/src/App.tsx',
    'apps/desktop-tauri/src-tauri/capabilities/default.json',
    'apps/desktop-tauri/src-tauri/tauri.conf.json'
  )
  $forbidden = @(
    'download_update', 'apply_update', 'open_external_url', 'open_path',
    'account/logout', 'manual_cookie', 'api_key', 'token_account', 'floatbar', 'PopOut',
    'global-shortcut', 'telemetry', 'analytics', 'sentry',
    'http://localhost', 'ws://localhost', 'http://127.0.0.1', 'ws://127.0.0.1', '/wham/'
  )
  foreach ($path in $activeFiles) {
    if (-not (Test-Path -LiteralPath $path)) { throw "active release file missing: $path" }
    $text = Get-Content -Raw -Encoding utf8 -LiteralPath $path
    if ($path -eq 'apps/desktop-tauri/src-tauri/tauri.conf.json') {
      # devUrl only applies to `tauri dev`; the release graph never uses it.
      # Strip it before scanning so the production CSP stays localhost-free.
      $text = $text -replace '"devUrl"\s*:\s*"[^"]*"', '"devUrl": ""'
    }
    foreach ($needle in $forbidden) {
      if ($text.Contains($needle)) { throw "$path contains forbidden release token: $needle" }
    }
  }

  # Exact invoke allowlist: parse `commands::<name>` lines from main.rs and
  # require the sorted set to equal the frozen V1 command list.
  $mainText = Get-Content -Raw -Encoding utf8 -LiteralPath 'apps/desktop-tauri/src-tauri/src/main.rs'
  $registered = @(
    [regex]::Matches($mainText, 'commands::([a-z_]+),') |
      ForEach-Object { $_.Groups[1].Value } |
      Where-Object { $_ -ne 'request_graceful_quit' } |
      Sort-Object -Unique
  )
  $commandDiff = Compare-Object -ReferenceObject $expectedCommands -DifferenceObject $registered
  if ($commandDiff) {
    $missing = @($commandDiff | Where-Object SideIndicator -eq '<=')
    $extra = @($commandDiff | Where-Object SideIndicator -eq '=>')
    throw "invoke allowlist mismatch. missing=[$($missing -join ',')] extra=[$($extra -join ',')]"
  }

  $codexProviderRoot = Join-Path $repoRoot 'rust/src/providers/codex'
  if (-not (Test-Path -LiteralPath $codexProviderRoot)) {
    throw "Codex provider source root missing: $codexProviderRoot"
  }
  $privateCodexPatterns = @(
    '/wham/',
    'Authorization:\s*Bearer',
    'reqwest::Client',
    'read_to_string.*auth\.json'
  )
  foreach ($file in Get-ChildItem -LiteralPath $codexProviderRoot -Recurse -File) {
    $text = Get-Content -Raw -Encoding utf8 -LiteralPath $file.FullName
    foreach ($pattern in $privateCodexPatterns) {
      if ($text -match "(?is)$pattern") {
        $relative = $file.FullName.Substring($repoRoot.Length).TrimStart('\', '/')
        throw "$relative contains forbidden private Codex implementation pattern: $pattern"
      }
    }
  }
  Write-Host '[assert-v1-boundaries] OK - no forbidden tokens in active release files'
} finally {
  Pop-Location
}
