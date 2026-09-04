//! Foreground-change observation for status-surface reconciliation.
//!
//! The WinEvent callback only records a pending class and schedules a short
//! non-blocking reconcile. It never takes the status-surface mutex or calls
//! WebView APIs.

#[cfg(windows)]
use std::sync::OnceLock;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use super::fullscreen_guard::{ForegroundClass, classify_current_foreground};

#[cfg(windows)]
const CLASS_NORMAL: u8 = 0;
#[cfg(windows)]
const CLASS_SHELL_TRANSIENT: u8 = 1;
#[cfg(windows)]
const CLASS_REAL_FULLSCREEN: u8 = 2;

#[cfg(windows)]
static PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static LAST_CLASS: AtomicU8 = AtomicU8::new(CLASS_NORMAL);
#[cfg(windows)]
static STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

#[cfg(windows)]
fn encode(class: ForegroundClass) -> u8 {
    match class {
        ForegroundClass::Normal => CLASS_NORMAL,
        ForegroundClass::ShellTransient => CLASS_SHELL_TRANSIENT,
        ForegroundClass::RealFullscreen => CLASS_REAL_FULLSCREEN,
    }
}

#[cfg(windows)]
fn decode(value: u8) -> ForegroundClass {
    match value {
        CLASS_SHELL_TRANSIENT => ForegroundClass::ShellTransient,
        CLASS_REAL_FULLSCREEN => ForegroundClass::RealFullscreen,
        _ => ForegroundClass::Normal,
    }
}

#[cfg(windows)]
pub fn last_foreground_class() -> ForegroundClass {
    decode(LAST_CLASS.load(Ordering::Relaxed))
}

#[cfg(windows)]
fn note_foreground_change() {
    let class = classify_current_foreground();
    LAST_CLASS.store(encode(class), Ordering::Relaxed);
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(app) = APP.get() else {
        PENDING.store(false, Ordering::SeqCst);
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(16)).await;
        PENDING.store(false, Ordering::SeqCst);
        crate::status_surfaces::schedule_foreground_reconcile(app, last_foreground_class());
    });
}

#[cfg(windows)]
mod native {
    use super::note_foreground_change;

    const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
    const EVENT_OBJECT_SHOW: u32 = 0x8002;
    const EVENT_OBJECT_HIDE: u32 = 0x8003;
    const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
    const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWinEventHook(
            event_min: u32,
            event_max: u32,
            module: isize,
            callback: Option<unsafe extern "system" fn(isize, u32, isize, i32, i32, u32, u32)>,
            process_id: u32,
            thread_id: u32,
            flags: u32,
        ) -> isize;
    }

    unsafe extern "system" fn on_win_event(
        _hook: isize,
        _event: u32,
        _hwnd: isize,
        _id_object: i32,
        _id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        note_foreground_change();
    }

    pub fn install_hook() -> bool {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                0,
                Some(on_win_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        let _ = (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE, hook);
        hook != 0
    }
}

#[cfg(windows)]
pub fn start_foreground_event_monitor(app: tauri::AppHandle) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = APP.set(app);
    LAST_CLASS.store(encode(classify_current_foreground()), Ordering::Relaxed);
    if !native::install_hook() {
        tracing::debug!(
            code = "FOREGROUND_HOOK_UNAVAILABLE",
            "foreground WinEvent hook unavailable; 250ms fallback remains"
        );
    }
}

#[cfg(not(windows))]
pub fn start_foreground_event_monitor(_app: tauri::AppHandle) {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn class_codec_round_trips() {
        for class in [
            ForegroundClass::Normal,
            ForegroundClass::ShellTransient,
            ForegroundClass::RealFullscreen,
        ] {
            assert_eq!(decode(encode(class)), class);
        }
    }
}
