//! Frozen event names emitted by the desktop shell.

#![allow(dead_code)]

pub const PROFILE_USAGE_STATE_CHANGED: &str = "profile-usage-state-changed";
pub const REFRESH_STATE_CHANGED: &str = "refresh-state-changed";
pub const ACCOUNTS_UPDATED: &str = "accounts-updated";
pub const ACCOUNT_LOGIN_UPDATED: &str = "account-login-updated";
pub const SELECTED_PROFILE_CHANGED: &str = "selected-profile-changed";
pub const SETTINGS_CHANGED: &str = "settings-changed";
pub const LOCALE_CHANGED: &str = "locale-changed";
pub const UPDATE_STATE_CHANGED: &str = "update-state-changed";
pub const STATUS_SURFACE_FEEDBACK_CHANGED: &str = "status-surface-feedback-changed";
pub const FLOAT_BALL_MOTION_CHANGED: &str = "codexbar:float-ball-motion-changed";

pub const TRAY_REBUILD_EVENTS: [&str; 6] = [
    PROFILE_USAGE_STATE_CHANGED,
    REFRESH_STATE_CHANGED,
    ACCOUNTS_UPDATED,
    SELECTED_PROFILE_CHANGED,
    SETTINGS_CHANGED,
    LOCALE_CHANGED,
];

pub const ALL: [&str; 10] = [
    PROFILE_USAGE_STATE_CHANGED,
    REFRESH_STATE_CHANGED,
    ACCOUNTS_UPDATED,
    ACCOUNT_LOGIN_UPDATED,
    SELECTED_PROFILE_CHANGED,
    SETTINGS_CHANGED,
    LOCALE_CHANGED,
    UPDATE_STATE_CHANGED,
    STATUS_SURFACE_FEEDBACK_CHANGED,
    FLOAT_BALL_MOTION_CHANGED,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_are_frozen() {
        assert_eq!(
            ALL,
            [
                "profile-usage-state-changed",
                "refresh-state-changed",
                "accounts-updated",
                "account-login-updated",
                "selected-profile-changed",
                "settings-changed",
                "locale-changed",
                "update-state-changed",
                "status-surface-feedback-changed",
                "codexbar:float-ball-motion-changed",
            ]
        );
    }

    #[test]
    fn tray_rebuild_event_set_is_fixed() {
        assert_eq!(
            TRAY_REBUILD_EVENTS,
            [
                "profile-usage-state-changed",
                "refresh-state-changed",
                "accounts-updated",
                "selected-profile-changed",
                "settings-changed",
                "locale-changed",
            ]
        );
    }
}
