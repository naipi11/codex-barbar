//! Fixed V1 native tray menu.
//!
//! Top-level order is intentionally stable. The account submenu contains only
//! checked profile-selection items: no logout, delete, token, or cookie action
//! is available from the tray.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
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

/// Deterministic native menu plan derived from a normalized visible order.
///
/// Separators are generated between neighboring groups; they are never
/// persisted and unknown IDs are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPlanEntry {
    Item(&'static str),
    Separator,
}

pub fn native_menu_plan(order: &[String]) -> Vec<MenuPlanEntry> {
    let mut plan = Vec::new();
    let mut previous_group: Option<&'static str> = None;
    for id in order {
        let (group, item_id) = match id.as_str() {
            OPEN_PANEL_ID => ("primary", OPEN_PANEL_ID),
            REFRESH_ID => ("primary", REFRESH_ID),
            ACCOUNTS_ID => ("primary", ACCOUNTS_ID),
            OPEN_USAGE_ID => ("primary", OPEN_USAGE_ID),
            SETTINGS_ID => ("secondary", SETTINGS_ID),
            ABOUT_ID => ("secondary", ABOUT_ID),
            QUIT_ID => ("quit", QUIT_ID),
            _ => continue,
        };
        if previous_group.is_some_and(|previous| previous != group) {
            plan.push(MenuPlanEntry::Separator);
        }
        plan.push(MenuPlanEntry::Item(item_id));
        previous_group = Some(group);
    }
    plan
}

pub fn build_native_menu<R: Runtime>(
    app: &AppHandle<R>,
    profiles: &[TrayProfileMenuItem],
    language: &str,
    order: &[String],
) -> tauri::Result<Menu<R>> {
    let labels = TrayMenuLabels::for_language(language);

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

    let menu = Menu::new(app)?;
    for entry in native_menu_plan(order) {
        match entry {
            MenuPlanEntry::Separator => {
                let separator = PredefinedMenuItem::separator(app)?;
                menu.append(&separator)?;
            }
            MenuPlanEntry::Item(id) => {
                if id == ACCOUNTS_ID {
                    menu.append(&accounts)?;
                    continue;
                }
                let item = match id {
                    OPEN_PANEL_ID => MenuItem::with_id(
                        app,
                        OPEN_PANEL_ID,
                        labels.open_panel,
                        true,
                        None::<&str>,
                    )?,
                    REFRESH_ID => {
                        MenuItem::with_id(app, REFRESH_ID, labels.refresh, true, None::<&str>)?
                    }
                    OPEN_USAGE_ID => MenuItem::with_id(
                        app,
                        OPEN_USAGE_ID,
                        labels.open_usage,
                        true,
                        None::<&str>,
                    )?,
                    SETTINGS_ID => {
                        MenuItem::with_id(app, SETTINGS_ID, labels.settings, true, None::<&str>)?
                    }
                    ABOUT_ID => MenuItem::with_id(app, ABOUT_ID, labels.about, true, None::<&str>)?,
                    QUIT_ID => MenuItem::with_id(app, QUIT_ID, labels.quit, true, None::<&str>)?,
                    _ => continue,
                };
                menu.append(&item)?;
            }
        }
    }
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_menu_plan_generates_separators_between_groups_and_ignores_unknown_ids() {
        let order = vec![
            "open_panel".to_string(),
            "refresh".to_string(),
            "accounts".to_string(),
            "open_usage".to_string(),
            "settings".to_string(),
            "about".to_string(),
            "quit".to_string(),
        ];
        assert_eq!(
            native_menu_plan(&order),
            vec![
                MenuPlanEntry::Item("open_panel"),
                MenuPlanEntry::Item("refresh"),
                MenuPlanEntry::Item("accounts"),
                MenuPlanEntry::Item("open_usage"),
                MenuPlanEntry::Separator,
                MenuPlanEntry::Item("settings"),
                MenuPlanEntry::Item("about"),
                MenuPlanEntry::Separator,
                MenuPlanEntry::Item("quit"),
            ]
        );
    }

    #[test]
    fn native_menu_plan_skips_unknown_ids_and_separates_group_changes() {
        let order = vec![
            "quit".to_string(),
            "bogus".to_string(),
            "settings".to_string(),
        ];
        assert_eq!(
            native_menu_plan(&order),
            vec![
                MenuPlanEntry::Item("quit"),
                MenuPlanEntry::Separator,
                MenuPlanEntry::Item("settings"),
            ]
        );
    }

    #[test]
    fn native_menu_plan_follows_candidate_order_deterministically() {
        let order = vec![
            "settings".to_string(),
            "open_panel".to_string(),
            "quit".to_string(),
        ];
        assert_eq!(
            native_menu_plan(&order),
            vec![
                MenuPlanEntry::Item("settings"),
                MenuPlanEntry::Separator,
                MenuPlanEntry::Item("open_panel"),
                MenuPlanEntry::Separator,
                MenuPlanEntry::Item("quit"),
            ]
        );
    }

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
