//! Strict Current User DPAPI credential vault (implemented in Task 2).

pub mod crypto;
pub mod envelope;
pub mod store;

pub use crypto::{CredentialProtector, VaultError, WindowsDpapiProtector};
pub use envelope::{
    CredentialFile, ManagedCredentialBundle, PrivateProfileMetadata, VaultEnvelope,
};
pub use store::{CredentialVault, VaultInfo, VaultRecovery};
