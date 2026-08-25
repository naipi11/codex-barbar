use crate::pricing::source::{
    PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot,
    snapshot_from_official_document,
};
use chrono::Utc;

pub struct DeepSeekAdapter;

impl PricingSourceAdapter for DeepSeekAdapter {
    fn id(&self) -> PricingSourceId {
        PricingSourceId::DeepSeek
    }

    fn source_url(&self) -> &'static str {
        "https://api-docs.deepseek.com/quick_start/pricing"
    }

    fn parser_revision(&self) -> &'static str {
        "deepseek-official-v1"
    }

    fn parse(&self, body: &str) -> Result<SourceSnapshot, PricingSourceError> {
        snapshot_from_official_document(
            self,
            body,
            Utc::now(),
            "cacheMissInputPerMillion",
            "cacheHitInputPerMillion",
            "outputPerMillion",
        )
    }
}
