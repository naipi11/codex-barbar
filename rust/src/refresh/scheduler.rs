//! Refresh scheduler: merges triggers, applies cooldown/staleness/backoff,
//! and owns one in-flight refresh per profile.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use tokio::sync::{Mutex, Notify};

use crate::core::{AppError, ProfileId, RefreshDisposition, RefreshStatus, RefreshTrigger};
use crate::refresh::policy::{Clock, JitterSource, RefreshPolicy, SystemClock};

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("refresh already in progress")]
    Busy,
    #[error("refresh operation failed")]
    App(#[from] AppError),
}

type RefreshFn = dyn Fn(ProfileId, RefreshTrigger) -> BoxFuture<'static, Result<RefreshDisposition, AppError>>
    + Send
    + Sync;

/// One profile's scheduler state.
#[derive(Debug)]
struct ProfileState {
    in_flight: bool,
    last_started: Option<DateTime<Utc>>,
    last_success: Option<DateTime<Utc>>,
    manual_cooldown_until: Option<DateTime<Utc>>,
    backoff_until: Option<DateTime<Utc>>,
    attempt: u32,
}

impl ProfileState {
    fn new() -> Self {
        Self {
            in_flight: false,
            last_started: None,
            last_success: None,
            manual_cooldown_until: None,
            backoff_until: None,
            attempt: 0,
        }
    }
}

/// Deterministic refresh scheduler.
pub struct RefreshScheduler {
    policy: RefreshPolicy,
    clock: Arc<dyn Clock>,
    jitter: Arc<dyn JitterSource>,
    profiles: Arc<Mutex<HashMap<ProfileId, ProfileState>>>,
    notify: Arc<Notify>,
    on_refresh: Arc<RefreshFn>,
}

impl RefreshScheduler {
    pub fn new(
        policy: RefreshPolicy,
        clock: Arc<dyn Clock>,
        jitter: Arc<dyn JitterSource>,
        on_refresh: Arc<RefreshFn>,
    ) -> Self {
        Self {
            policy,
            clock,
            jitter,
            profiles: Arc::new(Mutex::new(HashMap::new())),
            notify: Arc::new(Notify::new()),
            on_refresh,
        }
    }

    pub fn with_system(policy: RefreshPolicy, on_refresh: Arc<RefreshFn>) -> Self {
        Self::new(
            policy,
            Arc::new(SystemClock),
            Arc::new(crate::refresh::policy::DeterministicJitter),
            on_refresh,
        )
    }

    pub async fn request(
        &self,
        profile_id: ProfileId,
        trigger: RefreshTrigger,
    ) -> Result<RefreshDisposition, SchedulerError> {
        let now = self.clock.now();
        let mut profiles = self.profiles.lock().await;
        let state = profiles.entry(profile_id).or_insert_with(ProfileState::new);

        if state.in_flight {
            return Ok(RefreshDisposition::Joined);
        }
        if trigger == RefreshTrigger::Manual
            && let Some(until) = state.manual_cooldown_until
            && now < until
        {
            return Ok(RefreshDisposition::Cooldown { retry_at: until });
        }
        if let Some(until) = state.backoff_until
            && now < until
            && trigger != RefreshTrigger::Manual
        {
            return Ok(RefreshDisposition::Backoff { retry_at: until });
        }

        state.in_flight = true;
        state.last_started = Some(now);
        let refresh = Arc::clone(&self.on_refresh);
        let jitter = Arc::clone(&self.jitter);
        let profiles = Arc::clone(&self.profiles);
        let notify = self.notify.clone();
        let policy = self.policy;
        tokio::spawn(async move {
            let disposition = refresh(profile_id, trigger).await;
            let mut profiles = profiles.lock().await;
            let Some(state) = profiles.get_mut(&profile_id) else {
                return;
            };
            match disposition {
                Ok(RefreshDisposition::Started) => {
                    state.in_flight = false;
                    state.last_success = Some(now);
                    state.attempt = 0;
                    state.backoff_until = None;
                    if trigger == RefreshTrigger::Manual {
                        state.manual_cooldown_until = Some(
                            now + chrono::Duration::from_std(policy.manual_cooldown())
                                .unwrap_or_default(),
                        );
                    }
                }
                Ok(_) => {
                    state.in_flight = false;
                }
                Err(error) => {
                    if !policy.blocks(error.kind) {
                        state.attempt = state.attempt.saturating_add(1);
                        let delay = policy.jittered_backoff(
                            state.attempt,
                            &*jitter,
                            seed(profile_id, state.attempt, now),
                        );
                        state.backoff_until =
                            Some(now + chrono::Duration::from_std(delay).unwrap_or_default());
                    }
                    state.in_flight = false;
                }
            }
            notify.notify_one();
        });
        Ok(RefreshDisposition::Started)
    }

    pub async fn refresh_status(&self, profile_id: ProfileId) -> RefreshStatus {
        let profiles = self.profiles.lock().await;
        let Some(state) = profiles.get(&profile_id) else {
            return RefreshStatus::Idle;
        };
        if state.in_flight {
            RefreshStatus::Refreshing
        } else if state
            .backoff_until
            .is_some_and(|until| self.clock.now() < until)
        {
            RefreshStatus::Backoff
        } else if state
            .manual_cooldown_until
            .is_some_and(|until| self.clock.now() < until)
        {
            RefreshStatus::Cooldown
        } else {
            RefreshStatus::Idle
        }
    }

    pub async fn wait_for_refresh(&self, profile_id: ProfileId) {
        loop {
            let in_flight = {
                let profiles = self.profiles.lock().await;
                profiles
                    .get(&profile_id)
                    .is_some_and(|state| state.in_flight)
            };
            if !in_flight {
                return;
            }
            self.notify.notified().await;
        }
    }
}

fn seed(profile_id: ProfileId, attempt: u32, now: DateTime<Utc>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    profile_id.hash(&mut hasher);
    attempt.hash(&mut hasher);
    now.timestamp().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;
    use crate::refresh::policy::DeterministicJitter;
    use uuid::Uuid;

    #[derive(Default)]
    struct FakeClock {
        seconds: AtomicI64,
    }

    impl FakeClock {
        fn set(&self, seconds: i64) {
            self.seconds.store(seconds, Ordering::Relaxed);
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> DateTime<Utc> {
            DateTime::from_timestamp(self.seconds.load(Ordering::Relaxed), 0).unwrap()
        }
    }

    #[tokio::test]
    async fn duplicate_refresh_joins_and_manual_obeys_fifteen_second_cooldown() {
        let clock = Arc::new(FakeClock::default());
        clock.set(1_750_000_000);
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_for_closure = std::sync::Arc::clone(&calls);
        let scheduler = RefreshScheduler::new(
            RefreshPolicy::new(300),
            Arc::clone(&clock) as Arc<dyn Clock>,
            Arc::new(DeterministicJitter),
            Arc::new(move |_, _| {
                let calls_for_closure = Arc::clone(&calls_for_closure);
                Box::pin(async move {
                    calls_for_closure.fetch_add(1, Ordering::Relaxed);
                    Ok(RefreshDisposition::Started)
                })
            }),
        );

        assert_eq!(
            scheduler
                .request(Uuid::nil(), RefreshTrigger::Manual)
                .await
                .unwrap(),
            RefreshDisposition::Started
        );
        assert_eq!(
            scheduler
                .request(Uuid::nil(), RefreshTrigger::PanelOpened)
                .await
                .unwrap(),
            RefreshDisposition::Joined
        );
        scheduler.wait_for_refresh(Uuid::nil()).await;
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        clock.set(1_750_000_000 + 10);
        assert!(matches!(
            scheduler
                .request(Uuid::nil(), RefreshTrigger::Manual)
                .await
                .unwrap(),
            RefreshDisposition::Cooldown { .. }
        ));
        clock.set(1_750_000_000 + 16);
        assert_eq!(
            scheduler
                .request(Uuid::nil(), RefreshTrigger::Manual)
                .await
                .unwrap(),
            RefreshDisposition::Started
        );
    }
}
