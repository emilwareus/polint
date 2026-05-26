use serde::{Deserialize, Serialize};

use crate::analysis_kernel::incremental::{CacheStats, KernelRunReport, ProviderOutputMeta};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct EvalPerformanceReport {
    pub(crate) providers: Vec<ProviderStatsRow>,
    pub(crate) cache: CacheStatsSummary,
    pub(crate) demand_queries: Vec<DemandQueryStatsRow>,
    pub(crate) runtime: RuntimeStatsSummary,
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
                query_kind: entry.query_kind.clone(),
                query_version: entry.query_version.clone(),
                parameter_digest: entry.parameter_digest.clone(),
                cache_status: entry.cache_status.clone(),
                result_digest: entry.result_digest.clone(),
                precision_tier: entry.precision_tier.clone(),
                compute_duration_micros: Some(entry.compute_duration_micros),
            })
            .collect::<Vec<_>>();
        demand_queries.sort();

        Self {
            providers,
            cache: CacheStatsSummary::from_cache_stats(&report.cache_stats),
            demand_queries,
            runtime: RuntimeStatsSummary::default(),
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
        Self {
            provider_id: output.provider_id.clone(),
            provider_version: output.provider_version.clone(),
            schema_version: output.schema_version.clone(),
            output_digest: output.output_digest.value.clone(),
            precision: format!("{:?}", output.precision).to_ascii_lowercase(),
            validation: output.validation.clone(),
            dependency_input_count: output.dependency_inputs.len(),
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

pub(crate) fn strip_volatile_runtime(report: &mut EvalPerformanceReport) {
    report.runtime.observed_runtime_ms = None;
    report.runtime.peak_rss_bytes = None;
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
        CacheStats, DemandQueryTrace, DemandQueryTraceEntry, Digest, DigestKind,
        GoLifecycleSnapshot, InputComponent, InputComponentStatus, InputSnapshot, KernelRunReport,
        ProviderOutputMeta, TsJsLifecycleSnapshot, provider_output_digest_from_manifest,
        provider_output_from_manifest,
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
        first.providers[0].observed_runtime_ms = Some(20);
        first.demand_queries[0].compute_duration_micros = Some(30);
        second.runtime.observed_runtime_ms = Some(999);
        second.providers[0].observed_runtime_ms = Some(888);
        second.demand_queries[0].compute_duration_micros = Some(777);

        strip_volatile_runtime(&mut first);
        strip_volatile_runtime(&mut second);

        assert_eq!(first, second);
    }

    fn kernel_report_with_stats(stats: CacheStats) -> KernelRunReport {
        let manifests = AnalysisKernel::provider_manifests();
        let provider_outputs = manifests
            .iter()
            .take(2)
            .map(|manifest| {
                let output_digest = provider_output_digest_from_manifest(
                    manifest,
                    &[format!("provider={}", manifest.id)],
                );
                provider_output_from_manifest(manifest, output_digest, stats.clone())
            })
            .collect::<Vec<ProviderOutputMeta>>();

        let mut trace = DemandQueryTrace::default();
        trace.record_entry(DemandQueryTraceEntry {
            query_kind: "call_graph".to_string(),
            query_version: "1".to_string(),
            parameter_digest: Digest::from_parts(DigestKind::QueryParameters, "query", &["calls"])
                .value,
            input_layer_digests: Vec::new(),
            cache_status: "computed".to_string(),
            compute_duration_micros: 123,
            result_digest: Digest::from_parts(DigestKind::ProviderOutput, "result", &["calls"])
                .value,
            precision_tier: "setup_aware".to_string(),
        });

        KernelRunReport::new(
            InputSnapshot {
                schema_version: "test".to_string(),
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
            },
            provider_outputs,
            trace,
        )
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
