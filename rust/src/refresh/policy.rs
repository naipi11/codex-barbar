//! Refresh timing policy: intervals, staleness, cooldown, and backoff.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sha2::Digest;

use crate::core::AppErrorKind;

/// Wall-clock abstraction for deterministic scheduler tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// System wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Deterministic jitter source for backoff bounds.
pub trait JitterSource: Send + Sync {
    fn factor(&self, seed: u64) -> f64;
}

/// SHA-256-derived deterministic jitter in `[-0.2, 0.2]`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicJitter;

impl JitterSource for DeterministicJitter {
    fn factor(&self, seed: u64) -> f64 {
        let digest = sha2::Sha256::digest(seed.to_le_bytes());
        let value = u64::from_le_bytes(digest[..8].try_into().unwrap());
        let unit = value as f64 / u64::MAX as f64; // 0..=1
        -0.2 + unit * 0.4
    }
}

pub const ALLOWED_INTERVALS: &[u64] = &[0, 60, 300, 900, 1800];
pub const DEFAULT_INTERVAL_SECS: u64 = 300;
pub const MANUAL_COOLDOWN: Duration = Duration::from_secs(15);
pub const PANEL_REFRESH_THRESHOLD: Duration = Duration::from_secs(60);

const TRANSIENT_BACKOFFS: [Duration; 4] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(300),
    Duration::from_secs(900),
];

/// Immutable refresh timing policy.
#[derive(Debug, Clone, Copy)]
pub struct RefreshPolicy {
    interval: Duration,
}

impl RefreshPolicy {
    pub fn new(interval_secs: u64) -> Self {
        let interval_secs = if ALLOWED_INTERVALS.contains(&interval_secs) {
            interval_secs
        } else {
            DEFAULT_INTERVAL_SECS
        };
        Self {
            interval: Duration::from_secs(interval_secs),
        }
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn fresh_for(&self) -> Duration {
        self.interval * 2
    }

    pub fn stale_after(&self) -> Duration {
        (self.interval * 2).max(Duration::from_secs(600))
    }

    pub fn backoff_after(&self, attempt: u32) -> Duration {
        TRANSIENT_BACKOFFS[attempt.min(3) as usize]
    }

    pub fn jittered_backoff(&self, attempt: u32, jitter: &dyn JitterSource, seed: u64) -> Duration {
        let base = self.backoff_after(attempt);
        let factor = jitter.factor(seed);
        let seconds = (base.as_secs_f64() * (1.0 + factor)).round().max(1.0);
        Duration::from_secs_f64(seconds)
    }

    pub fn manual_cooldown(&self) -> Duration {
        MANUAL_COOLDOWN
    }

    pub fn panel_refresh_threshold(&self) -> Duration {
        PANEL_REFRESH_THRESHOLD
    }

    pub fn blocks(&self, kind: AppErrorKind) -> bool {
        matches!(
            kind,
            AppErrorKind::NotSignedIn
                | AppErrorKind::AuthExpired
                | AppErrorKind::UnsupportedCodexVersion
                | AppErrorKind::VaultFailure
                | AppErrorKind::StorageFailure
        )
    }

    pub fn retry_after(
        &self,
        error_kind: AppErrorKind,
        now: DateTime<Utc>,
        retry_after: Option<DateTime<Utc>>,
    ) -> Option<Duration> {
        if error_kind != AppErrorKind::RateLimited {
            return None;
        }
        let retry_after = retry_after?;
        let delay = retry_after.signed_duration_since(now).to_std().ok()?;
        if delay < Duration::from_secs(1) || delay > Duration::from_secs(24 * 3600) {
            return None;
        }
        Some(delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_threshold_is_max_of_twice_interval_and_ten_minutes() {
        let policy = RefreshPolicy::new(60);
        assert_eq!(policy.stale_after(), Duration::from_secs(600));
        let policy = RefreshPolicy::new(900);
        assert_eq!(policy.stale_after(), Duration::from_secs(1800));
    }

    #[test]
    fn allowed_intervals_and_default_fallback() {
        for interval in ALLOWED_INTERVALS {
            assert_eq!(
                RefreshPolicy::new(*interval).interval(),
                Duration::from_secs(*interval)
            );
        }
        assert_eq!(
            RefreshPolicy::new(123).interval(),
            Duration::from_secs(DEFAULT_INTERVAL_SECS)
        );
    }

    #[test]
    fn jitter_stays_within_plus_minus_twenty_percent() {
        let policy = RefreshPolicy::new(300);
        for seed in 0..100 {
            let delay = policy.jittered_backoff(2, &DeterministicJitter, seed);
            let ratio = delay.as_secs_f64() / policy.backoff_after(2).as_secs_f64();
            assert!((0.8..=1.2).contains(&ratio), "seed {seed} ratio {ratio}");
        }
    }

    #[test]
    fn blocking_kinds_never_get_transient_backoff() {
        let policy = RefreshPolicy::new(300);
        assert!(policy.blocks(AppErrorKind::AuthExpired));
        assert!(policy.blocks(AppErrorKind::UnsupportedCodexVersion));
        assert!(!policy.blocks(AppErrorKind::OfflineOrTimeout));
    }
}
