//! Deterministic JSON migration for persisted application settings.

use serde_json::{Map, Value};

use super::{MenuLayout, SETTINGS_SCHEMA_VERSION, StorageError, normalize_panel_actions};

/// Upgrade a persisted settings document before it is decoded into typed
/// preferences. The caller decides when to persist the returned value.
pub fn migrate_settings_json(mut value: Value) -> Result<(Value, bool), StorageError> {
    let object = value.as_object_mut().ok_or_else(settings_decode_error)?;
    let version = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    let mut changed = version < u64::from(SETTINGS_SCHEMA_VERSION);
    changed |= migrate_percent(object, "taskbarStatusOpacity", "taskbarTransparencyPercent");
    changed |= migrate_percent(object, "floatBallOpacity", "floatBallTransparencyPercent");
    changed |= migrate_percent(object, "floatBallGlow", "floatBallGlowPercent");
    changed |= migrate_taskbar_presentation(object);
    changed |= migrate_panel_layout(object);
    if version < u64::from(SETTINGS_SCHEMA_VERSION) {
        object.insert(
            "schemaVersion".to_string(),
            Value::from(SETTINGS_SCHEMA_VERSION),
        );
    }

    Ok((value, changed))
}

fn migrate_taskbar_presentation(object: &mut Map<String, Value>) -> bool {
    let legacy = object.remove("taskbarTray");
    let changed = legacy.is_some();
    if object.contains_key("taskbarPresentation") {
        return changed;
    }
    let Some(tray) = legacy.and_then(|value| value.as_object().cloned()) else {
        return changed;
    };

    let mut presentation = Map::new();
    for key in [
        "showTaskbarIcon",
        "showTaskbarAccount",
        "showWeeklyLabel",
        "showWeeklyPercent",
        "showResetDate",
        "hideStatusSurfacesInFullscreen",
    ] {
        if let Some(value) = tray.get(key).filter(|value| value.is_boolean()) {
            presentation.insert(key.to_string(), value.clone());
        }
    }
    if tray
        .get("density")
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "compact" | "standard"))
    {
        presentation.insert("density".to_string(), tray["density"].clone());
    }
    if presentation.is_empty() {
        return changed;
    }
    object.insert(
        "taskbarPresentation".to_string(),
        Value::Object(presentation),
    );
    true
}

fn migrate_percent(object: &mut Map<String, Value>, legacy_key: &str, v2_key: &str) -> bool {
    let legacy = object.remove(legacy_key);
    let changed = legacy.is_some();
    if object.contains_key(v2_key) {
        return changed;
    }

    let Some(value) = legacy.and_then(|value| value.as_u64()) else {
        return changed;
    };
    let scaled = ((value.min(80) * 100) + 40) / 80;
    object.insert(v2_key.to_string(), Value::from(scaled));
    true
}

fn migrate_panel_layout(object: &mut Map<String, Value>) -> bool {
    let legacy_menu = object.remove("menu");
    let changed = legacy_menu.is_some();
    let legacy_actions = legacy_menu.and_then(|value| {
        value
            .as_object()
            .and_then(|menu| menu.get("trayPanel"))
            .cloned()
    });
    let Some(legacy_actions) = legacy_actions else {
        return changed;
    };

    let panel = match object.entry("panel".to_string()) {
        serde_json::map::Entry::Occupied(entry) => entry.into_mut(),
        serde_json::map::Entry::Vacant(entry) => entry.insert(Value::Object(Map::new())),
    };
    let Some(panel) = panel.as_object_mut() else {
        return changed;
    };
    if panel.contains_key("actions") {
        return changed;
    }

    let mut actions = serde_json::from_value::<MenuLayout>(legacy_actions).unwrap_or_default();
    normalize_panel_actions(&mut actions);
    if let Ok(actions) = serde_json::to_value(actions) {
        panel.insert("actions".to_string(), actions);
        return true;
    }
    changed
}

fn settings_decode_error() -> StorageError {
    StorageError::new(
        crate::core::AppErrorKind::StorageFailure,
        "SETTINGS_DECODE_FAILED",
        "settings must be a JSON object",
    )
}
