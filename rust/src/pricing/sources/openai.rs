use crate::pricing::source::{
    PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot,
    snapshot_from_official_document,
};
use chrono::Utc;

pub struct OpenAiAdapter;

impl PricingSourceAdapter for OpenAiAdapter {
    fn id(&self) -> PricingSourceId {
        PricingSourceId::OpenAi
    }

    fn source_url(&self) -> &'static str {
        "https://platform.openai.com/pricing"
    }

    fn parser_revision(&self) -> &'static str {
        "openai-official-v1"
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
