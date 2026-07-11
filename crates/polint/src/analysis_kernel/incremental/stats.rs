use serde::{Deserialize, Serialize};

use super::digest::Digest;
use super::keys::PrecisionTier;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CacheStats {
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) recomputes: u64,
    pub(crate) writes: u64,
    pub(crate) bypasses_disabled: u64,
    pub(crate) invalid_evicted_reads: u64,
    pub(crate) verified_reuse: u64,
    pub(crate) quarantines: u64,
}

impl CacheStats {
    pub(crate) fn record_hit(&mut self) {
        self.hits += 1;
    }

    pub(crate) fn record_miss(&mut self) {
        self.misses += 1;
    }

    pub(crate) fn record_recompute(&mut self) {
        self.recomputes += 1;
    }

    pub(crate) fn record_write(&mut self) {
        self.writes += 1;
    }

    pub(crate) fn record_disabled_bypass(&mut self) {
        self.bypasses_disabled += 1;
    }

    pub(crate) fn record_invalid_evicted_read(&mut self) {
        self.invalid_evicted_reads += 1;
    }

    pub(crate) fn record_verified_reuse(&mut self) {
        self.verified_reuse += 1;
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "kept for private internal consumers")
    )]
    pub(crate) fn record_quarantine(&mut self) {
        self.quarantines += 1;
    }
}

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
    fn default_cache_stats_serialize_zero_counters() {
        let stats = CacheStats::default();
        let json = serde_json::to_value(stats).expect("cache stats should serialize");

        assert_eq!(json["hits"], 0);
        assert_eq!(json["misses"], 0);
        assert_eq!(json["recomputes"], 0);
        assert_eq!(json["writes"], 0);
        assert_eq!(json["bypasses_disabled"], 0);
        assert_eq!(json["invalid_evicted_reads"], 0);
        assert_eq!(json["verified_reuse"], 0);
        assert_eq!(json["quarantines"], 0);
    }

    #[test]
    fn record_methods_increment_each_counter() {
        let mut stats = CacheStats::default();

        stats.record_hit();
        stats.record_miss();
        stats.record_recompute();
        stats.record_write();
        stats.record_disabled_bypass();
        stats.record_invalid_evicted_read();
        stats.record_verified_reuse();
        stats.record_quarantine();

        assert_eq!(
            stats,
            CacheStats {
                hits: 1,
                misses: 1,
                recomputes: 1,
                writes: 1,
                bypasses_disabled: 1,
                invalid_evicted_reads: 1,
                verified_reuse: 1,
                quarantines: 1,
            }
        );
    }

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
