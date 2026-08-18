//! Shared library surface for CodexBar.
//!
//! V1 desktop surface: only the modules needed by the Tauri shell and its
//! Codex App Server provider compile in the release graph.

pub mod accounts;
pub mod app_paths;
pub mod core;
pub mod diagnostics;
pub mod locale;
pub mod logging;
pub mod platform;
pub mod providers;
pub mod refresh;
pub mod rolling_log;
pub mod secure_file;
pub mod settings;
pub mod storage;
pub mod tray;
pub mod update_check;
