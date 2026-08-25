//! Account profiles, lifecycle, and orchestration (Phase 2).

pub mod actor;
pub mod avatar;
pub mod credential_bundle;
pub mod identity;
pub mod local_identity;
pub mod model;
pub mod presentation;
pub mod recovery;
pub mod runtime_home;
pub mod secret_bytes;
pub mod service;
#[cfg(test)]
pub mod test_support;
pub mod vault;
pub mod windows_acl;

pub use actor::AccountOperationActor;
pub use recovery::{AccountRecovery, RecoveryActionTaken, RecoveryOutcome};
pub use runtime_home::{
    ManagedRuntimeHome, RecoveryCandidate, RuntimeHomeError, RuntimeHomeManager,
};
pub use service::AccountProfileService;
