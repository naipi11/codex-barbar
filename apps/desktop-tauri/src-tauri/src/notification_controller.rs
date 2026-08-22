use std::time::Duration;

use codexbar::{
    core::{ProfileId, ProfileUsageState},
    notifications::v1::{V1NotificationEngine, V1NotificationEvent},
    storage::{AppSettings, LanguagePreference, SettingsRepository},
    update_check::{ManualUpdateChecker, ManualUpdateResult},
};
use tauri::Manager;

use crate::state::AppState;

const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub trait ToastSink: Send {
    fn send(&mut self, title: &str, body: &str, play_sound: bool) -> Result<(), String>;
}

pub struct WindowsToastSink;

impl ToastSink for WindowsToastSink {
    fn send(&mut self, title: &str, body: &str, play_sound: bool) -> Result<(), String> {
        send_windows_toast(title, body, play_sound)
    }
}

pub struct NotificationController<S: ToastSink> {
    engine: V1NotificationEngine,
    sink: S,
    last_observed_update_version: Option<String>,
}

impl<S: ToastSink> NotificationController<S> {
    pub fn new(engine: V1NotificationEngine, sink: S) -> Self {
        Self {
            engine,
            sink,
            last_observed_update_version: None,
        }
    }

    pub fn observe_usage(
        &mut self,
        repository: &SettingsRepository,
        profile_id: ProfileId,
        state: &ProfileUsageState,
        reset_credits: Option<u64>,
    ) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let events =
            self.engine
                .observe_usage(&settings.notifications, profile_id, state, reset_credits);
        self.dispatch(&settings, events)
    }

    pub fn observe_refresh(
        &mut self,
        repository: &SettingsRepository,
        profile_id: ProfileId,
        success: bool,
    ) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let events = self
            .engine
            .observe_refresh(&settings.notifications, profile_id, success);
        self.dispatch(&settings, events)
    }

    pub fn observe_update_available(
        &mut self,
        repository: &SettingsRepository,
        version: &str,
    ) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let is_new = self.last_observed_update_version.as_deref() != Some(version);
        self.last_observed_update_version = Some(version.to_string());
        if !is_new
            || !settings.notifications.enabled
            || !settings.notifications.update_available_enabled
        {
            return Ok(());
        }
        self.dispatch(
            &settings,
            vec![V1NotificationEvent::UpdateAvailable {
                version: version.to_string(),
            }],
        )
    }

    pub fn send_test(&mut self, repository: &SettingsRepository) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let (title, body) = test_copy(&settings);
        self.sink
            .send(title, body, settings.notifications.play_sound)
            .map_err(|_| "NOTIFICATION_TEST_FAILED".to_string())
    }

    fn dispatch(
        &mut self,
        settings: &AppSettings,
        events: Vec<V1NotificationEvent>,
    ) -> Result<(), String> {
        for event in events {
            let (title, body) = event_copy(settings, &event);
            self.sink
                .send(&title, &body, settings.notifications.play_sound)
                .map_err(|_| "NOTIFICATION_DISPATCH_FAILED".to_string())?;
        }
        Ok(())
    }
}

fn load_settings(repository: &SettingsRepository) -> Result<AppSettings, String> {
    repository
        .load()
        .map_err(|_| "NOTIFICATION_SETTINGS_UNAVAILABLE".to_string())
}

fn use_chinese(settings: &AppSettings) -> bool {
    match settings.language {
        LanguagePreference::ZhCn => true,
        LanguagePreference::EnUs => false,
        LanguagePreference::System => {
            codexbar::platform::windows::system_locale::default_language() == "zh-CN"
        }
    }
}

fn test_copy(settings: &AppSettings) -> (&'static str, &'static str) {
    if use_chinese(settings) {
        (
            "codex-barbar 测试通知",
            "Windows 通知已连接；此操作不会更改 Codex 用量或账户。",
        )
    } else {
        (
            "codex-barbar Test Notification",
            "Windows notifications are connected. This does not change Codex usage or your account.",
        )
    }
}

fn event_copy(settings: &AppSettings, event: &V1NotificationEvent) -> (String, String) {
    let zh = use_chinese(settings);
    match event {
        V1NotificationEvent::Warning { remaining_percent } => (
            if zh { "用量预警" } else { "Quota warning" }.to_string(),
            if zh {
                format!("通用每周额度剩余 {remaining_percent}%。")
            } else {
                format!("Universal weekly allowance has {remaining_percent}% remaining.")
            },
        ),
        V1NotificationEvent::Danger { remaining_percent } => (
            if zh { "用量危险" } else { "Quota danger" }.to_string(),
            if zh {
                format!("通用每周额度仅剩 {remaining_percent}%。")
            } else {
                format!("Universal weekly allowance has only {remaining_percent}% remaining.")
            },
        ),
        V1NotificationEvent::WeeklyReset => (
            if zh {
                "每周额度已重置"
            } else {
                "Weekly allowance reset"
            }
            .to_string(),
            if zh {
                "通用每周额度已进入新的周期。".to_string()
            } else {
                "The universal weekly allowance started a new cycle.".to_string()
            },
        ),
        V1NotificationEvent::ResetCreditsIncreased { .. } => (
            if zh {
                "重置额度已增加"
            } else {
                "Reset credits increased"
            }
            .to_string(),
            if zh {
                "有更多可用重置额度。".to_string()
            } else {
                "More reset credits are available.".to_string()
            },
        ),
        V1NotificationEvent::RefreshFailed => (
            if zh {
                "刷新连续失败"
            } else {
                "Refresh repeatedly failed"
            }
            .to_string(),
            if zh {
                "codex-barbar 连续三次无法刷新用量。".to_string()
            } else {
                "codex-barbar could not refresh usage three times in a row.".to_string()
            },
        ),
        V1NotificationEvent::RefreshRecovered => (
            if zh {
                "刷新已恢复"
            } else {
                "Refresh recovered"
            }
            .to_string(),
            if zh {
                "codex-barbar 已恢复用量刷新。".to_string()
            } else {
                "codex-barbar is refreshing usage again.".to_string()
            },
        ),
        V1NotificationEvent::UpdateAvailable { version } => (
            if zh {
                "有可用更新"
            } else {
                "Update available"
            }
            .to_string(),
            if zh {
                format!("codex-barbar {version} 已发布。")
            } else {
                format!("codex-barbar {version} is available.")
            },
        ),
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "windows")]
fn send_windows_toast(title: &str, body: &str, play_sound: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let audio = if play_sound {
        ""
    } else {
        r#"<audio silent="true"/>"#
    };
    let script = format!(
        r#"try {{
    $appIdPath = 'HKCU:\SOFTWARE\Classes\AppUserModelId\CodexBar'
    New-Item -Path $appIdPath -Force | Out-Null
    New-ItemProperty -Path $appIdPath -Name DisplayName -Value 'codex-barbar' -PropertyType String -Force | Out-Null
    [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
    [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
    $template = @'
<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{}</toast>
'@
    $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
    $xml.LoadXml($template)
    $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
    $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('CodexBar')
    if ($null -eq $notifier) {{ exit 1 }}
    $notifier.Show($toast)
}} catch {{ exit 1 }}"#,
        xml_escape(title),
        xml_escape(body),
        audio
    );

    let status = Command::new("powershell")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|_| "NOTIFICATION_TRANSPORT_UNAVAILABLE".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("NOTIFICATION_TRANSPORT_FAILED".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn send_windows_toast(_title: &str, _body: &str, _play_sound: bool) -> Result<(), String> {
    Err("NOTIFICATION_TRANSPORT_UNAVAILABLE".to_string())
}

fn repository_from_app(app: &tauri::AppHandle) -> Option<SettingsRepository> {
    app.state::<std::sync::Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| {
            state
                .account_service
                .as_ref()
                .map(|service| service.repositories().settings.clone())
        })
}

fn should_check_for_updates(settings: &AppSettings) -> bool {
    settings.notifications.enabled && settings.notifications.update_available_enabled
}

pub fn start_update_check_loop(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(UPDATE_CHECK_INTERVAL).await;
            if crate::proof_harness::is_proof_mode(&app) {
                continue;
            }
            let Some(repository) = repository_from_app(&app) else {
                continue;
            };
            let Ok(settings) = repository.load() else {
                continue;
            };
            if !should_check_for_updates(&settings) {
                continue;
            }
            let Ok(ManualUpdateResult::Available { latest_version, .. }) =
                ManualUpdateChecker::new().check().await
            else {
                continue;
            };
            let controller =
                app.state::<std::sync::Mutex<NotificationController<WindowsToastSink>>>();
            if let Ok(mut controller) = controller.lock()
                && controller
                    .observe_update_available(&repository, &latest_version)
                    .is_err()
            {
                tracing::warn!(
                    code = "NOTIFICATION_UPDATE_DISPATCH_FAILED",
                    "update notification was not delivered"
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::{DateTime, Utc};
    use codexbar::{
        app_paths::AppPaths,
        core::{
            Freshness, ProfileId, ProfileUsageSnapshot, ProfileUsageState, RefreshStatus,
            UsageSource, UsageWindow,
        },
        notifications::v1::V1NotificationEngine,
        storage::{AppDatabase, NotificationPreferencesPatch, SettingsPatch, SettingsRepository},
    };
    use uuid::Uuid;

    use super::{NotificationController, ToastSink, should_check_for_updates, xml_escape};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SentToast {
        title: String,
        body: String,
        play_sound: bool,
    }

    #[derive(Clone)]
    struct FakeToastSink {
        sent: Arc<Mutex<Vec<SentToast>>>,
    }

    impl ToastSink for FakeToastSink {
        fn send(&mut self, title: &str, body: &str, play_sound: bool) -> Result<(), String> {
            self.sent.lock().unwrap().push(SentToast {
                title: title.to_string(),
                body: body.to_string(),
                play_sound,
            });
            Ok(())
        }
    }

    struct FailingToastSink;

    impl ToastSink for FailingToastSink {
        fn send(&mut self, _title: &str, _body: &str, _play_sound: bool) -> Result<(), String> {
            Err("raw transport details must stay private".to_string())
        }
    }

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("codexbar-notification-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (
        TestDir,
        SettingsRepository,
        NotificationController<FakeToastSink>,
        Arc<Mutex<Vec<SentToast>>>,
    ) {
        let temp = TestDir::new();
        let paths = AppPaths::from_local_app_data(&temp.0);
        let database = Arc::new(AppDatabase::open(&paths.database).unwrap());
        let repository = SettingsRepository::new(database);
        let sent = Arc::new(Mutex::new(Vec::new()));
        let controller = NotificationController::new(
            V1NotificationEngine::load(&paths),
            FakeToastSink {
                sent: Arc::clone(&sent),
            },
        );
        (temp, repository, controller, sent)
    }

    fn weekly(profile_id: ProfileId, used: f64) -> ProfileUsageState {
        let reset = DateTime::<Utc>::from_timestamp(1_752_000_000, 0).unwrap();
        ProfileUsageState {
            profile_id,
            snapshot: Some(ProfileUsageSnapshot {
                profile_id,
                plan_type: None,
                primary: None,
                secondary: Some(
                    UsageWindow::normalized("weekly", None, used, Some(10_080), Some(reset), None)
                        .0,
                ),
                additional_windows: Vec::new(),
                fetched_at: reset,
                source: UsageSource::AppServer,
                protocol_anomaly: false,
            }),
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }
    }

    fn enable_notifications(repository: &SettingsRepository) {
        repository
            .update(SettingsPatch {
                language: Some(codexbar::storage::LanguagePreference::EnUs),
                notifications: Some(NotificationPreferencesPatch {
                    enabled: Some(true),
                    ..NotificationPreferencesPatch::default()
                }),
                ..SettingsPatch::default()
            })
            .unwrap();
    }

    #[test]
    fn reloads_persisted_preferences_and_dispatches_only_after_master_is_enabled() {
        let (_temp, repository, mut controller, sent) = fixture();
        let profile_id = Uuid::new_v4();

        controller
            .observe_usage(&repository, profile_id, &weekly(profile_id, 20.0), None)
            .unwrap();
        controller
            .observe_usage(&repository, profile_id, &weekly(profile_id, 40.0), None)
            .unwrap();
        assert!(sent.lock().unwrap().is_empty());

        enable_notifications(&repository);
        controller
            .observe_usage(&repository, profile_id, &weekly(profile_id, 20.0), None)
            .unwrap();
        controller
            .observe_usage(&repository, profile_id, &weekly(profile_id, 40.0), None)
            .unwrap();

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].body.contains("60%"));
        assert!(sent[0].play_sound);
    }

    #[test]
    fn test_toast_does_not_change_reset_credit_observation_state() {
        let (_temp, repository, mut controller, sent) = fixture();
        enable_notifications(&repository);
        let profile_id = Uuid::new_v4();
        controller
            .observe_usage(&repository, profile_id, &weekly(profile_id, 20.0), Some(3))
            .unwrap();

        controller.send_test(&repository).unwrap();
        controller
            .observe_usage(&repository, profile_id, &weekly(profile_id, 20.0), Some(3))
            .unwrap();

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].title.contains("Test"));
    }

    #[test]
    fn newly_observed_update_version_dispatches_exactly_once() {
        let (_temp, repository, mut controller, sent) = fixture();
        enable_notifications(&repository);

        controller
            .observe_update_available(&repository, "v9.9.9")
            .unwrap();
        controller
            .observe_update_available(&repository, "v9.9.9")
            .unwrap();

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].body.contains("v9.9.9"));
    }

    #[test]
    fn periodic_update_checks_require_both_master_and_event_preferences() {
        let mut settings = codexbar::storage::AppSettings::default();
        assert!(!should_check_for_updates(&settings));

        settings.notifications.enabled = true;
        assert!(should_check_for_updates(&settings));

        settings.notifications.update_available_enabled = false;
        assert!(!should_check_for_updates(&settings));
    }

    #[test]
    fn test_action_redacts_transport_failures() {
        let temp = TestDir::new();
        let paths = AppPaths::from_local_app_data(&temp.0);
        let database = Arc::new(AppDatabase::open(&paths.database).unwrap());
        let repository = SettingsRepository::new(database);
        let mut controller =
            NotificationController::new(V1NotificationEngine::load(&paths), FailingToastSink);

        assert_eq!(
            controller.send_test(&repository).unwrap_err(),
            "NOTIFICATION_TEST_FAILED"
        );
    }

    #[test]
    fn toast_text_is_xml_escaped_before_transport() {
        assert_eq!(xml_escape("<&>\"'"), "&lt;&amp;&gt;&quot;&apos;");
    }
}
