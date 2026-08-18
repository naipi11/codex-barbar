//! Centralized V1 lifecycle: ordered startup, cache-first visibility, and
//! one-shot bounded shutdown.

use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// Cached tray visibility budget. Tray construction must never wait for Codex
/// discovery or network refresh beyond this window.
pub struct StartupBudget;

impl StartupBudget {
    pub const CACHED_TRAY_READY: Duration = Duration::from_secs(3);
}

/// Ordered startup milestones recorded by the coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupMilestone {
    Logging,
    Database,
    Recovery,
    Cache,
    Tray,
    CodexDiscovery,
    BackgroundRefresh,
}

impl StartupMilestone {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Logging => "logging",
            Self::Database => "database",
            Self::Recovery => "recovery",
            Self::Cache => "cache",
            Self::Tray => "tray",
            Self::CodexDiscovery => "codex-discovery",
            Self::BackgroundRefresh => "background-refresh",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupTrace(Vec<StartupMilestone>);

impl StartupTrace {
    pub fn push(&mut self, milestone: StartupMilestone) {
        self.0.push(milestone);
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.0.iter().map(|milestone| milestone.name()).collect()
    }
}

/// Canonical cache-first startup order: cached visibility is never blocked on
/// Codex discovery or the background refresh network call.
pub fn cached_start_steps() -> Vec<StartupMilestone> {
    vec![
        StartupMilestone::Logging,
        StartupMilestone::Database,
        StartupMilestone::Recovery,
        StartupMilestone::Cache,
        StartupMilestone::Tray,
        StartupMilestone::CodexDiscovery,
        StartupMilestone::BackgroundRefresh,
    ]
}

/// One-shot lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitState {
    Running,
    Stopping,
    Exited,
}

const QUIT_RUNNING: u8 = 0;
const QUIT_STOPPING: u8 = 1;
const QUIT_EXITED: u8 = 2;

/// Coordinates startup milestones and the single shutdown transition.
pub struct AppCoordinator {
    quit_state: AtomicU8,
    trace: Mutex<StartupTrace>,
}

impl Default for AppCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl AppCoordinator {
    pub fn new() -> Self {
        Self {
            quit_state: AtomicU8::new(QUIT_RUNNING),
            trace: Mutex::new(StartupTrace::default()),
        }
    }

    pub fn quit_state(&self) -> QuitState {
        match self.quit_state.load(Ordering::SeqCst) {
            QUIT_STOPPING => QuitState::Stopping,
            QUIT_EXITED => QuitState::Exited,
            _ => QuitState::Running,
        }
    }

    /// Atomically claim the single shutdown transition. Only the first caller
    /// receives `true`; later requests are ignored.
    pub fn begin_stopping(&self) -> bool {
        self.quit_state
            .compare_exchange(
                QUIT_RUNNING,
                QUIT_STOPPING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
    }

    pub fn mark_exited(&self) {
        self.quit_state.store(QUIT_EXITED, Ordering::SeqCst);
    }

    /// Record a non-secret startup milestone for diagnostics.
    pub fn record(&self, milestone: StartupMilestone) {
        if let Ok(mut trace) = self.trace.lock() {
            trace.push(milestone);
        }
    }

    /// Ordered startup milestones recorded by this process.
    pub fn trace_names(&self) -> Vec<&'static str> {
        self.trace
            .lock()
            .map(|trace| trace.names())
            .unwrap_or_default()
    }

    /// Run bounded shutdown exactly once. Later calls are no-ops.
    pub async fn request_quit<F, Fut>(&self, shutdown: F)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        if !self.begin_stopping() {
            return;
        }
        shutdown().await;
        self.mark_exited();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn cached_start_orders_recovery_before_tray_and_network() {
        let trace = cached_start_steps().into_iter().fold(
            StartupTrace::default(),
            |mut trace, milestone| {
                trace.push(milestone);
                trace
            },
        );
        assert_eq!(
            trace.names(),
            [
                "logging",
                "database",
                "recovery",
                "cache",
                "tray",
                "codex-discovery",
                "background-refresh",
            ]
        );
    }

    #[tokio::test]
    async fn repeated_quit_requests_run_shutdown_once() {
        let coordinator = Arc::new(AppCoordinator::new());
        let calls = Arc::new(AtomicUsize::new(0));

        let first = Arc::clone(&coordinator);
        let first_calls = Arc::clone(&calls);
        let second = Arc::clone(&coordinator);
        let second_calls = Arc::clone(&calls);

        tokio::join!(
            first.request_quit(move || async move {
                first_calls.fetch_add(1, Ordering::SeqCst);
            }),
            second.request_quit(move || async move {
                second_calls.fetch_add(1, Ordering::SeqCst);
            }),
        );

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(coordinator.quit_state(), QuitState::Exited);
    }

    #[test]
    fn cached_tray_ready_budget_is_three_seconds() {
        assert_eq!(StartupBudget::CACHED_TRAY_READY, Duration::from_secs(3));
    }
}
