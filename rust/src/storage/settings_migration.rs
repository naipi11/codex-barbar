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

    if version >= u64::from(SETTINGS_SCHEMA_VERSION) {
        return Ok((value, false));
    }

    migrate_percent(object, "taskbarStatusOpacity", "taskbarTransparencyPercent");
    migrate_percent(object, "floatBallOpacity", "floatBallTransparencyPercent");
    migrate_percent(object, "floatBallGlow", "floatBallGlowPercent");
    migrate_taskbar_presentation(object);
    migrate_panel_layout(object);
    object.insert(
        "schemaVersion".to_string(),
        Value::from(SETTINGS_SCHEMA_VERSION),
    );

    Ok((value, true))
}

fn migrate_taskbar_presentation(object: &mut Map<String, Value>) {
    let legacy = object.remove("taskbarTray");
    if object.contains_key("taskbarPresentation") {
        return;
    }
    let Some(tray) = legacy.and_then(|value| value.as_object().cloned()) else {
        return;
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
    object.insert(
        "taskbarPresentation".to_string(),
        Value::Object(presentation),
    );
}

fn migrate_percent(object: &mut Map<String, Value>, legacy_key: &str, v2_key: &str) {
    if object.contains_key(v2_key) {
        object.remove(legacy_key);
        return;
    }

    let Some(value) = object.remove(legacy_key).and_then(|value| value.as_u64()) else {
        return;
    };
    let scaled = ((value.min(80) * 100) + 40) / 80;
    object.insert(v2_key.to_string(), Value::from(scaled));
}

fn migrate_panel_layout(object: &mut Map<String, Value>) {
    let legacy_actions = object.remove("menu").and_then(|value| {
        value
            .as_object()
            .and_then(|menu| menu.get("trayPanel"))
            .cloned()
    });

    let panel = object
        .entry("panel".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(panel) = panel.as_object_mut() else {
        return;
    };

    let actions = panel
        .get("actions")
        .cloned()
        .or(legacy_actions)
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut actions = serde_json::from_value::<MenuLayout>(actions).unwrap_or_default();
    normalize_panel_actions(&mut actions);
    if let Ok(actions) = serde_json::to_value(actions) {
        panel.insert("actions".to_string(), actions);
    }
}

fn settings_decode_error() -> StorageError {
    StorageError::new(
        crate::core::AppErrorKind::StorageFailure,
        "SETTINGS_DECODE_FAILED",
        "settings must be a JSON object",
    )
}
