//! Dynamic, source-labelled model pricing.

pub mod cache;
pub mod catalog;
pub mod model_alias;
pub mod source;
pub mod sources;

pub use cache::{CatalogStore, CatalogStoreError};
pub use catalog::{
    CatalogEntry, CatalogValidationError, ContextTier, CostResolution, Currency, MoneyMicros,
    PriceProvenance, PricingCatalog, PricingResolver, TokenRates,
};
pub use model_alias::{AliasResolution, ModelAliasResolver};
pub use source::{PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot};
