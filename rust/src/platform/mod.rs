//! Operating-system integrations used by the V1 desktop shell.

pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    Windows,
    Linux,
    Other,
}

pub const fn kind() -> PlatformKind {
    #[cfg(target_os = "windows")]
    {
        return PlatformKind::Windows;
    }
    #[cfg(target_os = "linux")]
    {
        return PlatformKind::Linux;
    }
    #[allow(unreachable_code)]
    PlatformKind::Other
}

#[cfg(test)]
mod capability_contract_tests {
    #[test]
    fn platform_kind_reports_linux_on_linux() {
        #[cfg(target_os = "linux")]
        assert_eq!(super::kind(), super::PlatformKind::Linux);
    }
}
