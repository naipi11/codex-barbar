//! Fixed V1 native tray menu.
//!
//! Top-level order is intentionally stable. The account submenu contains only
//! checked profile-selection items: no logout, delete, token, or cookie action
//! is available from the tray.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, Submenu};
use tauri::{AppHandle, Runtime};
use uuid::Uuid;

pub const OPEN_PANEL_ID: &str = "open_panel";
pub const REFRESH_ID: &str = "refresh";
pub const ACCOUNTS_ID: &str = "accounts";
pub const OPEN_USAGE_ID: &str = "open_usage";
pub const SETTINGS_ID: &str = "settings";
pub const ABOUT_ID: &str = "about";
pub const QUIT_ID: &str = "quit";
const PROFILE_PREFIX: &str = "profile:";

#[allow(dead_code)]
pub const fn menu_item_ids() -> [&'static str; 7] {
    [
        OPEN_PANEL_ID,
        REFRESH_ID,
        ACCOUNTS_ID,
        OPEN_USAGE_ID,
        SETTINGS_ID,
        ABOUT_ID,
        QUIT_ID,
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayProfileMenuItem {
    pub id: Uuid,
    pub label: String,
    pub checked: bool,
}

pub fn profile_menu_items<I, S>(profiles: I, selected_profile_id: Uuid) -> Vec<TrayProfileMenuItem>
where
    I: IntoIterator<Item = (Uuid, S)>,
    S: Into<String>,
{
    profiles
        .into_iter()
        .map(|(id, label)| TrayProfileMenuItem {
            id,
            label: label.into(),
            checked: id == selected_profile_id,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    OpenPanel,
    Refresh,
    OpenUsage,
    Settings,
    About,
    Quit,
    SelectProfile(Uuid),
    None,
}

pub fn menu_action(id: &str) -> TrayMenuAction {
    match id {
        OPEN_PANEL_ID => TrayMenuAction::OpenPanel,
        REFRESH_ID => TrayMenuAction::Refresh,
        OPEN_USAGE_ID => TrayMenuAction::OpenUsage,
        SETTINGS_ID => TrayMenuAction::Settings,
        ABOUT_ID => TrayMenuAction::About,
        QUIT_ID => TrayMenuAction::Quit,
        _ => id
            .strip_prefix(PROFILE_PREFIX)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(TrayMenuAction::SelectProfile)
            .unwrap_or(TrayMenuAction::None),
    }
}

fn profile_menu_id(id: Uuid) -> String {
    format!("{PROFILE_PREFIX}{id}")
}

#[derive(Debug, Clone, Copy)]
struct TrayMenuLabels {
    open_panel: &'static str,
    refresh: &'static str,
    accounts: &'static str,
    open_usage: &'static str,
    settings: &'static str,
    about: &'static str,
    quit: &'static str,
}

impl TrayMenuLabels {
    fn for_language(language: &str) -> Self {
        if language == "zh-CN" {
            Self {
                open_panel: "打开 codex-barbar",
                refresh: "刷新",
                accounts: "账户",
                open_usage: "打开 Codex 用量",
                settings: "设置",
                about: "关于",
                quit: "退出",
            }
        } else {
            Self {
                open_panel: "Open codex-barbar",
                refresh: "Refresh",
                accounts: "Accounts",
                open_usage: "Open Codex Usage",
                settings: "Settings",
                about: "About",
                quit: "Quit",
            }
        }
    }
}

pub fn build_native_menu<R: Runtime>(
    app: &AppHandle<R>,
    profiles: &[TrayProfileMenuItem],
    language: &str,
) -> tauri::Result<Menu<R>> {
    let labels = TrayMenuLabels::for_language(language);
    let open_panel = MenuItem::with_id(app, OPEN_PANEL_ID, labels.open_panel, true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, REFRESH_ID, labels.refresh, true, None::<&str>)?;

    let accounts = Submenu::with_id(app, ACCOUNTS_ID, labels.accounts, !profiles.is_empty())?;
    for profile in profiles {
        let item = CheckMenuItem::with_id(
            app,
            profile_menu_id(profile.id),
            &profile.label,
            true,
            profile.checked,
            None::<&str>,
        )?;
        accounts.append(&item)?;
    }

    let open_usage = MenuItem::with_id(app, OPEN_USAGE_ID, labels.open_usage, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS_ID, labels.settings, true, None::<&str>)?;
    let about = MenuItem::with_id(app, ABOUT_ID, labels.about, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, labels.quit, true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &open_panel,
            &refresh,
            &accounts,
            &open_usage,
            &settings,
            &about,
            &quit,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_menu_order_is_fixed() {
        assert_eq!(
            menu_item_ids(),
            [
                "open_panel",
                "refresh",
                "accounts",
                "open_usage",
                "settings",
                "about",
                "quit",
            ]
        );
    }

    #[test]
    fn profile_submenu_checks_only_the_selected_profile() {
        let current = Uuid::from_u128(1);
        let work = Uuid::from_u128(2);
        let items = profile_menu_items([(current, "Current CLI"), (work, "Work")], work);
        assert_eq!(
            items,
            vec![
                TrayProfileMenuItem {
                    id: current,
                    label: "Current CLI".into(),
                    checked: false,
                },
                TrayProfileMenuItem {
                    id: work,
                    label: "Work".into(),
                    checked: true,
                },
            ]
        );
    }

    #[test]
    fn tray_menu_has_no_logout_or_delete_action() {
        let joined = menu_item_ids().join(" ");
        for forbidden in ["logout", "delete", "remove", "token", "cookie"] {
            assert!(!joined.contains(forbidden));
        }
    }

    #[test]
    fn profile_menu_ids_round_trip_to_a_selection_action() {
        let id = Uuid::from_u128(42);
        assert_eq!(
            menu_action(&profile_menu_id(id)),
            TrayMenuAction::SelectProfile(id)
        );
    }
}
