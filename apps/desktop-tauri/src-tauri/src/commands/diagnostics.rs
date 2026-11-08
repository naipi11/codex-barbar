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

fn diagnostics_from_service(
    service: Option<&codexbar::accounts::service::AccountProfileService>,
    proof_mode: bool,
) -> Result<Diagnostics, String> {
    let codex_version = if proof_mode {
        None
    } else {
        let settings = service
            .map(|service| service.repositories().settings.load())
            .transpose()
            .map_err(|_| "SETTINGS_LOAD_FAILED".to_string())?
            .unwrap_or_default();
        let request =
            crate::codex_factory::resolve_request(settings.codex_executable_override.as_deref());
        super::bridge::detect_codex_compatibility(&request).version
    };
    let mut diagnostics = Diagnostics {
        codex_version,
        vault_status: if service.is_some() {
            "ok".to_string()
        } else {
            "unavailable".to_string()
        },
        ..Default::default()
    };
    if let Some(service) = service {
        let Ok(snapshot) = service.snapshot() else {
            return Ok(diagnostics);
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
    Ok(diagnostics)
}

async fn diagnostics_snapshot(
    state: &Mutex<AppState>,
) -> Result<(Diagnostics, Option<codexbar::app_paths::AppPaths>), String> {
    let (service, proof_mode) = {
        let guard = state
            .lock()
            .map_err(|_| "DIAGNOSTICS_STATE_UNAVAILABLE".to_string())?;
        (guard.account_service.clone(), guard.proof_config.is_some())
    };
    // Probe outside both the UI thread and the AppState mutex.
    tauri::async_runtime::spawn_blocking(move || {
        let paths = crate::proof_harness::desktop_app_paths(proof_mode).map_err(str::to_string)?;
        Ok((
            diagnostics_from_service(service.as_deref(), proof_mode)?,
            paths,
        ))
    })
    .await
    .map_err(|_| "DIAGNOSTICS_FAILED".to_string())?
}

#[tauri::command]
pub async fn get_diagnostics_summary(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<DiagnosticsSummaryDto, String> {
    let (diagnostics, paths) = diagnostics_snapshot(&state).await?;
    Ok(diagnostics.summary(paths.as_ref().map(|paths| paths.logs.as_path())))
}

#[tauri::command]
pub async fn export_diagnostics(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<DiagnosticsExportDto, String> {
    let (diagnostics, paths) = diagnostics_snapshot(&state).await?;
    let path = export_to_app_paths(&diagnostics, paths.as_ref())?;
    Ok(DiagnosticsExportDto {
        path: path.to_string_lossy().into_owned(),
    })
}

fn export_to_app_paths(
    diagnostics: &Diagnostics,
    paths: Option<&codexbar::app_paths::AppPaths>,
) -> Result<std::path::PathBuf, String> {
    let Some(paths) = paths else {
        return diagnostics.export();
    };
    // Respect the same proof data root as the repositories. Keep the final
    // secret scan required by the shared exporter before atomic publication.
    diagnostics.export_to(&paths.root.join("diagnostics"), |path| {
        let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        if codexbar::core::SecretRedactor::redact(&content) != content {
            return Err("final scan found unredacted secret material".to_string());
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_diagnostics_does_not_probe_codex() {
        let diagnostics = diagnostics_from_service(None, true).unwrap();
        assert!(diagnostics.codex_version.is_none());
        assert_eq!(diagnostics.vault_status, "unavailable");
    }

    #[test]
    fn diagnostics_export_stays_below_supplied_app_paths() {
        let base =
            std::env::temp_dir().join(format!("codexbar-proof-export-{}", uuid::Uuid::new_v4()));
        let paths = codexbar::app_paths::AppPaths::from_local_app_data(&base);
        let export = export_to_app_paths(&Diagnostics::default(), Some(&paths)).unwrap();
        assert_eq!(
            export.parent(),
            Some(paths.root.join("diagnostics").as_path())
        );
        assert!(export.is_file());
        std::fs::remove_dir_all(base).unwrap();
    }

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
