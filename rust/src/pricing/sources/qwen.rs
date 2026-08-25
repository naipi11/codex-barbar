use crate::pricing::source::{
    PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot,
    snapshot_from_official_document,
};
use chrono::Utc;

pub struct QwenAdapter;

impl PricingSourceAdapter for QwenAdapter {
    fn id(&self) -> PricingSourceId {
        PricingSourceId::Qwen
    }

    fn source_url(&self) -> &'static str {
        "https://www.alibabacloud.com/help/en/model-studio/model-pricing"
    }

    fn parser_revision(&self) -> &'static str {
        "qwen-official-v1"
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
