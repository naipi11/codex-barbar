//! Desktop Codex resolution always follows the latest persisted settings.

use std::path::PathBuf;

use codexbar::core::{AppError, AppErrorKind, RecoveryAction};
use codexbar::providers::codex::app_server::{LocalAppServerFactory, ResolveRequest};
use codexbar::storage::SettingsRepository;

/// Do not capture an override at startup: Settings can change while the app is
/// running, and both Current CLI and managed sessions must see the new value.
pub(crate) fn settings_backed_factory(repository: SettingsRepository) -> LocalAppServerFactory {
    LocalAppServerFactory::with_request_source(move || request_from_repository(&repository))
}

fn request_from_repository(repository: &SettingsRepository) -> Result<ResolveRequest, AppError> {
    let settings = repository.load().map_err(|_| {
        AppError::new(
            AppErrorKind::StorageFailure,
            "errors.settingsLoadFailed",
            RecoveryAction::Retry,
            "SETTINGS_LOAD_FAILED",
        )
    })?;
    Ok(resolve_request(
        settings.codex_executable_override.as_deref(),
    ))
}

/// Shared by session creation, bootstrap compatibility, and diagnostics. An
/// invalid configured override is deliberately not discarded in favor of PATH.
pub(crate) fn resolve_request(override_path: Option<&str>) -> ResolveRequest {
    ResolveRequest {
        override_path: override_path.map(PathBuf::from),
        path: std::env::var_os("PATH"),
        pathext: std::env::var_os("PATHEXT"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codexbar::storage::{AppDatabase, SettingsPatch};
    use std::sync::Arc;

    #[test]
    fn request_reads_updated_override_and_can_return_to_discovery() {
        let dir =
            std::env::temp_dir().join(format!("codexbar-factory-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database = Arc::new(AppDatabase::open(&dir.join("settings.db")).unwrap());
        let repository = SettingsRepository::new(database);
        assert!(
            request_from_repository(&repository)
                .unwrap()
                .override_path
                .is_none()
        );

        for name in ["first.exe", "second.cmd"] {
            let path = dir.join(name);
            repository
                .update(SettingsPatch {
                    codex_executable_override: Some(Some(path.to_string_lossy().into_owned())),
                    ..SettingsPatch::default()
                })
                .unwrap();
            assert_eq!(
                request_from_repository(&repository).unwrap().override_path,
                Some(path)
            );
        }

        repository
            .update(SettingsPatch {
                codex_executable_override: Some(None),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert!(
            request_from_repository(&repository)
                .unwrap()
                .override_path
                .is_none()
        );
        drop(repository);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn factory_reloads_settings_for_each_current_and_managed_session() {
        use codexbar::providers::codex::app_server::AppServerFactory;

        let dir =
            std::env::temp_dir().join(format!("codexbar-factory-open-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database = Arc::new(AppDatabase::open(&dir.join("settings.db")).unwrap());
        let repository = SettingsRepository::new(database);
        let factory = settings_backed_factory(repository.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        for (path, code) in [
            ("relative.exe".to_string(), "CODEX_OVERRIDE_NOT_ABSOLUTE"),
            (
                dir.join("missing.exe").to_string_lossy().into_owned(),
                "CODEX_OVERRIDE_NOT_USABLE",
            ),
        ] {
            repository
                .update(SettingsPatch {
                    codex_executable_override: Some(Some(path)),
                    ..SettingsPatch::default()
                })
                .unwrap();
            // Both errors happen before any process launch or runtime-home access.
            let current = runtime
                .block_on(factory.open_current_cli())
                .err()
                .expect("invalid override");
            assert_eq!(current.diagnostic_code, code);
            let managed = runtime
                .block_on(factory.open_managed(&dir.join("unused-home")))
                .err()
                .expect("invalid override");
            assert_eq!(managed.diagnostic_code, code);
        }
        drop(factory);
        drop(repository);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_override_is_forwarded_without_falling_back_to_path() {
        let request = resolve_request(Some("relative-codex.exe"));
        assert_eq!(
            request.override_path,
            Some(PathBuf::from("relative-codex.exe"))
        );
        let error = codexbar::providers::codex::app_server::CodexCommandResolver::new()
            .resolve(&request)
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "CODEX_OVERRIDE_NOT_ABSOLUTE");
    }
}
