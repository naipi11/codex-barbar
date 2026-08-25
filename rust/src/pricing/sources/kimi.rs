use crate::pricing::source::{
    PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot,
    snapshot_from_official_document,
};
use chrono::Utc;

pub struct KimiAdapter;

impl PricingSourceAdapter for KimiAdapter {
    fn id(&self) -> PricingSourceId {
        PricingSourceId::Kimi
    }

    fn source_url(&self) -> &'static str {
        "https://platform.kimi.com/docs/pricing/overview"
    }

    fn parser_revision(&self) -> &'static str {
        "kimi-official-v1"
    }

    fn parse(&self, body: &str) -> Result<SourceSnapshot, PricingSourceError> {
        snapshot_from_official_document(
            self,
            body,
            Utc::now(),
            "inputPerMillion",
            "cachedInputPerMillion",
            "outputPerMillion",
        )
    }
}
