use super::demand::DemandQueryTrace;
use super::{
    CacheStats, DemandCacheStatus, Digest, DigestKind, InputSnapshot, PrecisionTier,
    ProviderOutputMeta, ProviderValidationStatus,
};
#[cfg(test)]
use crate::analysis::summaries::provider::SccClosureDebugSnapshot;
use crate::analysis_kernel::{ProviderManifest, StableFactMetaRow, StoreStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelRunReport {
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) provider_outputs: Vec<ProviderOutputMeta>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) demand_query_trace: DemandQueryTrace,
    store_status: StoreStatus,
    #[cfg(test)]
    pub(crate) scc_closure_debug: Option<SccClosureDebugSnapshot>,
}

impl KernelRunReport {
    pub(crate) fn new(
        input_snapshot: InputSnapshot,
        provider_outputs: Vec<ProviderOutputMeta>,
        demand_query_trace: DemandQueryTrace,
        store_status: StoreStatus,
    ) -> Self {
        let mut cache_stats = aggregate_cache_stats(&provider_outputs);
        aggregate_demand_query_stats(&demand_query_trace, &mut cache_stats);

        Self {
            input_snapshot,
            provider_outputs,
            cache_stats,
            demand_query_trace,
            store_status,
            #[cfg(test)]
            scc_closure_debug: None,
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "demand trace is currently surfaced through test-only metadata debug output"
        )
    )]
    pub(crate) fn demand_query_trace(&self) -> &DemandQueryTrace {
        &self.demand_query_trace
    }

    #[cfg(test)]
    pub(crate) fn with_scc_closure_debug(mut self, debug: Option<SccClosureDebugSnapshot>) -> Self {
        self.scc_closure_debug = debug;
        self
    }

    #[cfg(test)]
    pub(crate) fn scc_closure_debug(&self) -> Option<&SccClosureDebugSnapshot> {
        self.scc_closure_debug.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn store_status(&self) -> &StoreStatus {
        &self.store_status
    }
}

pub(crate) fn provider_output_from_manifest(
    manifest: &ProviderManifest,
    output_digest: Digest,
    cache_stats: CacheStats,
) -> ProviderOutputMeta {
    ProviderOutputMeta::new(
        manifest.id,
        manifest.provider_version(),
        manifest.primary_schema_label(),
        output_digest,
        PrecisionTier::from_ceiling(manifest.precision_ceiling),
        ProviderValidationStatus::NativeTrusted,
        dependency_inputs_from_manifest(manifest),
        cache_stats,
    )
}

pub(crate) fn provider_output_digest_from_manifest(
    manifest: &ProviderManifest,
    stable_rows: &[StableFactMetaRow],
) -> Digest {
    let schema_label = manifest.primary_schema_label();
    let language_scope = manifest.language_scope_label();
    let cache_policy = manifest.cache_policy_label();
    let precision = manifest.precision_ceiling.label();

    let mut output_families = manifest.outputs.to_vec();
    output_families.sort();
    output_families.dedup();

    let mut metadata_rows = stable_rows.to_vec();
    metadata_rows.sort();
    metadata_rows.dedup();

    let mut builder = Digest::builder(DigestKind::ProviderOutput, "provider_output");
    builder.labeled_part("provider_id", manifest.id);
    builder.labeled_part("schema_version", &schema_label);
    builder.labeled_part("language_scope", language_scope);
    builder.labeled_part("cache_policy", &cache_policy);
    builder.labeled_part("precision", precision);
    for output_family in output_families {
        builder.labeled_part("output_family", output_family);
    }
    for row in metadata_rows {
        builder.labeled_part("fact_family", row.family.label());
        builder.labeled_part("stable_key", &row.stable_key);
        builder.labeled_part("producer_id", &row.producer_id);
        builder.labeled_part("layer_id", &row.layer_id);
        builder.labeled_part("fact_precision", row.precision.label());
        builder.labeled_part("fact_confidence", row.confidence.label());
        builder.labeled_part("validation", row.validation.label());
        builder.labeled_part("payload_digest", &row.payload_digest);
    }

    builder.finish()
}

fn dependency_inputs_from_manifest(manifest: &ProviderManifest) -> Vec<Digest> {
    let mut inputs = manifest.inputs.to_vec();
    inputs.sort();
    inputs
        .into_iter()
        .map(|input| Digest::from_parts(DigestKind::DependencyLayer, "dependency_input", &[input]))
        .collect()
}

/// Aggregates demand query cache statistics into an existing `CacheStats`.
///
/// Cache hits count as hits; cache misses and freshly computed results count
/// as recomputes.
fn aggregate_demand_query_stats(trace: &DemandQueryTrace, stats: &mut CacheStats) {
    for entry in trace.entries() {
        match entry.cache_status {
            DemandCacheStatus::Hit => stats.hits += 1,
            DemandCacheStatus::Miss | DemandCacheStatus::Computed => stats.recomputes += 1,
        }
    }
}

fn aggregate_cache_stats(provider_outputs: &[ProviderOutputMeta]) -> CacheStats {
    let mut aggregate = CacheStats::default();
    for output in provider_outputs {
        aggregate.hits += output.cache_stats.hits;
        aggregate.misses += output.cache_stats.misses;
        aggregate.recomputes += output.cache_stats.recomputes;
        aggregate.writes += output.cache_stats.writes;
        aggregate.bypasses_disabled += output.cache_stats.bypasses_disabled;
        aggregate.invalid_evicted_reads += output.cache_stats.invalid_evicted_reads;
        aggregate.verified_reuse += output.cache_stats.verified_reuse;
        aggregate.quarantines += output.cache_stats.quarantines;
    }
    aggregate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{CacheStats, DigestKind, PrecisionTier};
    use crate::analysis_kernel::{
        CachePolicy, FactConfidence, FactFamily, FactPrecision, LanguageScope, PrecisionCeiling,
        ProviderKind, ProviderManifest, SchemaVersion, StableFactMetaRow, ValidationStatus,
    };

    fn stable_fact_row(
        manifest: &ProviderManifest,
        stable_key: &str,
        payload_digest: &str,
    ) -> StableFactMetaRow {
        StableFactMetaRow {
            family: FactFamily::Import,
            stable_key: stable_key.to_string(),
            producer_id: manifest.id.to_string(),
            layer_id: manifest.id.to_string(),
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }

    #[test]
    fn provider_outputs_are_constructed_in_manifest_order() {
        let provider_outputs = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| {
                let row = stable_fact_row(manifest, "fixture:fact", "fixture:payload");
                let output_digest = provider_output_digest_from_manifest(manifest, &[row]);
                provider_output_from_manifest(manifest, output_digest, CacheStats::default())
            })
            .collect::<Vec<_>>();

        assert_eq!(
            provider_outputs
                .iter()
                .map(|output| output.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.module_topology",
                "polint.semantic_mir",
                "polint.cfg",
                "polint.calls",
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn provider_output_rows_include_manifest_identity_digest_dependencies_and_stats() {
        let mut stats = CacheStats::default();
        stats.record_miss();
        stats.record_recompute();
        stats.record_write();

        for manifest in crate::analysis_kernel::AnalysisKernel::provider_manifests() {
            let row = stable_fact_row(manifest, "fixture:fact", "fixture:payload");
            let output_digest = provider_output_digest_from_manifest(manifest, &[row]);
            let row = provider_output_from_manifest(manifest, output_digest, stats.clone());

            assert_eq!(row.provider_id, manifest.id);
            assert_eq!(row.provider_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(row.schema_version, manifest.primary_schema_label());
            assert_eq!(row.output_digest.kind, DigestKind::ProviderOutput);
            assert!(matches!(
                row.precision,
                PrecisionTier::Exact | PrecisionTier::Syntax | PrecisionTier::SetupAware
            ));
            assert_eq!(row.validation, ProviderValidationStatus::NativeTrusted);
            assert_eq!(row.dependency_inputs.len(), manifest.inputs.len());
            assert_eq!(row.cache_stats, stats);
        }
    }

    #[test]
    fn provider_output_digest_is_deterministic_for_identical_inputs() {
        let manifest = &crate::analysis_kernel::AnalysisKernel::provider_manifests()[1];
        let a = stable_fact_row(manifest, "fact:a", "payload:a");
        let b = stable_fact_row(manifest, "fact:b", "payload:b");
        let first = provider_output_digest_from_manifest(manifest, &[b.clone(), a.clone()]);
        let second = provider_output_digest_from_manifest(manifest, &[a, b]);

        assert_eq!(first, second);
    }

    #[test]
    fn provider_output_digest_changes_for_every_semantic_fact_field() {
        let manifest = &crate::analysis_kernel::AnalysisKernel::provider_manifests()[1];
        let base = stable_fact_row(manifest, "fact:base", "payload:base");
        let base_digest =
            provider_output_digest_from_manifest(manifest, std::slice::from_ref(&base));
        let mut mutations = Vec::new();

        let mut row = base.clone();
        row.family = FactFamily::Function;
        mutations.push(row);
        let mut row = base.clone();
        row.stable_key = "fact:changed".to_string();
        mutations.push(row);
        let mut row = base.clone();
        row.producer_id = "polint.changed.producer".to_string();
        mutations.push(row);
        let mut row = base.clone();
        row.layer_id = "polint.changed.layer".to_string();
        mutations.push(row);
        let mut row = base.clone();
        row.precision = FactPrecision::Heuristic;
        mutations.push(row);
        let mut row = base.clone();
        row.confidence = FactConfidence::Low;
        mutations.push(row);
        let mut row = base.clone();
        row.validation = ValidationStatus::SchemaValidated;
        mutations.push(row);
        let mut row = base;
        row.payload_digest = "payload:changed".to_string();
        mutations.push(row);

        for mutation in mutations {
            assert_ne!(
                provider_output_digest_from_manifest(manifest, &[mutation]),
                base_digest
            );
        }
    }

    #[test]
    fn provider_output_digest_consumes_language_scope_and_cache_policy() {
        const SCHEMAS: &[SchemaVersion] = &[SchemaVersion {
            name: "example-facts",
            version: 1,
        }];

        let base = ProviderManifest {
            id: "polint.example",
            kind: ProviderKind::LanguageSyntax,
            inputs: &["source_files"],
            outputs: &["example_facts"],
            language_scope: LanguageScope::Go,
            cache_policy: CachePolicy::NoCache,
            schema_versions: SCHEMAS,
            precision_ceiling: PrecisionCeiling::Syntax,
        };
        let scope_changed = ProviderManifest {
            language_scope: LanguageScope::TypeScriptJavaScript,
            ..base
        };
        let policy_changed = ProviderManifest {
            cache_policy: CachePolicy::ExistingFileFactCache {
                schema: "example-facts",
            },
            ..base
        };

        let row = stable_fact_row(&base, "fact:one", "payload:one");
        let base_digest = provider_output_digest_from_manifest(&base, std::slice::from_ref(&row));

        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&scope_changed, std::slice::from_ref(&row))
        );
        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&policy_changed, &[row])
        );
    }

    #[test]
    fn provider_output_family_digest_source_excludes_cache_telemetry() {
        let source = include_str!("run_report.rs");
        let digest_projection = source
            .split_once("pub(crate) fn provider_output_digest_from_manifest")
            .expect("provider output digest projection exists")
            .1
            .split_once("fn dependency_inputs_from_manifest")
            .expect("provider output digest projection has a bounded source section")
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
                !digest_projection.contains(forbidden),
                "provider output digest must exclude `{forbidden}`"
            );
        }
    }
}
