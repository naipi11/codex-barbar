use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use codexbar::pricing::{
    CatalogStore, PricingRefreshCoordinator, PricingRefreshOutcome, refresh_catalog, refresh_due,
};
use tauri::Manager;

const POLL_INTERVAL: Duration = Duration::from_secs(60 * 60);

fn cache_root() -> PathBuf {
    codexbar::app_paths::AppPaths::discover()
        .map(|paths| paths.root)
        .unwrap_or_else(|_| std::env::temp_dir().join("codex-barbar"))
}

pub fn start_pricing_refresh_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let coordinator = PricingRefreshCoordinator::default();
        loop {
            if crate::proof_harness::is_proof_mode(&app) {
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
            let cache_root = cache_root();
            let store = CatalogStore::for_cache_root(&cache_root);
            let previous = store.load().ok().flatten();
            let due = previous
                .as_ref()
                .and_then(codexbar::pricing::refresh::latest_catalog_timestamp)
                .map(|last| refresh_due(last, Utc::now()))
                .unwrap_or(true);
            if due {
                let client = match codexbar::core::public_http_client() {
                    Ok(client) => client,
                    Err(_) => {
                        tokio::time::sleep(POLL_INTERVAL).await;
                        continue;
                    }
                };
                let outcome = refresh_catalog(&client, &cache_root, Utc::now(), &coordinator).await;
                let _ = refresh_fx_best_effort(&client, &cache_root).await;
                dispatch_pricing_notification(
                    &app,
                    outcome,
                    previous
                        .as_ref()
                        .map(|catalog| catalog.entries.len() as u8)
                        .unwrap_or(0),
                );
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

async fn refresh_fx_best_effort(client: &codexbar::core::HttpClient, cache_root: &Path) {
    let _ = codexbar::pricing::refresh::refresh_fx(client, cache_root).await;
}

fn dispatch_pricing_notification(
    app: &tauri::AppHandle,
    outcome: PricingRefreshOutcome,
    source_count: u8,
) {
    let Some(repository) = crate::notification_controller::repository_from_app(app) else {
        return;
    };
    let controller = app.state::<std::sync::Mutex<
        crate::notification_controller::NotificationController<
            crate::notification_controller::WindowsToastSink,
        >,
    >>();
    let Ok(mut controller) = controller.lock() else {
        return;
    };
    let failed = matches!(outcome, PricingRefreshOutcome::Failed);
    let catalog_changed = matches!(outcome, PricingRefreshOutcome::Updated);
    let _ = controller.observe_pricing_refresh(&repository, failed, source_count, catalog_changed);
}
