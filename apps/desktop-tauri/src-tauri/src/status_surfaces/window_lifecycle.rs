use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseOutcome {
    Destroyed,
    HiddenPendingDestroy,
}

pub fn hide_and_destroy_with(
    hide: impl FnOnce() -> Result<(), ()>,
    destroy: impl FnOnce() -> Result<(), ()>,
) -> Result<CloseOutcome, &'static str> {
    let hidden = hide().is_ok();
    let destroyed = destroy().is_ok();
    if destroyed {
        Ok(CloseOutcome::Destroyed)
    } else if hidden {
        Ok(CloseOutcome::HiddenPendingDestroy)
    } else {
        Err("STATUS_SURFACE_WINDOW_CLOSE_FAILED")
    }
}

pub fn hide_and_destroy(window: &tauri::WebviewWindow) -> Result<CloseOutcome, &'static str> {
    hide_and_destroy_with(
        || window.hide().map_err(|_| ()),
        || window.destroy().map_err(|_| ()),
    )
}

pub fn close_cached_or_labeled(
    app: &tauri::AppHandle,
    cached: &mut Option<tauri::WebviewWindow>,
    label: &str,
) -> Result<CloseOutcome, String> {
    let Some(window) = cached
        .as_ref()
        .cloned()
        .or_else(|| app.get_webview_window(label))
    else {
        return Ok(CloseOutcome::Destroyed);
    };
    let outcome = hide_and_destroy(&window).map_err(str::to_string)?;
    *cached = None;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destroy_success_wins_when_hide_fails() {
        let outcome = hide_and_destroy_with(|| Err(()), || Ok(())).unwrap();
        assert_eq!(outcome, CloseOutcome::Destroyed);
    }

    #[test]
    fn hidden_window_can_be_reconciled_when_destroy_fails() {
        let outcome = hide_and_destroy_with(|| Ok(()), || Err(())).unwrap();
        assert_eq!(outcome, CloseOutcome::HiddenPendingDestroy);
    }

    #[test]
    fn close_fails_only_when_hide_and_destroy_both_fail() {
        assert_eq!(
            hide_and_destroy_with(|| Err(()), || Err(())).unwrap_err(),
            "STATUS_SURFACE_WINDOW_CLOSE_FAILED"
        );
    }
}
