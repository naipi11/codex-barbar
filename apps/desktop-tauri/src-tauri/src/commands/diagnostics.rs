//! Redacted diagnostics summary and fixed-location export commands.

use std::sync::Mutex;

use codexbar::diagnostics::{Diagnostics, DiagnosticsSummaryDto};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsExportDto {
    pub path: String,
}

fn diagnostics_from_state(state: &AppState) -> Diagnostics {
    let mut diagnostics = Diagnostics {
        codex_version: super::bridge::detect_codex_compatibility().version,
        vault_status: if state.account_service.is_some() {
            "ok".to_string()
        } else {
            "unavailable".to_string()
        },
        ..Default::default()
    };
    if let Some(service) = state.account_service.as_ref() {
        let Ok(snapshot) = service.snapshot() else {
            return diagnostics;
        };
        let identities = service.identity_records().unwrap_or_default();
        for profile in snapshot.profiles {
            let kind_name = match profile.kind {
                codexbar::accounts::model::ProfileKind::CurrentCli => "currentCli",
                codexbar::accounts::model::ProfileKind::Managed => "managed",
            };
            *diagnostics
                .profile_kinds
                .entry(kind_name.to_string())
                .or_insert(0) += 1;
            if let Some(identity) = identities.get(&profile.id) {
                let status_name = match identity.status {
                    codexbar::accounts::identity::AccountStatus::SignedIn => "signedIn",
                    codexbar::accounts::identity::AccountStatus::SignedOut => "signedOut",
                    codexbar::accounts::identity::AccountStatus::Unavailable => "unavailable",
                };
                *diagnostics
                    .account_statuses
                    .entry(status_name.to_string())
                    .or_insert(0) += 1;
            }
            if let Some(last) = profile.last_success_at {
                diagnostics
                    .refresh_times
                    .push(last.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            }
        }
        if let Ok(states) = service.repositories().usage.load_all_states() {
            for state in states {
                if let Some(error) = state.current_error {
                    diagnostics
                        .error_kinds
                        .push(super::error_kind_name(error.kind).to_string());
                    if !error.diagnostic_code.is_empty() {
                        diagnostics.error_codes.push(error.diagnostic_code);
                    }
                }
            }
        }
    }
    diagnostics
}

#[tauri::command]
pub fn get_diagnostics_summary(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<DiagnosticsSummaryDto, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    let diagnostics = diagnostics_from_state(&guard);
    let logs = app_paths_log_dir();
    Ok(diagnostics.summary(logs.as_deref()))
}

#[tauri::command]
pub fn export_diagnostics(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<DiagnosticsExportDto, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    let diagnostics = diagnostics_from_state(&guard);
    let path = diagnostics.export().map_err(|error| error.to_string())?;
    Ok(DiagnosticsExportDto {
        path: path.to_string_lossy().into_owned(),
    })
}

/// Resolve the canonical log directory for summary log tails.
fn app_paths_log_dir() -> Option<std::path::PathBuf> {
    codexbar::app_paths::AppPaths::discover()
        .ok()
        .map(|paths| paths.logs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_dto_carries_only_a_path() {
        let dto = DiagnosticsExportDto {
            path: "%LOCALAPPDATA%\\codex-barbar\\diagnostics\\test.json".to_string(),
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(
            json["path"],
            "%LOCALAPPDATA%\\codex-barbar\\diagnostics\\test.json"
        );
    }
}

#[tauri::command]
pub fn get_status_surface_diagnostics()
-> Vec<crate::shell::surface_lifecycle_trace::SurfaceLifecycleSnapshot> {
    crate::status_surfaces::get_status_surface_diagnostics()
}
