//! Best-effort local motion probe for the float-ball animation.
//! Process discovery stays in-process so it never flashes a console window.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::Emitter;

pub const FLOAT_BALL_MOTION_CHANGED: &str = "codexbar:float-ball-motion-changed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MotionState {
    Idle,
    Thinking,
    Fast,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatBallMotion {
    pub state: MotionState,
    pub observed_at: DateTime<Utc>,
    pub thinking: bool,
    pub fast: bool,
}

impl FloatBallMotion {
    fn from_flags(thinking: bool, fast: bool) -> Self {
        Self {
            state: derive_motion(thinking, fast),
            observed_at: Utc::now(),
            thinking,
            fast,
        }
    }
}

pub fn derive_motion(thinking: bool, fast: bool) -> MotionState {
    if fast {
        MotionState::Fast
    } else if thinking {
        MotionState::Thinking
    } else {
        MotionState::Idle
    }
}

fn user_profile() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn codex_home() -> PathBuf {
    if let Some(value) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(value);
    }
    user_profile()
        .map(|home| home.join(".codex"))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn config_path() -> PathBuf {
    codex_home().join("config.toml")
}

fn config_looks_fast(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("service_tier = \"fast\"")
        || lower.contains("service_tier=\"fast\"")
        || lower.contains("model_reasoning_effort = \"fast\"")
        || lower.contains("model_reasoning_effort=\"fast\"")
}

fn snapshot_looks_active(names: &[String]) -> bool {
    names.iter().any(|name| {
        let lower = name.to_ascii_lowercase();
        lower == "chatgpt.exe" || lower == "codex.exe"
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConfigMetadata {
    modified: Option<SystemTime>,
    len: u64,
}

struct MotionMonitor {
    last_metadata: Option<ConfigMetadata>,
    last_fast: bool,
    parse_count: usize,
    last_state: Option<MotionState>,
}

impl MotionMonitor {
    fn new() -> Self {
        Self {
            last_metadata: None,
            last_fast: false,
            parse_count: 0,
            last_state: None,
        }
    }

    fn tick(
        &mut self,
        metadata: ConfigMetadata,
        contents: Option<&str>,
        thinking: bool,
    ) -> Option<FloatBallMotion> {
        if self.last_metadata != Some(metadata) {
            self.last_metadata = Some(metadata);
            if let Some(text) = contents {
                self.parse_count += 1;
                self.last_fast = config_looks_fast(text);
            }
        }
        let motion = FloatBallMotion::from_flags(thinking, self.last_fast);
        if self.last_state == Some(motion.state) {
            return None;
        }
        self.last_state = Some(motion.state);
        Some(motion)
    }

    #[cfg(test)]
    fn parse_count(&self) -> usize {
        self.parse_count
    }
}

fn read_config_metadata(path: &PathBuf) -> ConfigMetadata {
    match fs::metadata(path) {
        Ok(meta) => ConfigMetadata {
            modified: meta.modified().ok(),
            len: meta.len(),
        },
        Err(_) => ConfigMetadata {
            modified: None,
            len: 0,
        },
    }
}

#[cfg(windows)]
fn query_process_names() -> Vec<String> {
    use std::mem::size_of;
    use std::os::windows::ffi::OsStringExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn K32EnumProcesses(lpid_process: *mut u32, cb: u32, cb_needed: *mut u32) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn CloseHandle(handle: isize) -> i32;
        fn K32GetModuleBaseNameW(process: isize, module: isize, name: *mut u16, size: u32) -> u32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_VM_READ: u32 = 0x0010;

    let mut pids = vec![0u32; 1024];
    let mut needed = 0u32;
    let ok = unsafe {
        K32EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * size_of::<u32>()) as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return Vec::new();
    }
    let count = (needed as usize / size_of::<u32>()).min(pids.len());
    let mut names = Vec::new();
    for pid in pids.into_iter().take(count) {
        if pid == 0 {
            continue;
        }
        let handle = unsafe {
            let limited = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
            if limited != 0 {
                limited
            } else {
                OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid)
            }
        };
        if handle == 0 {
            continue;
        }
        let mut buf = [0u16; 260];
        let written =
            unsafe { K32GetModuleBaseNameW(handle, 0, buf.as_mut_ptr(), buf.len() as u32) };
        unsafe {
            CloseHandle(handle);
        }
        if written == 0 {
            continue;
        }
        names.push(
            std::ffi::OsString::from_wide(&buf[..written as usize])
                .to_string_lossy()
                .into_owned(),
        );
    }
    names
}

#[cfg(not(windows))]
fn query_process_names() -> Vec<String> {
    Vec::new()
}

fn current_motion(monitor: &mut MotionMonitor) -> FloatBallMotion {
    let path = config_path();
    let metadata = read_config_metadata(&path);
    let contents = if monitor.last_metadata != Some(metadata) {
        fs::read_to_string(&path).ok()
    } else {
        None
    };
    let thinking = snapshot_looks_active(&query_process_names());
    monitor
        .tick(metadata, contents.as_deref(), thinking)
        .unwrap_or_else(|| FloatBallMotion::from_flags(thinking, monitor.last_fast))
}

#[tauri::command]
pub fn get_float_ball_motion() -> FloatBallMotion {
    let mut monitor = MotionMonitor::new();
    current_motion(&mut monitor)
}

pub fn start_float_ball_motion_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut monitor = MotionMonitor::new();
        loop {
            interval.tick().await;
            let path = config_path();
            let metadata = read_config_metadata(&path);
            let contents = if monitor.last_metadata != Some(metadata) {
                fs::read_to_string(&path).ok()
            } else {
                None
            };
            let thinking = snapshot_looks_active(&query_process_names());
            if let Some(motion) = monitor.tick(metadata, contents.as_deref(), thinking)
                && app.emit(FLOAT_BALL_MOTION_CHANGED, motion).is_err()
            {
                tracing::debug!(
                    code = "FLOAT_BALL_MOTION_EMIT_FAILED",
                    "float ball motion event was not delivered"
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(id: u64) -> ConfigMetadata {
        ConfigMetadata {
            modified: None,
            len: id,
        }
    }

    fn test_monitor(initial: &str) -> MotionMonitor {
        let mut monitor = MotionMonitor::new();
        monitor.tick(metadata(1), Some(initial), false);
        monitor
    }

    #[test]
    fn explicit_fast_tier_wins_over_thinking() {
        assert_eq!(derive_motion(true, true), MotionState::Fast);
        assert_eq!(derive_motion(true, false), MotionState::Thinking);
        assert_eq!(derive_motion(false, false), MotionState::Idle);
    }

    #[test]
    fn unchanged_config_metadata_does_not_reparse() {
        let mut monitor = test_monitor("service_tier = \"fast\"");
        monitor.tick(metadata(1), Some("service_tier = \"fast\""), false);
        monitor.tick(metadata(1), Some("service_tier = \"fast\""), false);
        assert_eq!(monitor.parse_count(), 1);
    }

    #[test]
    fn fast_tier_is_detected_from_config() {
        assert!(config_looks_fast(
            "service_tier = \"fast\"\nmodel = \"gpt-5.6-terra\""
        ));
        assert!(!config_looks_fast(
            "service_tier = \"standard\"\nmodel = \"gpt-5.6-terra\""
        ));
        assert!(!config_looks_fast("model = \"gpt-5.4-spark\""));
    }

    #[test]
    fn chatgpt_process_counts_as_thinking() {
        assert!(snapshot_looks_active(&[
            "codex-barbar.exe".into(),
            "ChatGPT.exe".into()
        ]));
        assert!(!snapshot_looks_active(&["codex-barbar.exe".into()]));
    }
}
