use crate::pricing::source::{
    PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot,
    snapshot_from_official_document,
};
use chrono::Utc;

pub struct XaiAdapter;

impl PricingSourceAdapter for XaiAdapter {
    fn id(&self) -> PricingSourceId {
        PricingSourceId::Xai
    }

    fn source_url(&self) -> &'static str {
        "https://docs.x.ai/developers/pricing"
    }

    fn parser_revision(&self) -> &'static str {
        "xai-official-v1"
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
