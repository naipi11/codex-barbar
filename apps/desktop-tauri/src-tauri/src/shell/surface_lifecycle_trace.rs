//! Privacy-bounded in-memory lifecycle trace for status surfaces.
//!
//! Snapshots never include window titles, browser URLs, account identity,
//! cookies, tokens, or API keys.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::fullscreen_guard::ForegroundClass;

const DEFAULT_CAPACITY: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceLabel {
    TaskbarStatus,
    FloatBall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceSuspensionReason {
    None,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TopmostResult {
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceLifecycleSnapshot {
    pub surface: SurfaceLabel,
    pub desired_visible: bool,
    pub actual_visible: bool,
    pub minimized: bool,
    pub bounds: Option<SurfaceBounds>,
    pub topmost_result: TopmostResult,
    pub foreground_class: ForegroundClass,
    pub suspension_reason: SurfaceSuspensionReason,
    pub observed_at_ms: u64,
}

impl SurfaceLifecycleSnapshot {
    pub fn observed_now(mut self) -> Self {
        self.observed_at_ms = now_ms();
        self
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug)]
pub struct SurfaceLifecycleTrace {
    events: VecDeque<SurfaceLifecycleSnapshot>,
    capacity: usize,
}

impl SurfaceLifecycleTrace {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity.max(1)),
            capacity: capacity.max(1),
        }
    }

    pub fn record(&mut self, snapshot: SurfaceLifecycleSnapshot) {
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(snapshot);
    }

    pub fn recent(&self, limit: usize) -> Vec<SurfaceLifecycleSnapshot> {
        self.events
            .iter()
            .rev()
            .take(limit)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

impl Default for SurfaceLifecycleTrace {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_TRACE: Mutex<Option<SurfaceLifecycleTrace>> = Mutex::new(None);

pub fn record_global(snapshot: SurfaceLifecycleSnapshot) {
    if let Ok(mut guard) = GLOBAL_TRACE.lock() {
        let trace = guard.get_or_insert_with(SurfaceLifecycleTrace::new);
        trace.record(snapshot);
    }
}

pub fn recent_global(limit: usize) -> Vec<SurfaceLifecycleSnapshot> {
    GLOBAL_TRACE
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|trace| trace.recent(limit)))
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn event(id: u64) -> SurfaceLifecycleSnapshot {
    SurfaceLifecycleSnapshot {
        surface: SurfaceLabel::TaskbarStatus,
        desired_visible: true,
        actual_visible: id.is_multiple_of(2),
        minimized: false,
        bounds: Some(SurfaceBounds {
            x: 0,
            y: 0,
            width: 100,
            height: 40,
        }),
        topmost_result: TopmostResult::Ok,
        foreground_class: ForegroundClass::Normal,
        suspension_reason: SurfaceSuspensionReason::None,
        observed_at_ms: id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_discards_oldest_events_at_capacity() {
        let mut trace = SurfaceLifecycleTrace::with_capacity(2);
        trace.record(event(1));
        trace.record(event(2));
        trace.record(event(3));
        assert_eq!(trace.recent(10), vec![event(2), event(3)]);
    }

    #[test]
    fn recent_respects_limit_without_exposing_titles() {
        let mut trace = SurfaceLifecycleTrace::with_capacity(4);
        trace.record(event(1));
        trace.record(event(2));
        trace.record(event(3));
        let recent = trace.recent(2);
        assert_eq!(recent, vec![event(2), event(3)]);
        let encoded = serde_json::to_value(recent).unwrap();
        let json = encoded.to_string();
        assert!(!json.contains("title"));
        assert!(!json.contains("url"));
        assert!(!json.contains("email"));
    }
}
