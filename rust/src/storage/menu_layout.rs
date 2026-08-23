//! Stable built-in menu item registries, layout preferences, and safe
//! normalization.
//!
//! Only stable built-in IDs are configured here. The frontend never sends
//! commands, scripts, URLs, executable paths, or arbitrary action definitions.

use serde::{Deserialize, Serialize};

/// Native tray right-click menu registry in default order.
pub const NATIVE_TRAY_ITEMS: [&str; 7] = [
    "open_panel", "refresh", "accounts", "open_usage", "settings", "about", "quit",
];

/// Tray-panel quick-action registry in default order.
pub const TRAY_PANEL_ACTIONS: [&str; 5] = [
    "refresh", "open_usage", "settings", "dismiss", "quit",
];

/// Native items that must always be visible and reachable.
pub const REQUIRED_NATIVE_TRAY_ITEMS: [&str; 2] = ["settings", "quit"];

/// Ordered visible IDs for a surface after normalization.
pub fn default_visible_order(registry: &[&str]) -> Vec<String> {
    registry.iter().map(|id| (*id).to_string()).collect()
}

/// Normalize a user layout into the deterministic visible order.
///
/// 1. Retain the first occurrence of each known, non-hidden ID in order.
/// 2. Restore required IDs even when the user hid them (registry order).
/// 3. Append remaining known, non-hidden registry IDs omitted from order.
/// 4. Fall back to the registry default when nothing is visible.
pub fn normalize_layout(
    layout: &MenuLayout,
    registry: &[&str],
    required_visible: &[&str],
) -> Vec<String> {
    let mut visible = Vec::new();
    for id in &layout.order {
        if !registry.contains(&id.as_str()) || layout.hidden.iter().any(|h| h == id) {
            continue;
        }
        if !visible.contains(id) {
            visible.push(id.clone());
        }
    }
    for id in required_visible {
        if layout.hidden.iter().any(|h| h == id) && !visible.iter().any(|v| v == id) {
            visible.push((*id).to_string());
        }
    }
    for id in registry {
        if layout.hidden.iter().any(|h| h == id) || visible.iter().any(|v| v == id) {
            continue;
        }
        visible.push((*id).to_string());
    }
    if visible.is_empty() {
        return default_visible_order(registry);
    }
    visible
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MenuLayout {
    pub order: Vec<String>,
    pub hidden: Vec<String>,
}

impl Default for MenuLayout {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            hidden: Vec::new(),
        }
    }
}

impl MenuLayout {
    pub fn normalized_order(&self, registry: &[&str], required_visible: &[&str]) -> Vec<String> {
        normalize_layout(self, registry, required_visible)
    }

    pub fn sanitized_hidden(&self, registry: &[&str]) -> Vec<String> {
        let mut hidden = self
            .hidden
            .iter()
            .filter(|id| registry.contains(&id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        hidden.dedup();
        hidden
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MenuPreferences {
    pub native_tray: MenuLayout,
    pub tray_panel: MenuLayout,
}

impl Default for MenuPreferences {
    fn default() -> Self {
        Self {
            native_tray: MenuLayout {
                order: default_visible_order(&NATIVE_TRAY_ITEMS),
                hidden: Vec::new(),
            },
            tray_panel: MenuLayout {
                order: default_visible_order(&TRAY_PANEL_ACTIONS),
                hidden: Vec::new(),
            },
        }
    }
}

impl MenuPreferences {
    pub fn normalize(&mut self) {
        self.native_tray.order =
            normalize_layout(&self.native_tray, &NATIVE_TRAY_ITEMS, &REQUIRED_NATIVE_TRAY_ITEMS);
        self.native_tray.hidden = self.native_tray.sanitized_hidden(&NATIVE_TRAY_ITEMS);
        self.tray_panel.order = normalize_layout(&self.tray_panel, &TRAY_PANEL_ACTIONS, &[]);
        self.tray_panel.hidden = self.tray_panel.sanitized_hidden(&TRAY_PANEL_ACTIONS);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuLayoutPatch {
    pub order: Option<Vec<String>>,
    pub hidden: Option<Vec<String>>,
}

impl MenuLayoutPatch {
    fn apply_to(self, layout: &mut MenuLayout) {
        if let Some(order) = self.order {
            layout.order = order;
        }
        if let Some(hidden) = self.hidden {
            layout.hidden = hidden;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuPreferencesPatch {
    pub native_tray: Option<MenuLayoutPatch>,
    pub tray_panel: Option<MenuLayoutPatch>,
}

impl MenuPreferencesPatch {
    pub(crate) fn apply_to(self, preferences: &mut MenuPreferences) {
        if let Some(native_tray) = self.native_tray {
            native_tray.apply_to(&mut preferences.native_tray);
        }
        if let Some(tray_panel) = self.tray_panel {
            tray_panel.apply_to(&mut preferences.tray_panel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_order_unknown_ids_duplicates_and_hidden_required_are_normalized() {
        assert_eq!(
            normalize_layout(
                &MenuLayout {
                    order: vec![
                        "quit".into(),
                        "unknown".into(),
                        "refresh".into(),
                        "refresh".into()
                    ],
                    hidden: vec!["settings".into(), "refresh".into()],
                },
                &NATIVE_TRAY_ITEMS,
                &REQUIRED_NATIVE_TRAY_ITEMS,
            ),
            vec![
                "quit", "settings", "open_panel", "accounts", "open_usage", "about"
            ]
        );
    }

    #[test]
    fn empty_layout_restores_default_registry_order() {
        assert_eq!(
            normalize_layout(
                &MenuLayout::default(),
                &NATIVE_TRAY_ITEMS,
                &REQUIRED_NATIVE_TRAY_ITEMS,
            ),
            vec![
                "open_panel",
                "refresh",
                "accounts",
                "open_usage",
                "settings",
                "about",
                "quit"
            ]
        );
    }

    #[test]
    fn hidden_unknown_ids_are_harmless() {
        let layout = MenuLayout {
            order: vec!["refresh".into(), "bogus".into()],
            hidden: vec!["nope".into(), "open_panel".into()],
        };
        let visible = normalize_layout(&layout, &NATIVE_TRAY_ITEMS, &REQUIRED_NATIVE_TRAY_ITEMS);
        assert_eq!(visible.first().map(String::as_str), Some("refresh"));
        assert!(!visible.contains(&"open_panel".to_string()));
        assert!(!visible.contains(&"bogus".to_string()));
    }

    #[test]
    fn tray_panel_layout_has_no_implicit_required_items() {
        let visible = normalize_layout(
            &MenuLayout {
                order: vec!["quit".into()],
                hidden: vec!["settings".into()],
            },
            &TRAY_PANEL_ACTIONS,
            &[],
        );
        assert_eq!(
            visible,
            vec![
                "quit".to_string(),
                "refresh".to_string(),
                "open_usage".to_string(),
                "dismiss".to_string()
            ]
        );
    }

    #[test]
    fn mandatory_native_ids_are_restored_even_when_all_others_are_hidden() {
        let visible = normalize_layout(
            &MenuLayout {
                order: Vec::new(),
                hidden: NATIVE_TRAY_ITEMS.iter().map(|id| (*id).to_string()).collect(),
            },
            &NATIVE_TRAY_ITEMS,
            &REQUIRED_NATIVE_TRAY_ITEMS,
        );
        assert_eq!(visible, vec!["settings".to_string(), "quit".to_string()]);
    }

    #[test]
    fn menu_preferences_defaults_match_registries() {
        let defaults = MenuPreferences::default();
        assert_eq!(
            defaults.native_tray.order,
            NATIVE_TRAY_ITEMS.iter().map(|id| (*id).to_string()).collect::<Vec<_>>()
        );
        assert_eq!(
            defaults.tray_panel.order,
            TRAY_PANEL_ACTIONS.iter().map(|id| (*id).to_string()).collect::<Vec<_>>()
        );
        assert!(defaults.native_tray.hidden.is_empty());
        assert!(defaults.tray_panel.hidden.is_empty());
    }

    #[test]
    fn normalize_sanitizes_hidden_but_forces_required_visibility() {
        let mut preferences = MenuPreferences {
            native_tray: MenuLayout {
                order: vec!["quit".into(), "unknown".into()],
                hidden: vec!["settings".into(), "refresh".into(), "unknown".into()],
            },
            ..MenuPreferences::default()
        };
        preferences.normalize();

        assert!(preferences.native_tray.order.contains(&"settings".to_string()));
        assert!(preferences.native_tray.order.contains(&"quit".to_string()));
        assert!(!preferences.native_tray.order.contains(&"refresh".to_string()));
        assert!(!preferences.native_tray.order.contains(&"unknown".to_string()));
        assert_eq!(
            preferences.native_tray.hidden,
            vec!["settings".to_string(), "refresh".to_string()]
        );
    }

    #[test]
    fn partial_menu_patch_preserves_peer_surface_layout() {
        let mut preferences = MenuPreferences::default();
        let patch = MenuPreferencesPatch {
            native_tray: Some(MenuLayoutPatch {
                order: Some(vec!["quit".into(), "about".into(), "settings".into()]),
                hidden: None,
            }),
            tray_panel: None,
        };
        patch.apply_to(&mut preferences);

        assert_eq!(
            preferences.native_tray.order,
            vec!["quit".to_string(), "about".to_string(), "settings".to_string()]
        );
        assert_eq!(
            preferences.tray_panel.order,
            TRAY_PANEL_ACTIONS.iter().map(|id| (*id).to_string()).collect::<Vec<_>>()
        );
    }
}

