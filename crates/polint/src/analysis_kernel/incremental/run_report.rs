use super::demand::DemandQueryTrace;
use super::{CacheStats, Digest, DigestKind, InputSnapshot, PrecisionTier, ProviderOutputMeta};
#[cfg(test)]
use crate::analysis::summaries::provider::SccClosureDebugSnapshot;
use crate::analysis_kernel::{PrecisionCeiling, ProviderManifest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelRunReport {
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) provider_outputs: Vec<ProviderOutputMeta>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) demand_query_trace: DemandQueryTrace,
    #[cfg(test)]
    pub(crate) scc_closure_debug: Option<SccClosureDebugSnapshot>,
}

impl KernelRunReport {
    pub(crate) fn new(
        input_snapshot: InputSnapshot,
        provider_outputs: Vec<ProviderOutputMeta>,
        demand_query_trace: DemandQueryTrace,
    ) -> Self {
        let mut cache_stats = aggregate_cache_stats(&provider_outputs);
        aggregate_demand_query_stats(&demand_query_trace, &mut cache_stats);

        Self {
            input_snapshot,
            provider_outputs,
            cache_stats,
            demand_query_trace,
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
        precision_tier(manifest.precision_ceiling),
        "native_trusted",
        dependency_inputs_from_manifest(manifest),
        cache_stats,
    )
}

pub(crate) fn provider_output_digest_from_manifest(
    manifest: &ProviderManifest,
    summary_parts: &[String],
) -> Digest {
    let schema_label = manifest.primary_schema_label();
    let language_scope = manifest.language_scope_label();
    let cache_policy = manifest.cache_policy_label();
    let precision = precision_label(manifest.precision_ceiling);

    let mut output_families = manifest
        .outputs
        .iter()
        .map(|output| format!("output_family={output}"))
        .collect::<Vec<_>>();
    output_families.sort();

    let mut metadata_fact_summary_parts = summary_parts.to_vec();
    metadata_fact_summary_parts.sort();

    let mut digest_parts = vec![
        format!("provider_id={}", manifest.id),
        format!("schema_version={schema_label}"),
        format!("language_scope={language_scope}"),
        format!("cache_policy={cache_policy}"),
        format!("precision={precision}"),
    ];
    digest_parts.extend(output_families);
    digest_parts.extend(metadata_fact_summary_parts);

    let digest_refs = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "provider_output", &digest_refs)
}

fn dependency_inputs_from_manifest(manifest: &ProviderManifest) -> Vec<Digest> {
    let mut inputs = manifest.inputs.to_vec();
    inputs.sort();
    inputs
        .into_iter()
        .map(|input| Digest::from_parts(DigestKind::DependencyLayer, "dependency_input", &[input]))
        .collect()
}

fn precision_tier(precision: PrecisionCeiling) -> PrecisionTier {
    match precision {
        PrecisionCeiling::Exact => PrecisionTier::Exact,
        PrecisionCeiling::Syntax => PrecisionTier::Syntax,
        PrecisionCeiling::SetupAware => PrecisionTier::SetupAware,
    }
}

fn precision_label(precision: PrecisionCeiling) -> &'static str {
    match precision {
        PrecisionCeiling::Exact => "exact",
        PrecisionCeiling::Syntax => "syntax",
        PrecisionCeiling::SetupAware => "setup_aware",
    }
}

/// Aggregates demand query cache statistics into an existing `CacheStats`.
///
/// Entries with `cache_status == "hit"` count as hits; entries with
/// `cache_status == "miss"` or `"computed"` count as recomputes.
fn aggregate_demand_query_stats(trace: &DemandQueryTrace, stats: &mut CacheStats) {
    for entry in trace.entries() {
        match entry.cache_status.as_str() {
            "hit" => stats.hits += 1,
            "miss" | "computed" => stats.recomputes += 1,
            _ => {}
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
        CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest, SchemaVersion,
    };

    #[test]
    fn provider_outputs_are_constructed_in_manifest_order() {
        let provider_outputs = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| {
                let output_digest = provider_output_digest_from_manifest(
                    manifest,
                    &[format!("provider={}", manifest.id)],
                );
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
            let output_digest = provider_output_digest_from_manifest(
                manifest,
                &[format!("provider={}", manifest.id)],
            );
            let row = provider_output_from_manifest(manifest, output_digest, stats.clone());

            assert_eq!(row.provider_id, manifest.id);
            assert_eq!(row.provider_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(row.schema_version, manifest.primary_schema_label());
            assert_eq!(row.output_digest.kind, DigestKind::ProviderOutput);
            assert!(matches!(
                row.precision,
                PrecisionTier::Exact | PrecisionTier::Syntax | PrecisionTier::SetupAware
            ));
            assert_eq!(row.validation, "native_trusted");
            assert_eq!(row.dependency_inputs.len(), manifest.inputs.len());
            assert_eq!(row.cache_stats, stats);
        }
    }

    #[test]
    fn provider_output_digest_is_deterministic_for_identical_inputs() {
        let manifest = &crate::analysis_kernel::AnalysisKernel::provider_manifests()[1];
        let first = provider_output_digest_from_manifest(
            manifest,
            &["fact=b".to_string(), "fact=a".to_string()],
        );
        let second = provider_output_digest_from_manifest(
            manifest,
            &["fact=a".to_string(), "fact=b".to_string()],
        );

        assert_eq!(first, second);
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

        let base_digest = provider_output_digest_from_manifest(&base, &["facts=1".to_string()]);

        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&scope_changed, &["facts=1".to_string()])
        );
        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&policy_changed, &["facts=1".to_string()])
        );
    }
}
