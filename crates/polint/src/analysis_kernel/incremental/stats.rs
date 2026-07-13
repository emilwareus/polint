use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use super::digest::Digest;
use super::keys::PrecisionTier;
use super::layer_cache::LayerRunMetadata;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderValidationStatus {
    NativeTrusted,
    ProviderFailed,
}

impl ProviderValidationStatus {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::NativeTrusted => "native_trusted",
            Self::ProviderFailed => "provider_failed",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownProviderValidationStatusLabel> {
        match label {
            "native_trusted" => Ok(Self::NativeTrusted),
            "provider_failed" => Ok(Self::ProviderFailed),
            _ => Err(UnknownProviderValidationStatusLabel {
                label: label.to_string(),
            }),
        }
    }
}

impl<'de> Deserialize<'de> for ProviderValidationStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let label = String::deserialize(deserializer)?;
        Self::parse_label(&label).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownProviderValidationStatusLabel {
    label: String,
}

impl fmt::Display for UnknownProviderValidationStatusLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown provider validation status label `{}`",
            self.label
        )
    }
}

impl std::error::Error for UnknownProviderValidationStatusLabel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct ProviderSemanticProjection<'a> {
    pub(crate) provider_id: &'a str,
    pub(crate) provider_version: &'a str,
    pub(crate) schema_version: &'a str,
    pub(crate) output_digest: &'a Digest,
    pub(crate) precision: PrecisionTier,
    pub(crate) validation: ProviderValidationStatus,
    pub(crate) dependency_inputs: &'a [Digest],
    pub(crate) layers: &'a [LayerRunMetadata],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderOutputMeta {
    pub(crate) provider_id: String,
    pub(crate) provider_version: String,
    pub(crate) schema_version: String,
    pub(crate) output_digest: Digest,
    pub(crate) precision: PrecisionTier,
    pub(crate) validation: ProviderValidationStatus,
    pub(crate) dependency_inputs: Vec<Digest>,
    pub(crate) layers: Vec<LayerRunMetadata>,
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
        validation: ProviderValidationStatus,
        dependency_inputs: Vec<Digest>,
        layers: Vec<LayerRunMetadata>,
        cache_stats: CacheStats,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            provider_version: provider_version.into(),
            schema_version: schema_version.into(),
            output_digest,
            precision,
            validation,
            dependency_inputs: sorted_digests(dependency_inputs),
            layers: sorted_layers(layers),
            cache_stats,
        }
    }

    pub(crate) fn semantic_projection(&self) -> ProviderSemanticProjection<'_> {
        ProviderSemanticProjection {
            provider_id: &self.provider_id,
            provider_version: &self.provider_version,
            schema_version: &self.schema_version,
            output_digest: &self.output_digest,
            precision: self.precision,
            validation: self.validation,
            dependency_inputs: &self.dependency_inputs,
            layers: &self.layers,
        }
    }
}

fn sorted_digests(mut digests: Vec<Digest>) -> Vec<Digest> {
    digests.sort();
    digests
}

fn sorted_layers(mut layers: Vec<LayerRunMetadata>) -> Vec<LayerRunMetadata> {
    layers.sort();
    layers.dedup();
    layers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        DigestKind, LayerCacheManifest, LayerKey, LayerKind,
    };

    fn layer(label: &str, layer_kind: LayerKind) -> LayerRunMetadata {
        let key = LayerKey::new(
            layer_kind,
            format!("polint.{label}"),
            "1",
            format!("{label}-v1"),
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::AnalysisSettings, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![Digest::from_parts(DigestKind::SourceText, label, &[label])],
            Vec::new(),
            Vec::new(),
        );
        LayerRunMetadata::from_manifest(LayerCacheManifest::new(
            key,
            Digest::from_parts(DigestKind::LayerOutput, label, &["output"]),
            Digest::from_parts(DigestKind::LayerOutput, label, &["payload"]),
            Vec::new(),
            PrecisionTier::Syntax,
            ProviderValidationStatus::NativeTrusted,
            vec!["z-warning".to_string(), "a-warning".to_string()],
        ))
    }

    fn permutations<T: Clone>(values: &[T]) -> Vec<Vec<T>> {
        fn visit<T: Clone>(remaining: Vec<T>, prefix: Vec<T>, output: &mut Vec<Vec<T>>) {
            if remaining.is_empty() {
                output.push(prefix);
                return;
            }
            for index in 0..remaining.len() {
                let mut next_remaining = remaining.clone();
                let value = next_remaining.remove(index);
                let mut next_prefix = prefix.clone();
                next_prefix.push(value);
                visit(next_remaining, next_prefix, output);
            }
        }

        let mut output = Vec::new();
        visit(values.to_vec(), Vec::new(), &mut output);
        output
    }

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
            ProviderValidationStatus::NativeTrusted,
            vec![Digest::from_parts(
                DigestKind::SourceText,
                "file",
                &["src/main.ts"],
            )],
            Vec::new(),
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
        assert!(json.get("layers").is_some());
        assert!(json.get("cache_stats").is_some());
        assert_eq!(json["validation"], "native_trusted");
    }

    #[test]
    fn provider_validation_status_labels_round_trip_and_reject_unknown_values() {
        for status in [
            ProviderValidationStatus::NativeTrusted,
            ProviderValidationStatus::ProviderFailed,
        ] {
            assert_eq!(
                ProviderValidationStatus::parse_label(status.label()),
                Ok(status)
            );
            assert_eq!(
                serde_json::to_string(&status).expect("status serializes"),
                format!("\"{}\"", status.label())
            );
        }

        assert!(ProviderValidationStatus::parse_label("metadata_conflict").is_err());
        assert!(serde_json::from_str::<ProviderValidationStatus>("\"metadata_conflict\"").is_err());
    }

    #[test]
    fn cache_counter_mutations_preserve_provider_semantic_projection() {
        let base = ProviderOutputMeta::new(
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::from_parts(DigestKind::ProviderOutput, "output", &["facts"]),
            PrecisionTier::Syntax,
            ProviderValidationStatus::NativeTrusted,
            vec![Digest::from_parts(
                DigestKind::SourceText,
                "file",
                &["src/main.ts"],
            )],
            vec![layer("ts.syntax", LayerKind::TsSyntax)],
            CacheStats::default(),
        );
        let expected = base.semantic_projection();

        for stats in [
            CacheStats {
                hits: 1,
                ..CacheStats::default()
            },
            CacheStats {
                misses: 1,
                ..CacheStats::default()
            },
            CacheStats {
                recomputes: 1,
                ..CacheStats::default()
            },
            CacheStats {
                writes: 1,
                ..CacheStats::default()
            },
            CacheStats {
                bypasses_disabled: 1,
                ..CacheStats::default()
            },
            CacheStats {
                invalid_evicted_reads: 1,
                ..CacheStats::default()
            },
            CacheStats {
                verified_reuse: 1,
                ..CacheStats::default()
            },
            CacheStats {
                quarantines: 1,
                ..CacheStats::default()
            },
        ] {
            let mut changed = base.clone();
            changed.cache_stats = stats;
            assert_eq!(changed.semantic_projection(), expected);
            assert_eq!(changed.output_digest, base.output_digest);
        }
    }

    #[test]
    fn provider_and_layer_rows_are_identical_across_completion_order_permutations() {
        let layers = vec![
            layer("go.syntax", LayerKind::GoSyntax),
            layer("ts.syntax", LayerKind::TsSyntax),
            layer("module_graph", LayerKind::ModuleGraph),
            layer("metrics", LayerKind::Metrics),
        ];
        let permutations = permutations(&layers);
        assert_eq!(permutations.len(), 24);

        let expected = ProviderOutputMeta::new(
            "polint.aggregate",
            "1",
            "aggregate-v1",
            Digest::from_parts(DigestKind::ProviderOutput, "aggregate", &["facts"]),
            PrecisionTier::Syntax,
            ProviderValidationStatus::NativeTrusted,
            Vec::new(),
            layers,
            CacheStats::default(),
        );

        for mut completion_order in permutations {
            completion_order.push(completion_order[0].clone());
            let actual = ProviderOutputMeta::new(
                "polint.aggregate",
                "1",
                "aggregate-v1",
                Digest::from_parts(DigestKind::ProviderOutput, "aggregate", &["facts"]),
                PrecisionTier::Syntax,
                ProviderValidationStatus::NativeTrusted,
                Vec::new(),
                completion_order,
                CacheStats {
                    hits: 1,
                    ..CacheStats::default()
                },
            );

            assert_eq!(actual.layers, expected.layers);
            assert_eq!(actual.semantic_projection(), expected.semantic_projection());
        }
    }

    #[test]
    fn provider_semantic_projection_source_excludes_cache_telemetry() {
        let source = include_str!("stats.rs");
        let projection = source
            .split_once("pub(crate) fn semantic_projection")
            .expect("semantic projection exists")
            .1
            .split_once("fn sorted_digests")
            .expect("semantic projection has a bounded source section")
            .0;

        for forbidden in [
            "cache_stats",
            "hits",
            "misses",
            "recomputes",
            "writes",
            "bypasses_disabled",
            "invalid_evicted_reads",
            "verified_reuse",
            "quarantines",
        ] {
            assert!(
                !projection.contains(forbidden),
                "provider semantic projection must exclude `{forbidden}`"
            );
        }
    }
}
