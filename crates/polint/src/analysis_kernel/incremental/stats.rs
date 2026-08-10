use serde::{Deserialize, Serialize};

use super::digest::Digest;
use super::keys::PrecisionTier;

pub(crate) use polint_analysis_api::CacheStats;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderOutputMeta {
    pub(crate) provider_id: String,
    pub(crate) provider_version: String,
    pub(crate) schema_version: String,
    pub(crate) output_digest: Digest,
    pub(crate) precision: PrecisionTier,
    pub(crate) validation: String,
    pub(crate) dependency_inputs: Vec<Digest>,
    pub(crate) cache_stats: CacheStats,
}

impl ProviderOutputMeta {
    #[expect(
        clippy::too_many_arguments,
        reason = "Provider output metadata construction keeps every identity and status field explicit."
    )]
    pub(crate) fn new(
        provider_id: impl Into<String>,
        provider_version: impl Into<String>,
        schema_version: impl Into<String>,
        output_digest: Digest,
        precision: PrecisionTier,
        validation: impl Into<String>,
        dependency_inputs: Vec<Digest>,
        cache_stats: CacheStats,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            provider_version: provider_version.into(),
            schema_version: schema_version.into(),
            output_digest,
            precision,
            validation: validation.into(),
            dependency_inputs: sorted_digests(dependency_inputs),
            cache_stats,
        }
    }
}

fn sorted_digests(mut digests: Vec<Digest>) -> Vec<Digest> {
    digests.sort();
    digests
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::digest::{Digest, DigestKind};

    #[test]
    fn provider_output_meta_serializes_provider_identity_output_and_stats() {
        let meta = ProviderOutputMeta::new(
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::from_parts(DigestKind::ProviderOutput, "output", &["facts"]),
            PrecisionTier::Syntax,
            "native_trusted",
            vec![Digest::from_parts(
                DigestKind::SourceText,
                "file",
                &["src/main.ts"],
            )],
            CacheStats::default(),
        );
        let json = serde_json::to_value(meta).expect("provider output metadata should serialize");

        assert!(json.get("provider_id").is_some());
        assert!(json.get("provider_version").is_some());
        assert!(json.get("schema_version").is_some());
        assert!(json.get("output_digest").is_some());
        assert!(json.get("precision").is_some());
        assert!(json.get("validation").is_some());
        assert!(json.get("dependency_inputs").is_some());
        assert!(json.get("cache_stats").is_some());
    }
}
