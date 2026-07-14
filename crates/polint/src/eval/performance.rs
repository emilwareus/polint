use serde::{Deserialize, Serialize};

use crate::analysis_kernel::incremental::{CacheStats, KernelRunReport, ProviderOutputMeta};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct EvalPerformanceReport {
    pub(crate) providers: Vec<ProviderStatsRow>,
    pub(crate) cache: CacheStatsSummary,
    pub(crate) demand_queries: Vec<DemandQueryStatsRow>,
    pub(crate) runtime: RuntimeStatsSummary,
    #[serde(default)]
    pub(crate) rss: RssStatsSummary,
}

impl EvalPerformanceReport {
    pub(crate) fn from_kernel_report(report: &KernelRunReport) -> Self {
        let mut providers = report
            .provider_outputs
            .iter()
            .map(ProviderStatsRow::from_provider_output)
            .collect::<Vec<_>>();
        providers.sort();

        let mut demand_queries = report
            .demand_query_trace
            .entries()
            .iter()
            .map(|entry| DemandQueryStatsRow {
                query_kind: entry.query_key.query_kind.clone(),
                query_version: entry.query_key.query_version.clone(),
                parameter_digest: entry.query_key.parameter_digest.value.clone(),
                cache_status: entry.cache_status.label().to_string(),
                result_digest: entry.result_digest.value.clone(),
                precision_tier: format!("{:?}", entry.precision_tier),
                compute_duration_micros: Some(entry.compute_duration_micros),
            })
            .collect::<Vec<_>>();
        demand_queries.sort();

        Self {
            providers,
            cache: CacheStatsSummary::from_cache_stats(&report.cache_stats),
            demand_queries,
            runtime: RuntimeStatsSummary::default(),
            rss: RssStatsSummary::default(),
        }
    }

    pub(crate) fn cache_by_provider(&self) -> Vec<ProviderCacheStatsRow> {
        self.providers
            .iter()
            .map(|provider| ProviderCacheStatsRow {
                provider_id: provider.provider_id.clone(),
                cache: provider.cache.clone(),
            })
            .collect()
    }

    pub(crate) fn sync_peak_rss_from_runtime(&mut self) {
        if self.rss.peak_rss_observed_mb.is_none()
            && let Some(peak_rss_bytes) = self.runtime.peak_rss_bytes
        {
            self.rss.peak_rss_observed_mb = Some(bytes_to_mib(peak_rss_bytes));
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ProviderStatsRow {
    pub(crate) provider_id: String,
    pub(crate) provider_version: String,
    pub(crate) schema_version: String,
    pub(crate) output_digest: String,
    pub(crate) precision: String,
    pub(crate) validation: String,
    pub(crate) dependency_input_count: usize,
    pub(crate) facts_emitted: Option<u64>,
    pub(crate) diagnostics_emitted: Option<u64>,
    pub(crate) validation_rejections: Option<u64>,
    pub(crate) cache: CacheStatsSummary,
    pub(crate) observed_runtime_ms: Option<u64>,
}

impl ProviderStatsRow {
    fn from_provider_output(output: &ProviderOutputMeta) -> Self {
        let semantic = output.semantic_projection();
        Self {
            provider_id: semantic.provider_id.to_string(),
            provider_version: semantic.provider_version.to_string(),
            schema_version: semantic.schema_version.to_string(),
            output_digest: semantic.output_digest.value.clone(),
            precision: semantic.precision.label().to_string(),
            validation: semantic.validation.label().to_string(),
            dependency_input_count: semantic.dependency_inputs.len(),
            facts_emitted: None,
            diagnostics_emitted: None,
            validation_rejections: None,
            cache: CacheStatsSummary::from_cache_stats(&output.cache_stats),
            observed_runtime_ms: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CacheStatsSummary {
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) recomputes: u64,
    pub(crate) writes: u64,
    pub(crate) bypasses_disabled: u64,
    pub(crate) invalid_evicted_reads: u64,
    pub(crate) verified_reuse: u64,
    pub(crate) quarantines: u64,
}

impl CacheStatsSummary {
    pub(crate) fn from_cache_stats(stats: &CacheStats) -> Self {
        Self {
            hits: stats.hits,
            misses: stats.misses,
            recomputes: stats.recomputes,
            writes: stats.writes,
            bypasses_disabled: stats.bypasses_disabled,
            invalid_evicted_reads: stats.invalid_evicted_reads,
            verified_reuse: stats.verified_reuse,
            quarantines: stats.quarantines,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ProviderCacheStatsRow {
    pub(crate) provider_id: String,
    pub(crate) cache: CacheStatsSummary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct DemandQueryStatsRow {
    pub(crate) query_kind: String,
    pub(crate) query_version: String,
    pub(crate) parameter_digest: String,
    pub(crate) cache_status: String,
    pub(crate) result_digest: String,
    pub(crate) precision_tier: String,
    pub(crate) compute_duration_micros: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct RuntimeStatsSummary {
    pub(crate) observed_runtime_ms: Option<u64>,
    pub(crate) peak_rss_bytes: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct RssStatsSummary {
    #[serde(default)]
    pub(crate) cold_rss_threshold_mb: Option<u64>,
    #[serde(default)]
    pub(crate) cold_rss_observed_mb: Option<u64>,
    #[serde(default)]
    pub(crate) warm_rss_threshold_mb: Option<u64>,
    #[serde(default)]
    pub(crate) warm_rss_observed_mb: Option<u64>,
    #[serde(default)]
    pub(crate) peak_rss_observed_mb: Option<u64>,
}

fn bytes_to_mib(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

pub(crate) fn strip_volatile_runtime(report: &mut EvalPerformanceReport) {
    report.runtime.observed_runtime_ms = None;
    report.runtime.peak_rss_bytes = None;
    report.rss.cold_rss_observed_mb = None;
    report.rss.warm_rss_observed_mb = None;
    report.rss.peak_rss_observed_mb = None;
    for provider in &mut report.providers {
        provider.observed_runtime_ms = None;
    }
    for query in &mut report.demand_queries {
        query.compute_duration_micros = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{
        CacheStats, DemandCacheStatus, DemandQueryTrace, DemandQueryTraceEntry, Digest, DigestKind,
        GoLifecycleSnapshot, InputComponent, InputComponentStatus, InputSnapshot, KernelRunReport,
        PrecisionTier, ProviderOutputMeta, TsJsLifecycleSnapshot, dependency_free_test_query_key,
        provider_output_digest_from_manifest, provider_output_from_manifest,
    };
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, ProviderManifest, StableFactMetaRow,
        ValidationStatus,
    };

    #[test]
    fn eval_performance_projects_provider_and_cache_rows() {
        let mut stats = CacheStats::default();
        stats.record_hit();
        stats.record_miss();
        stats.record_recompute();
        stats.record_write();
        stats.record_quarantine();

        let report = kernel_report_with_stats(stats);
        let performance = EvalPerformanceReport::from_kernel_report(&report);

        assert_eq!(performance.providers.len(), 2);
        assert_eq!(performance.cache.hits, 2);
        assert_eq!(performance.cache.misses, 2);
        assert_eq!(performance.cache.recomputes, 3);
        assert_eq!(performance.cache.writes, 2);
        assert_eq!(performance.cache.quarantines, 2);
        assert_eq!(performance.cache_by_provider().len(), 2);
        assert_eq!(performance.providers[0].validation, "native_trusted");
        assert_eq!(performance.demand_queries[0].query_kind, "call_graph");
        assert_eq!(performance.demand_queries[0].query_version, "1");
        assert_eq!(performance.demand_queries[0].cache_status, "computed");
        assert_eq!(performance.demand_queries[0].precision_tier, "SetupAware");
        assert_eq!(
            performance.demand_queries[0].compute_duration_micros,
            Some(123)
        );
    }

    #[test]
    fn eval_performance_serializes_provider_rows_deterministically() {
        let report = kernel_report_with_stats(CacheStats::default());
        let mut performance = EvalPerformanceReport::from_kernel_report(&report);
        let first = serde_json::to_string(&performance).unwrap();

        performance.providers.reverse();
        performance.providers.sort();
        let second = serde_json::to_string(&performance).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn eval_performance_strips_volatile_runtime_before_hashing() {
        let report = kernel_report_with_stats(CacheStats::default());
        let mut first = EvalPerformanceReport::from_kernel_report(&report);
        let mut second = first.clone();
        first.runtime.observed_runtime_ms = Some(10);
        first.runtime.peak_rss_bytes = Some(333 * 1024 * 1024);
        first.rss.cold_rss_threshold_mb = Some(512);
        first.rss.cold_rss_observed_mb = Some(300);
        first.rss.warm_rss_threshold_mb = Some(384);
        first.rss.warm_rss_observed_mb = Some(200);
        first.rss.peak_rss_observed_mb = Some(333);
        first.providers[0].observed_runtime_ms = Some(20);
        first.demand_queries[0].compute_duration_micros = Some(30);
        second.runtime.observed_runtime_ms = Some(999);
        second.runtime.peak_rss_bytes = Some(777 * 1024 * 1024);
        second.rss.cold_rss_threshold_mb = Some(512);
        second.rss.cold_rss_observed_mb = Some(999);
        second.rss.warm_rss_threshold_mb = Some(384);
        second.rss.warm_rss_observed_mb = Some(888);
        second.rss.peak_rss_observed_mb = Some(777);
        second.providers[0].observed_runtime_ms = Some(888);
        second.demand_queries[0].compute_duration_micros = Some(777);

        strip_volatile_runtime(&mut first);
        strip_volatile_runtime(&mut second);

        assert_eq!(first, second);
        assert_eq!(first.rss.cold_rss_threshold_mb, Some(512));
        assert_eq!(first.rss.cold_rss_observed_mb, None);
        assert_eq!(first.rss.warm_rss_threshold_mb, Some(384));
        assert_eq!(first.rss.warm_rss_observed_mb, None);
        assert_eq!(first.rss.peak_rss_observed_mb, None);
        assert_eq!(first.runtime.peak_rss_bytes, None);
    }

    #[test]
    fn eval_performance_rss_summary_accepts_older_reports_without_peak_rss() {
        let report = serde_json::json!({
            "providers": [],
            "cache": {
                "hits": 0,
                "misses": 0,
                "recomputes": 0,
                "writes": 0,
                "bypasses_disabled": 0,
                "invalid_evicted_reads": 0,
                "verified_reuse": 0,
                "quarantines": 0
            },
            "demand_queries": [],
            "runtime": {
                "observed_runtime_ms": null,
                "peak_rss_bytes": null
            },
            "rss": {
                "cold_rss_threshold_mb": 512,
                "cold_rss_observed_mb": null,
                "warm_rss_threshold_mb": 384,
                "warm_rss_observed_mb": null
            }
        });

        let performance: EvalPerformanceReport =
            serde_json::from_value(report).expect("older rss summary deserializes");

        assert_eq!(performance.rss.cold_rss_threshold_mb, Some(512));
        assert_eq!(performance.rss.warm_rss_threshold_mb, Some(384));
        assert_eq!(performance.rss.peak_rss_observed_mb, None);
    }

    #[test]
    fn eval_performance_populates_peak_rss_from_peak_rss_bytes() {
        let report = kernel_report_with_stats(CacheStats::default());
        let mut performance = EvalPerformanceReport::from_kernel_report(&report);
        performance.runtime.peak_rss_bytes = Some(2 * 1024 * 1024 + 1);

        performance.sync_peak_rss_from_runtime();

        assert_eq!(performance.rss.peak_rss_observed_mb, Some(3));
        assert_eq!(performance.rss.warm_rss_observed_mb, None);

        performance.rss.peak_rss_observed_mb = Some(99);
        performance.sync_peak_rss_from_runtime();
        assert_eq!(performance.rss.peak_rss_observed_mb, Some(99));
    }

    #[test]
    fn eval_provider_digest_fixture_is_permutation_stable_and_semantic() {
        let manifest = &AnalysisKernel::provider_manifests()[0];
        let rows = stable_fact_rows(manifest);
        let mut reversed = rows.clone();
        reversed.reverse();

        let first = provider_output_digest_from_manifest(manifest, &rows);
        let second = provider_output_digest_from_manifest(manifest, &reversed);
        let mut changed = rows;
        changed[0].payload_digest = "payload:changed".to_string();
        let changed = provider_output_digest_from_manifest(manifest, &changed);

        assert_eq!(first, second);
        assert_ne!(first, changed);
    }

    fn kernel_report_with_stats(stats: CacheStats) -> KernelRunReport {
        let manifests = AnalysisKernel::provider_manifests();
        let provider_outputs = manifests
            .iter()
            .take(2)
            .map(|manifest| {
                let stable_rows = stable_fact_rows(manifest);
                let output_digest = provider_output_digest_from_manifest(manifest, &stable_rows);
                provider_output_from_manifest(manifest, output_digest, stats.clone())
            })
            .collect::<Vec<ProviderOutputMeta>>();

        let mut trace = DemandQueryTrace::default();
        trace.record_entry(DemandQueryTraceEntry {
            query_key: dependency_free_test_query_key(
                "call_graph",
                "1",
                Digest::from_parts(DigestKind::QueryParameters, "query", &["calls"]),
                Digest::from_parts(DigestKind::Budget, "budget", &["default"]),
                PrecisionTier::SetupAware,
            ),
            result_digest: Digest::from_parts(DigestKind::ProviderOutput, "result", &["calls"]),
            precision_tier: PrecisionTier::SetupAware,
            provenance: "native".to_string(),
            cache_status: DemandCacheStatus::Computed,
            compute_duration_micros: 123,
        });

        let input_snapshot = InputSnapshot {
            schema_version: crate::analysis_kernel::incremental::INPUT_SNAPSHOT_SCHEMA_VERSION
                .to_string(),
            workspace_identity: crate::analysis_kernel::incremental::WorkspaceIdentity::from_roots(
                [std::path::Path::new("test-workspace")],
            ),
            config_identity:
                crate::analysis_kernel::incremental::ConfigIdentity::from_complete_config_parts(
                    "config",
                    &["test"],
                ),
            analysis_settings: Vec::new(),
            requested_capabilities: Vec::new(),
            analysis_requirements_identity: Digest::absent(
                DigestKind::AnalysisRequirements,
                "requested_capabilities",
            ),
            files: Vec::new(),
            config: input_component("config"),
            go_lifecycle: GoLifecycleSnapshot {
                components: Vec::new(),
            },
            ts_js_lifecycle: TsJsLifecycleSnapshot {
                components: Vec::new(),
            },
            rules: Vec::new(),
            models: Vec::new(),
            extensions: Vec::new(),
            tool_invocations: Vec::new(),
            provider_schemas: Vec::new(),
        };
        assert!(input_snapshot.requested_capabilities.is_empty());
        assert_eq!(
            input_snapshot.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        KernelRunReport::new_for_test(
            input_snapshot,
            provider_outputs,
            trace,
            Vec::new(),
            crate::analysis_kernel::StoreStatus::Disabled,
        )
    }

    fn stable_fact_rows(manifest: &ProviderManifest) -> Vec<StableFactMetaRow> {
        vec![
            StableFactMetaRow {
                family: FactFamily::SourceFile,
                stable_key: format!("{}:source", manifest.id),
                producer_id: manifest.id.to_string(),
                layer_id: manifest.id.to_string(),
                precision: FactPrecision::Syntax,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:source".to_string(),
            },
            StableFactMetaRow {
                family: FactFamily::Function,
                stable_key: format!("{}:function", manifest.id),
                producer_id: manifest.id.to_string(),
                layer_id: manifest.id.to_string(),
                precision: FactPrecision::SetupAware,
                confidence: FactConfidence::Medium,
                validation: ValidationStatus::SchemaValidated,
                payload_digest: "payload:function".to_string(),
            },
        ]
    }

    fn input_component(name: &str) -> InputComponent {
        InputComponent {
            name: name.to_string(),
            status: InputComponentStatus::Present,
            digest: Digest::from_parts(DigestKind::Config, name, &["test"]),
            detail: Vec::new(),
        }
    }
}
