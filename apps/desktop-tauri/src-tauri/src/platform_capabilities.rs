use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilitiesDto {
    pub platform: &'static str,
    pub system_tray: bool,
    pub taskbar_status: bool,
    pub floating_ball: bool,
    pub autostart: bool,
    pub notifications: &'static str,
    pub managed_credentials: bool,
}

pub const fn snapshot() -> PlatformCapabilitiesDto {
    #[cfg(target_os = "windows")]
    {
        return PlatformCapabilitiesDto {
            platform: "windows",
            system_tray: true,
            taskbar_status: true,
            floating_ball: true,
            autostart: true,
            notifications: "available",
            managed_credentials: true,
        };
    }
    #[cfg(target_os = "linux")]
    {
        return PlatformCapabilitiesDto {
            platform: "linux",
            system_tray: true,
            taskbar_status: false,
            floating_ball: true,
            autostart: true,
            notifications: "unsupported",
            managed_credentials: false,
        };
    }
    #[allow(unreachable_code)]
    PlatformCapabilitiesDto {
        platform: "other",
        system_tray: false,
        taskbar_status: false,
        floating_ball: false,
        autostart: false,
        notifications: "unsupported",
        managed_credentials: false,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_capabilities_disable_windows_taskbar_status() {
        let capabilities = crate::platform_capabilities::snapshot();
        assert_eq!(capabilities.platform, "linux");
        assert!(!capabilities.taskbar_status);
        assert!(capabilities.floating_ball);
    }
}
