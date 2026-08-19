//! Best-effort local motion probe for the float-ball animation.
//! Process discovery stays in-process so it never flashes a console window.

use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FloatBallMotion {
    pub thinking: bool,
    pub fast: bool,
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

fn config_looks_fast(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("service_tier = \"fast\"")
        || lower.contains("service_tier=\"fast\"")
        || lower.contains("-fast\"")
        || lower.contains("spark")
}

fn snapshot_looks_active(names: &[String]) -> bool {
    names.iter().any(|name| {
        let lower = name.to_ascii_lowercase();
        lower == "chatgpt.exe" || lower == "codex.exe"
    })
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
        let written = unsafe { K32GetModuleBaseNameW(handle, 0, buf.as_mut_ptr(), buf.len() as u32) };
        unsafe { CloseHandle(handle); }
        if written == 0 {
            continue;
        }
        names.push(std::ffi::OsString::from_wide(&buf[..written as usize]).to_string_lossy().into_owned());
    }
    names
}

#[cfg(not(windows))]
fn query_process_names() -> Vec<String> {
    Vec::new()
}

#[tauri::command]
pub fn get_float_ball_motion() -> FloatBallMotion {
    let config = fs::read_to_string(codex_home().join("config.toml")).unwrap_or_default();
    FloatBallMotion {
        thinking: snapshot_looks_active(&query_process_names()),
        fast: config_looks_fast(&config),
    }
}

#[cfg(test)]
mod tests {
    use super::{config_looks_fast, snapshot_looks_active};

    #[test]
    fn fast_tier_is_detected_from_config() {
        assert!(config_looks_fast("service_tier = \"fast\"\nmodel = \"gpt-5.6-terra\""));
        assert!(!config_looks_fast("service_tier = \"standard\"\nmodel = \"gpt-5.6-terra\""));
    }

    #[test]
    fn chatgpt_process_counts_as_thinking() {
        assert!(snapshot_looks_active(&["codex-barbar.exe".into(), "ChatGPT.exe".into()]));
        assert!(!snapshot_looks_active(&["codex-barbar.exe".into()]));
    }
}
