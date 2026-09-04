//! Strict Current User DPAPI credential vault (implemented in Task 2).

pub mod crypto;
pub mod envelope;
pub mod store;

pub use crypto::{
    CredentialProtector, VaultError, WindowsDpapiProtector, platform_credential_protector,
    platform_managed_credentials_available,
};
#[cfg(target_os = "linux")]
pub use crypto::{LinuxSecretServiceProtector, secret_service_marker};
pub use envelope::{
    CredentialFile, ManagedCredentialBundle, PrivateProfileMetadata, VaultEnvelope,
};
pub use store::{CredentialVault, VaultInfo, VaultRecovery};
