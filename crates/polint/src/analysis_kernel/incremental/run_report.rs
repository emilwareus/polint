use super::demand::DemandQueryTrace;
use super::{CacheStats, Digest, DigestKind, InputSnapshot, PrecisionTier, ProviderTelemetry};
#[cfg(test)]
use crate::analysis::summaries::provider::SccClosureDebugSnapshot;
use crate::analysis_kernel::{PrecisionCeiling, ProviderManifest};
use crate::analysis_kernel::{ProviderOutcome, ProviderOutputIdentity, StoreStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelRunReport {
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) provider_outcomes: Vec<ProviderOutcome>,
    pub(crate) provider_telemetry: Vec<ProviderTelemetry>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) demand_query_trace: DemandQueryTrace,
    store_status: StoreStatus,
    #[cfg(test)]
    pub(crate) scc_closure_debug: Option<SccClosureDebugSnapshot>,
}

impl KernelRunReport {
    pub(crate) fn new(
        input_snapshot: InputSnapshot,
        provider_outcomes: Vec<ProviderOutcome>,
        provider_telemetry: Vec<ProviderTelemetry>,
        demand_query_trace: DemandQueryTrace,
        store_status: StoreStatus,
    ) -> Self {
        assert_eq!(
            provider_outcomes.len(),
            provider_telemetry.len(),
            "provider outcomes and telemetry must cover the same inventory"
        );
        assert!(
            provider_outcomes
                .iter()
                .zip(&provider_telemetry)
                .all(|(outcome, telemetry)| outcome.provider_id == telemetry.provider_id),
            "provider outcomes and telemetry must use identical manifest order"
        );
        let mut cache_stats = aggregate_cache_stats(&provider_telemetry);
        aggregate_demand_query_stats(&demand_query_trace, &mut cache_stats);

        Self {
            input_snapshot,
            provider_outcomes,
            provider_telemetry,
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

pub(crate) fn provider_output_identity_from_manifest(
    manifest: &ProviderManifest,
    output_digest: Digest,
) -> ProviderOutputIdentity {
    ProviderOutputIdentity::from_manifest(
        manifest,
        output_digest,
        precision_tier(manifest.precision_ceiling),
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

fn aggregate_cache_stats(provider_telemetry: &[ProviderTelemetry]) -> CacheStats {
    let mut aggregate = CacheStats::default();
    for telemetry in provider_telemetry {
        aggregate.hits += telemetry.cache_stats.hits;
        aggregate.misses += telemetry.cache_stats.misses;
        aggregate.recomputes += telemetry.cache_stats.recomputes;
        aggregate.writes += telemetry.cache_stats.writes;
        aggregate.bypasses_disabled += telemetry.cache_stats.bypasses_disabled;
        aggregate.invalid_evicted_reads += telemetry.cache_stats.invalid_evicted_reads;
        aggregate.verified_reuse += telemetry.cache_stats.verified_reuse;
        aggregate.quarantines += telemetry.cache_stats.quarantines;
    }
    aggregate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::DigestKind;
    use crate::analysis_kernel::{
        CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest,
        ProviderOutcomeStatus, ProviderOutcomeTracker, SchemaVersion, ValidationDowngrades,
    };
    use std::collections::BTreeSet;

    #[test]
    fn provider_outputs_are_constructed_in_manifest_order() {
        let manifests = crate::analysis_kernel::AnalysisKernel::provider_manifests();
        let selected = manifests
            .iter()
            .map(|manifest| manifest.id)
            .collect::<BTreeSet<_>>();
        let mut tracker = ProviderOutcomeTracker::from_manifests(manifests, &selected).unwrap();
        for manifest in manifests {
            let output_digest = provider_output_digest_from_manifest(
                manifest,
                &[format!("provider={}", manifest.id)],
            );
            tracker
                .record_success(
                    manifest.id,
                    provider_output_identity_from_manifest(manifest, output_digest),
                )
                .unwrap();
        }
        let provider_outcomes = tracker.seal(&ValidationDowngrades::default()).unwrap();

        assert_eq!(
            provider_outcomes
                .iter()
                .map(|outcome| outcome.provider_id.as_str())
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
    fn provider_output_identity_rows_include_manifest_identity_and_digest() {
        for manifest in crate::analysis_kernel::AnalysisKernel::provider_manifests() {
            let output_digest = provider_output_digest_from_manifest(
                manifest,
                &[format!("provider={}", manifest.id)],
            );
            let identity = provider_output_identity_from_manifest(manifest, output_digest);

            assert_eq!(identity.provider_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(identity.schema_version, manifest.primary_schema_label());
            assert_eq!(identity.output_digest.kind, DigestKind::ProviderOutput);
            assert!(matches!(
                identity.precision,
                PrecisionTier::Exact | PrecisionTier::Syntax | PrecisionTier::SetupAware
            ));
        }
    }

    #[test]
    fn report_requires_outcomes_and_telemetry_in_the_same_manifest_order() {
        let manifests = crate::analysis_kernel::AnalysisKernel::provider_manifests();
        let selected = manifests
            .iter()
            .map(|manifest| manifest.id)
            .collect::<BTreeSet<_>>();
        let mut tracker = ProviderOutcomeTracker::from_manifests(manifests, &selected).unwrap();
        for manifest in manifests {
            tracker
                .record_success(
                    manifest.id,
                    provider_output_identity_from_manifest(
                        manifest,
                        provider_output_digest_from_manifest(manifest, &[]),
                    ),
                )
                .unwrap();
        }
        let outcomes = tracker.seal(&ValidationDowngrades::default()).unwrap();
        assert!(outcomes.iter().all(|outcome| {
            outcome.status == ProviderOutcomeStatus::Succeeded && outcome.output_identity.is_some()
        }));
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
