//! V1 desktop command surface.
//!
//! Only the Phase-0 tray/settings commands are registered; the legacy
//! provider/cookie/key/chart command modules remain in Git history and are
//! deleted from the source tree in Phase 4.

mod accounts;
mod app;
mod bridge;
mod diagnostics;
mod fixed_actions;
mod settings;
mod status_surfaces;
mod update;
mod usage_spend;
mod window;

pub use accounts::*;
pub use bridge::*;
pub use diagnostics::*;
pub use fixed_actions::*;
pub use settings::*;
pub use status_surfaces::*;
pub use update::*;
pub use usage_spend::*;
pub use window::*;
