use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum NotificationCapabilityStatus {
    Available,
    AppDisabled,
    GlobalDisabled,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilitiesDto {
    pub platform: &'static str,
    pub system_tray: bool,
    pub taskbar_status: bool,
    pub floating_ball: bool,
    pub autostart: bool,
    pub notifications: NotificationCapabilityStatus,
    pub managed_credentials: bool,
}

#[cfg(not(test))]
pub fn snapshot() -> PlatformCapabilitiesDto {
    let notifications = crate::notification_controller::notification_capability().status;
    let managed_credentials = codexbar::accounts::vault::platform_managed_credentials_available();
    snapshot_for(current_platform(), notifications, managed_credentials)
}

#[cfg(test)]
pub fn snapshot() -> PlatformCapabilitiesDto {
    snapshot_for(
        current_platform(),
        if cfg!(target_os = "windows") {
            NotificationCapabilityStatus::Available
        } else {
            NotificationCapabilityStatus::Unsupported
        },
        cfg!(target_os = "windows"),
    )
}

const fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}

pub(crate) fn snapshot_for(
    platform: &'static str,
    notifications: NotificationCapabilityStatus,
    managed_credentials: bool,
) -> PlatformCapabilitiesDto {
    match platform {
        "windows" => PlatformCapabilitiesDto {
            platform: "windows",
            system_tray: true,
            taskbar_status: true,
            floating_ball: true,
            autostart: true,
            notifications,
            managed_credentials,
        },
        "linux" => PlatformCapabilitiesDto {
            platform: "linux",
            system_tray: true,
            taskbar_status: false,
            floating_ball: true,
            autostart: true,
            notifications,
            managed_credentials,
        },
        _ => PlatformCapabilitiesDto {
            platform: "other",
            system_tray: false,
            taskbar_status: false,
            floating_ball: false,
            autostart: false,
            notifications: NotificationCapabilityStatus::Unsupported,
            managed_credentials: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{NotificationCapabilityStatus, snapshot_for};

    #[test]
    fn linux_runtime_probe_results_are_exposed_without_enabling_taskbar_status() {
        let capabilities = snapshot_for("linux", NotificationCapabilityStatus::Available, true);

        assert_eq!(capabilities.platform, "linux");
        assert!(!capabilities.taskbar_status);
        assert!(matches!(
            capabilities.notifications,
            NotificationCapabilityStatus::Available
        ));
        assert!(capabilities.managed_credentials);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_capabilities_disable_windows_taskbar_status() {
        let capabilities = crate::platform_capabilities::snapshot();
        assert_eq!(capabilities.platform, "linux");
        assert!(!capabilities.taskbar_status);
        assert!(capabilities.floating_ball);
    }
}
