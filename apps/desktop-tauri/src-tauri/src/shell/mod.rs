//! V1 shell behavior: the configured `main` WebView is the tray flyout and
//! `settings` is the sole detached window.
//!
//! The shared-window surface state machine from the legacy multi-provider
//! shell is gone. The main WebView starts hidden and is shown only while the
//! tray panel is open.

pub(crate) mod dwm;
pub mod flyout_window;
pub(crate) mod fullscreen_guard;
pub mod settings_window;
