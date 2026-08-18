//! Codex-only provider factory for the V1 desktop release.
//!
//! The V1 product ships exactly one provider. Non-Codex `ProviderId` values
//! remain only as enum compatibility and are rejected fail-closed.

use super::{Provider, ProviderError, ProviderId};
use crate::providers::codex::CodexProvider;

/// Provider IDs that ship in the codex-barbar V1 desktop product.
pub const fn shipping_provider_ids() -> &'static [ProviderId] {
    &[ProviderId::Codex]
}

/// Instantiate the only provider in the V1 release surface.
pub fn instantiate_shipping_provider(id: ProviderId) -> Result<Box<dyn Provider>, ProviderError> {
    if id != ProviderId::Codex {
        return Err(ProviderError::UnsupportedProvider(id.cli_name().to_owned()));
    }
    Ok(Box::new(CodexProvider::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_shipping_registry_contains_only_codex() {
        assert_eq!(shipping_provider_ids(), &[ProviderId::Codex]);
    }

    #[test]
    fn factory_rejects_non_codex_provider() {
        match instantiate_shipping_provider(ProviderId::Claude) {
            Err(error) => assert!(matches!(error, ProviderError::UnsupportedProvider(_))),
            Ok(_) => panic!("Claude must not be instantiable in the shipping registry"),
        }
    }
}
