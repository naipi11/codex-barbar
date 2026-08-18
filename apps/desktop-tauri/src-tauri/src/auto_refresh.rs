//! Periodic auto-refresh for the desktop shell.
//!
//! The account service only refreshes on demand; this loop turns the
//! configured `refreshIntervalSeconds` setting into timer-driven refreshes
//! for the currently selected profile.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use codexbar::core::RefreshTrigger;

use tauri::Manager;

use crate::state::AppState;

/// Start the timer-driven refresh loop. The loop checks every 30 seconds and
/// refreshes only when the configured interval has elapsed since the previous
/// attempt; an interval of `0` disables automatic refreshes.
pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(30));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_refresh: Option<Instant> = None;
        loop {
            ticker.tick().await;
            let service = {
                let state = app.state::<Mutex<AppState>>();
                let Ok(guard) = state.lock() else {
                    continue;
                };
                guard.account_service.clone()
            };
            let Some(service) = service else {
                continue;
            };
            let Ok(settings) = service.repositories().settings.load() else {
                continue;
            };
            if settings.refresh_interval_seconds == 0 {
                last_refresh = None;
                continue;
            }
            let due = match last_refresh {
                Some(started) => started.elapsed().as_secs() >= settings.refresh_interval_seconds,
                None => true,
            };
            if !due {
                continue;
            }
            let Ok(snapshot) = service.snapshot() else {
                continue;
            };
            last_refresh = Some(Instant::now());
            let _ = service
                .request_refresh(snapshot.selected_profile_id, RefreshTrigger::Timer)
                .await;
        }
    });
}