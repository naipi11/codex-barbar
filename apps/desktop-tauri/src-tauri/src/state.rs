use std::sync::Arc;

use crate::app_coordinator::AppCoordinator;
use crate::proof_harness::ProofConfig;
use crate::surface::SurfaceStateMachine;
use crate::surface_target::SurfaceTarget;

/// Central app state behind `Mutex` for Tauri managed state.
pub struct AppState {
    pub surface_machine: SurfaceStateMachine,
    pub current_target: SurfaceTarget,
    /// Proof-harness configuration (set when `CODEXBAR_PROOF_MODE` is active).
    pub proof_config: Option<ProofConfig>,
    /// Phase-2 account service; `None` when the database is read-only.
    pub account_service: Option<Arc<codexbar::accounts::service::AccountProfileService>>,
    /// Central one-shot lifecycle coordinator.
    pub coordinator: Arc<AppCoordinator>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            surface_machine: SurfaceStateMachine::new(),
            current_target: SurfaceTarget::Summary,
            proof_config: None,
            account_service: None,
            coordinator: Arc::new(AppCoordinator::new()),
        }
    }
}
