use std::time::Duration;

use codexbar::{
    core::{ProfileId, ProfileUsageState},
    notifications::v1::{V1NotificationEngine, V1NotificationEvent},
    storage::{AppSettings, LanguagePreference, SettingsRepository},
    update_check::{ManualUpdateChecker, ManualUpdateResult},
};
use serde::Serialize;
use tauri::Manager;

use crate::state::AppState;

const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const APP_NOTIFICATION_SETTINGS_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\CodexBar";
const GLOBAL_NOTIFICATION_SETTINGS_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\PushNotifications";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationCapabilityStatus {
    Available,
    AppDisabled,
    GlobalDisabled,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCapabilityDto {
    pub status: NotificationCapabilityStatus,
    pub can_open_settings: bool,
}

trait NotificationRegistryReader {
    fn read_dword(&self, key: &str, value: &str) -> Result<Option<u32>, ()>;
}

struct SystemNotificationRegistry;

#[cfg(target_os = "windows")]
impl NotificationRegistryReader for SystemNotificationRegistry {
    fn read_dword(&self, key: &str, value: &str) -> Result<Option<u32>, ()> {
        use std::io::ErrorKind;
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = match hkcu.open_subkey_with_flags(key, KEY_READ) {
            Ok(key) => key,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(()),
        };
        match key.get_value::<u32, _>(value) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(_) => Err(()),
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl NotificationRegistryReader for SystemNotificationRegistry {
    fn read_dword(&self, _key: &str, _value: &str) -> Result<Option<u32>, ()> {
        Err(())
    }
}

fn detect_notification_capability<R: NotificationRegistryReader>(
    reader: &R,
    is_windows: bool,
) -> NotificationCapabilityDto {
    if !is_windows {
        return NotificationCapabilityDto {
            status: NotificationCapabilityStatus::Unsupported,
            can_open_settings: false,
        };
    }

    let app_enabled = match reader.read_dword(APP_NOTIFICATION_SETTINGS_KEY, "Enabled") {
        Ok(value) => value,
        Err(()) => {
            return NotificationCapabilityDto {
                status: NotificationCapabilityStatus::Unsupported,
                can_open_settings: true,
            };
        }
    };
    if app_enabled == Some(0) {
        return NotificationCapabilityDto {
            status: NotificationCapabilityStatus::AppDisabled,
            can_open_settings: true,
        };
    }

    let global_enabled = match reader.read_dword(GLOBAL_NOTIFICATION_SETTINGS_KEY, "ToastEnabled") {
        Ok(value) => value,
        Err(()) => {
            return NotificationCapabilityDto {
                status: NotificationCapabilityStatus::Unsupported,
                can_open_settings: true,
            };
        }
    };
    NotificationCapabilityDto {
        status: if global_enabled == Some(0) {
            NotificationCapabilityStatus::GlobalDisabled
        } else {
            NotificationCapabilityStatus::Available
        },
        can_open_settings: true,
    }
}

pub fn notification_capability() -> NotificationCapabilityDto {
    detect_notification_capability(&SystemNotificationRegistry, cfg!(target_os = "windows"))
}

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
}

impl<S: ToastSink> NotificationController<S> {
    pub fn new(engine: V1NotificationEngine, sink: S) -> Self {
        Self { engine, sink }
    }

    pub fn observe_usage(
        &mut self,
        repository: &SettingsRepository,
        profile_id: ProfileId,
        account_marker: Option<&str>,
        state: &ProfileUsageState,
        reset_credits: Option<u64>,
    ) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let events = self.engine.observe_usage_for_account(
            &settings.notifications,
            profile_id,
            account_marker,
            state,
            reset_credits,
        );
        self.dispatch(&settings, events)
    }

    pub fn observe_refresh(
        &mut self,
        repository: &SettingsRepository,
        profile_id: ProfileId,
        account_marker: Option<&str>,
        success: bool,
    ) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let events = self.engine.observe_refresh_for_account(
            &settings.notifications,
            profile_id,
            account_marker,
            success,
        );
        self.dispatch(&settings, events)
    }

    pub fn observe_account_service_event(
        &mut self,
        repository: &SettingsRepository,
        account_marker: Option<&str>,
        event: &codexbar::accounts::model::AccountServiceEvent,
    ) -> Result<bool, String> {
        let codexbar::accounts::model::AccountServiceEvent::RefreshCompleted {
            profile_id,
            success,
        } = event
        else {
            return Ok(false);
        };
        self.observe_refresh(repository, *profile_id, account_marker, *success)?;
        Ok(true)
    }

    pub fn observe_update_available(
        &mut self,
        repository: &SettingsRepository,
        version: &str,
    ) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let events = self
            .engine
            .observe_update_available(&settings.notifications, version);
        self.dispatch(&settings, events)
    }

    pub fn send_test(&mut self, repository: &SettingsRepository) -> Result<(), String> {
        let settings = load_settings(repository)?;
        let (title, body) = test_copy(&settings);
        self.sink
            .send(title, body, settings.notifications.play_sound)
            .map_err(|error| map_sink_error(error, "NOTIFICATION_TEST_FAILED"))
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
                .map_err(|error| map_sink_error(error, "NOTIFICATION_DISPATCH_FAILED"))?;
        }
        Ok(())
    }
}

fn map_sink_error(error: String, fallback: &str) -> String {
    if error == "NOTIFICATION_PERMISSION_DISABLED" {
        error
    } else {
        fallback.to_string()
    }
}

fn load_settings(repository: &SettingsRepository) -> Result<AppSettings, String> {
    repository
        .load()
        .map_err(|_| "NOTIFICATION_SETTINGS_UNAVAILABLE".to_string())
}

fn account_marker_from_email(email: Option<&str>) -> Option<String> {
    let normalized_email = email?.trim().to_ascii_lowercase();
    if normalized_email.is_empty() {
        None
    } else {
        Some(codexbar::core::sha256_hex(normalized_email.as_bytes()))
    }
}

pub(crate) fn account_marker_for_profile(
    service: &codexbar::accounts::service::AccountProfileService,
    profile_id: ProfileId,
) -> Option<String> {
    let identity = service.identity_for(profile_id).ok().flatten()?;
    account_marker_from_email(identity.email.as_deref())
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

fn send_windows_toast_with<R, F>(reader: &R, is_windows: bool, transport: F) -> Result<(), String>
where
    R: NotificationRegistryReader,
    F: FnOnce() -> Result<(), String>,
{
    match detect_notification_capability(reader, is_windows).status {
        NotificationCapabilityStatus::Available => transport(),
        NotificationCapabilityStatus::AppDisabled
        | NotificationCapabilityStatus::GlobalDisabled => {
            Err("NOTIFICATION_PERMISSION_DISABLED".to_string())
        }
        NotificationCapabilityStatus::Unsupported => {
            Err("NOTIFICATION_TRANSPORT_UNAVAILABLE".to_string())
        }
    }
}

#[cfg(target_os = "windows")]
fn send_windows_toast(title: &str, body: &str, play_sound: bool) -> Result<(), String> {
    send_windows_toast_with(&SystemNotificationRegistry, true, || {
        run_windows_toast_transport(title, body, play_sound)
    })
}

#[cfg(target_os = "windows")]
fn run_windows_toast_transport(title: &str, body: &str, play_sound: bool) -> Result<(), String> {
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
    send_windows_toast_with(&SystemNotificationRegistry, false, || Ok(()))
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

    use super::{
        APP_NOTIFICATION_SETTINGS_KEY, GLOBAL_NOTIFICATION_SETTINGS_KEY,
        NotificationCapabilityStatus, NotificationController, NotificationRegistryReader,
        ToastSink, account_marker_from_email, detect_notification_capability,
        send_windows_toast_with, should_check_for_updates, xml_escape,
    };

    #[derive(Default)]
    struct FakeNotificationRegistry {
        app_enabled: Option<u32>,
        global_enabled: Option<u32>,
    }

    impl NotificationRegistryReader for FakeNotificationRegistry {
        fn read_dword(&self, key: &str, value: &str) -> Result<Option<u32>, ()> {
            assert_eq!(
                value,
                if key == APP_NOTIFICATION_SETTINGS_KEY {
                    "Enabled"
                } else {
                    "ToastEnabled"
                }
            );
            match key {
                APP_NOTIFICATION_SETTINGS_KEY => Ok(self.app_enabled),
                GLOBAL_NOTIFICATION_SETTINGS_KEY => Ok(self.global_enabled),
                _ => panic!("unexpected registry key: {key}"),
            }
        }
    }

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

    struct DisabledToastSink;

    impl ToastSink for DisabledToastSink {
        fn send(&mut self, _title: &str, _body: &str, _play_sound: bool) -> Result<(), String> {
            Err("NOTIFICATION_PERMISSION_DISABLED".to_string())
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
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 20.0),
                None,
            )
            .unwrap();
        controller
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 40.0),
                None,
            )
            .unwrap();
        assert!(sent.lock().unwrap().is_empty());

        enable_notifications(&repository);
        controller
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 20.0),
                None,
            )
            .unwrap();
        controller
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 40.0),
                None,
            )
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
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 20.0),
                Some(3),
            )
            .unwrap();

        controller.send_test(&repository).unwrap();
        controller
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 20.0),
                Some(3),
            )
            .unwrap();

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].title.contains("Test"));
    }

    #[test]
    fn newly_observed_update_version_dispatches_exactly_once() {
        let (temp, repository, mut controller, sent) = fixture();
        enable_notifications(&repository);

        controller
            .observe_update_available(&repository, "v9.9.9")
            .unwrap();
        drop(controller);

        let paths = AppPaths::from_local_app_data(&temp.0);
        let mut reloaded = NotificationController::new(
            V1NotificationEngine::load(&paths),
            FakeToastSink {
                sent: Arc::clone(&sent),
            },
        );
        reloaded
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
    fn test_action_preserves_disabled_permission_code() {
        let temp = TestDir::new();
        let paths = AppPaths::from_local_app_data(&temp.0);
        let database = Arc::new(AppDatabase::open(&paths.database).unwrap());
        let repository = SettingsRepository::new(database);
        let mut controller =
            NotificationController::new(V1NotificationEngine::load(&paths), DisabledToastSink);

        assert_eq!(
            controller.send_test(&repository).unwrap_err(),
            "NOTIFICATION_PERMISSION_DISABLED"
        );
    }

    #[test]
    fn toast_text_is_xml_escaped_before_transport() {
        assert_eq!(xml_escape("<&>\"'"), "&lt;&amp;&gt;&quot;&apos;");
    }

    #[test]
    fn notification_capability_reports_app_level_suppression() {
        let capability = detect_notification_capability(
            &FakeNotificationRegistry {
                app_enabled: Some(0),
                global_enabled: None,
            },
            true,
        );

        assert_eq!(capability.status, NotificationCapabilityStatus::AppDisabled);
        assert!(capability.can_open_settings);
        assert_eq!(
            serde_json::to_value(capability).unwrap(),
            serde_json::json!({
                "status": "appDisabled",
                "canOpenSettings": true,
            })
        );
    }

    #[test]
    fn notification_capability_reports_global_suppression() {
        let capability = detect_notification_capability(
            &FakeNotificationRegistry {
                app_enabled: Some(1),
                global_enabled: Some(0),
            },
            true,
        );

        assert_eq!(
            capability.status,
            NotificationCapabilityStatus::GlobalDisabled
        );
        assert!(capability.can_open_settings);
    }

    #[test]
    fn notification_capability_defaults_missing_registry_values_to_available() {
        let capability = detect_notification_capability(&FakeNotificationRegistry::default(), true);

        assert_eq!(capability.status, NotificationCapabilityStatus::Available);
        assert!(capability.can_open_settings);
    }

    #[test]
    fn notification_capability_reports_unsupported_off_windows() {
        let capability =
            detect_notification_capability(&FakeNotificationRegistry::default(), false);

        assert_eq!(capability.status, NotificationCapabilityStatus::Unsupported);
        assert!(!capability.can_open_settings);
    }

    #[test]
    fn disabled_notification_preflight_does_not_start_transport() {
        let mut transport_started = false;
        let result = send_windows_toast_with(
            &FakeNotificationRegistry {
                app_enabled: Some(0),
                global_enabled: None,
            },
            true,
            || {
                transport_started = true;
                Ok(())
            },
        );

        assert_eq!(result.unwrap_err(), "NOTIFICATION_PERMISSION_DISABLED");
        assert!(!transport_started);
    }

    #[test]
    fn account_change_on_stable_profile_does_not_carry_controller_events() {
        let (_temp, repository, mut controller, sent) = fixture();
        enable_notifications(&repository);
        let profile_id = Uuid::new_v4();

        controller
            .observe_usage(
                &repository,
                profile_id,
                Some("account-hash-a"),
                &weekly(profile_id, 20.0),
                Some(1),
            )
            .unwrap();
        controller
            .observe_usage(
                &repository,
                profile_id,
                Some("account-hash-a"),
                &weekly(profile_id, 40.0),
                Some(1),
            )
            .unwrap();
        for _ in 0..3 {
            controller
                .observe_refresh(&repository, profile_id, Some("account-hash-a"), false)
                .unwrap();
        }
        assert_eq!(sent.lock().unwrap().len(), 2);

        controller
            .observe_usage(
                &repository,
                profile_id,
                Some("account-hash-b"),
                &weekly(profile_id, 80.0),
                Some(5),
            )
            .unwrap();
        controller
            .observe_refresh(&repository, profile_id, Some("account-hash-b"), true)
            .unwrap();
        assert_eq!(sent.lock().unwrap().len(), 2);
    }

    #[test]
    fn unavailable_account_marker_preserves_controller_state() {
        let (_temp, repository, mut controller, sent) = fixture();
        enable_notifications(&repository);
        let profile_id = Uuid::new_v4();

        controller
            .observe_usage(
                &repository,
                profile_id,
                Some("account-hash-a"),
                &weekly(profile_id, 20.0),
                None,
            )
            .unwrap();
        controller
            .observe_usage(
                &repository,
                profile_id,
                None,
                &weekly(profile_id, 40.0),
                None,
            )
            .unwrap();
        controller
            .observe_refresh(&repository, profile_id, Some("account-hash-a"), false)
            .unwrap();
        controller
            .observe_refresh(&repository, profile_id, None, false)
            .unwrap();
        controller
            .observe_refresh(&repository, profile_id, None, false)
            .unwrap();

        assert_eq!(sent.lock().unwrap().len(), 2);
    }

    #[test]
    fn account_marker_normalizes_email_and_never_returns_raw_identity() {
        let expected = codexbar::core::sha256_hex(b"user@example.invalid");
        let marker = account_marker_from_email(Some(" User@Example.Invalid ")).unwrap();

        assert_eq!(marker, expected);
        assert!(!marker.contains("example"));
        assert_eq!(account_marker_from_email(Some("   ")), None);
        assert_eq!(account_marker_from_email(None), None);
    }

    #[test]
    fn typed_terminal_refresh_events_drive_failure_and_recovery() {
        let (_temp, repository, mut controller, sent) = fixture();
        enable_notifications(&repository);
        let profile_id = Uuid::new_v4();

        for _ in 0..3 {
            assert!(
                controller
                    .observe_account_service_event(
                        &repository,
                        Some("account-hash-a"),
                        &codexbar::accounts::model::AccountServiceEvent::RefreshCompleted {
                            profile_id,
                            success: false,
                        },
                    )
                    .unwrap()
            );
        }
        assert!(
            controller
                .observe_account_service_event(
                    &repository,
                    Some("account-hash-a"),
                    &codexbar::accounts::model::AccountServiceEvent::RefreshCompleted {
                        profile_id,
                        success: true,
                    },
                )
                .unwrap()
        );

        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert!(sent[0].title.contains("failed"));
        assert!(sent[1].title.contains("recovered"));
    }
}
