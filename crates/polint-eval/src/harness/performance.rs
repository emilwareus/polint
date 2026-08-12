use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::analysis_kernel::incremental::{CacheStats, KernelRunReport};
use crate::analysis_kernel::{ProviderOutcome, hard_dependencies};

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
        let telemetry = report
            .provider_telemetry
            .iter()
            .map(|row| (row.provider_id.as_str(), &row.cache_stats))
            .collect::<BTreeMap<_, _>>();
        let mut providers = report
            .provider_outcomes
            .iter()
            .map(|outcome| {
                ProviderStatsRow::from_provider_outcome(
                    outcome,
                    telemetry
                        .get(outcome.provider_id.as_str())
                        .copied()
                        .expect("run report requires matching provider telemetry"),
                )
            })
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
    fn from_provider_outcome(outcome: &ProviderOutcome, cache_stats: &CacheStats) -> Self {
        let identity = outcome.output_identity.as_ref();
        Self {
            provider_id: outcome.provider_id.clone(),
            provider_version: identity
                .map(|identity| identity.provider_version.clone())
                .unwrap_or_default(),
            schema_version: identity
                .map(|identity| identity.schema_version.clone())
                .unwrap_or_default(),
            output_digest: identity
                .map(|identity| identity.output_digest.value.clone())
                .unwrap_or_default(),
            precision: identity
                .map(|identity| format!("{:?}", identity.precision).to_ascii_lowercase())
                .unwrap_or_default(),
            validation: outcome.validation_display(),
            dependency_input_count: hard_dependencies(&outcome.provider_id).len(),
            facts_emitted: None,
            diagnostics_emitted: None,
            validation_rejections: None,
            cache: CacheStatsSummary::from_cache_stats(cache_stats),
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
        CacheStats, DemandQueryTrace, DemandQueryTraceEntry, Digest, DigestKind,
        GoLifecycleSnapshot, InputComponent, InputComponentStatus, InputSnapshot, KernelRunReport,
        ProviderTelemetry, TsJsLifecycleSnapshot, provider_output_digest_from_manifest,
        provider_output_identity_from_manifest,
    };
    use crate::analysis_kernel::{ProviderOutcomeTracker, ValidationDowngrades};
    use std::collections::BTreeSet;

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

        assert_eq!(
            performance.providers.len(),
            AnalysisKernel::provider_manifests().len()
        );
        assert_eq!(performance.cache.hits, 2);
        assert_eq!(performance.cache.misses, 2);
        assert_eq!(performance.cache.recomputes, 3);
        assert_eq!(performance.cache.writes, 2);
        assert_eq!(performance.cache.quarantines, 2);
        assert_eq!(
            performance.cache_by_provider().len(),
            AnalysisKernel::provider_manifests().len()
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
    fn performance_projection_keeps_failure_identity_separate_from_telemetry() {
        let manifests = AnalysisKernel::provider_manifests();
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
                        provider_output_digest_from_manifest(manifest, &[manifest.id.to_string()]),
                    ),
                )
                .unwrap();
        }
        let outcomes = tracker
            .seal(&ValidationDowngrades::for_providers([
                "polint.go.syntax".to_string()
            ]))
            .unwrap();
        let telemetry = manifests
            .iter()
            .map(|manifest| {
                let mut stats = CacheStats::default();
                if manifest.id == "polint.go.syntax" {
                    stats.record_hit();
                }
                ProviderTelemetry::new(manifest.id, stats)
            })
            .collect();
        let report = KernelRunReport::new(
            empty_input_snapshot(),
            outcomes,
            telemetry,
            DemandQueryTrace::default(),
            crate::analysis_kernel::StoreStatus::Disabled,
        );

        let performance = EvalPerformanceReport::from_kernel_report(&report);
        let failed = performance
            .providers
            .iter()
            .find(|row| row.provider_id == "polint.go.syntax")
            .unwrap();
        assert_eq!(failed.validation, "failed:validation:validation_rejected");
        assert!(failed.output_digest.is_empty());
        assert_eq!(failed.cache.hits, 1);
        let succeeded = performance
            .providers
            .iter()
            .find(|row| row.provider_id == "polint.source")
            .unwrap();
        assert_eq!(succeeded.validation, "succeeded");
        assert!(!succeeded.output_digest.is_empty());
    }

    fn kernel_report_with_stats(stats: CacheStats) -> KernelRunReport {
        let manifests = AnalysisKernel::provider_manifests();
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
        let provider_telemetry = manifests
            .iter()
            .enumerate()
            .map(|(index, manifest)| {
                ProviderTelemetry::new(
                    manifest.id,
                    if index < 2 {
                        stats.clone()
                    } else {
                        CacheStats::default()
                    },
                )
            })
            .collect();

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
            empty_input_snapshot(),
            provider_outcomes,
            provider_telemetry,
            trace,
            crate::analysis_kernel::StoreStatus::Disabled,
        )
    }

    fn empty_input_snapshot() -> InputSnapshot {
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
        }
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
