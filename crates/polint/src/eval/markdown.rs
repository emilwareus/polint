use crate::eval::competitors::ResultSource;
use crate::eval::model::EvaluationMode;
use crate::eval::report::{EvaluationRun, normalize_run};

pub(crate) fn render_markdown(run: &EvaluationRun) -> String {
    let run = normalize_run(run);
    let mut out = String::new();

    out.push_str("# polint eval report\n\n");
    out.push_str(&format!("Suite: `{}`\n\n", escape_cell(&run.suite_id)));
    out.push_str(&format!("Mode: `{}`\n\n", mode_label(run.mode)));

    if let Some(manifest) = &run.suite_manifest {
        out.push_str("## Suite\n\n");
        out.push_str("| Field | Value |\n|---|---|\n");
        out.push_str(&format!("| Support | `{}` |\n", support_label(manifest)));
        out.push_str(&format!(
            "| Adapter | `{}` |\n",
            escape_cell(&manifest.adapter_id)
        ));
        out.push_str(&format!(
            "| License | `{}` |\n\n",
            escape_cell(&manifest.license)
        ));
    }

    out.push_str("## Comparison\n\n");
    out.push_str("| Product | Mode | Source | Precision | Recall | F0.5 | F1 | Limitations |\n|---|---|---|---:|---:|---:|---:|---|\n");
    for row in &run.comparison_rows {
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {} |\n",
            escape_cell(&row.product.name),
            mode_label(row.mode),
            result_source_label(&row.result_source),
            metric_cell(row.metrics.get("precision").copied()),
            metric_cell(row.metrics.get("recall").copied()),
            metric_cell(row.metrics.get("f0_5").copied()),
            metric_cell(row.metrics.get("f1").copied()),
            escape_cell(&row.limitations.join("; "))
        ));
    }
    if run.comparison_rows.is_empty() {
        out.push_str("| _none_ |  |  |  |  |  |  |  |\n");
    }
    out.push('\n');

    out.push_str("## Scanner Metrics\n\n");
    out.push_str("| TP | FP | FN | TN | Precision | Recall | F0.5 | F1 | FPR |\n|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
        run.metrics.true_positives,
        run.metrics.false_positives,
        run.metrics.false_negatives,
        run.metrics.true_negatives,
        metric_cell(run.metrics.precision),
        metric_cell(run.metrics.recall),
        metric_cell(run.metrics.sections.scanner.f0_5),
        metric_cell(run.metrics.f1),
        metric_cell(run.metrics.false_positive_rate),
    ));

    out.push_str("## Graph Fact Path Metrics\n\n");
    out.push_str("| Graph Expected | Graph Observed | Graph Unconfirmed | Paths Expected | Paths Observed | Paths Unconfirmed | Unknowns |\n");
    out.push_str("|---:|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} |\n\n",
        run.metrics.sections.graph.edges_expected,
        run.metrics.sections.graph.edges_observed,
        run.metrics.sections.graph.edges_unconfirmed,
        run.metrics.sections.paths.paths_expected,
        run.metrics.sections.paths.paths_observed,
        run.metrics.sections.paths.paths_unconfirmed,
        run.metrics.sections.unknowns.total,
    ));

    out.push_str("## Provider Cache Stats\n\n");
    out.push_str("| Provider | Hits | Misses | Recomputes | Writes | Quarantines |\n|---|---:|---:|---:|---:|---:|\n");
    if let Some(performance) = &run.performance {
        for provider in &performance.providers {
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                escape_cell(&provider.provider_id),
                provider.cache.hits,
                provider.cache.misses,
                provider.cache.recomputes,
                provider.cache.writes,
                provider.cache.quarantines,
            ));
        }
    }
    out.push('\n');

    out.push_str("## RSS Thresholds\n\n");
    out.push_str(
        "| Cold Threshold MiB | Cold Observed MiB | Warm Threshold MiB | Warm Observed MiB | Peak Observed MiB |\n",
    );
    out.push_str("|---:|---:|---:|---:|---:|\n");
    if let Some(performance) = &run.performance {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n\n",
            optional_u64_cell(performance.rss.cold_rss_threshold_mb),
            optional_u64_cell(performance.rss.cold_rss_observed_mb),
            optional_u64_cell(performance.rss.warm_rss_threshold_mb),
            optional_u64_cell(performance.rss.warm_rss_observed_mb),
            optional_u64_cell(performance.rss.peak_rss_observed_mb),
        ));
    } else {
        out.push_str("| - | - | - | - | - |\n\n");
    }

    out.push_str("## Adaptation\n\n");
    if let Some(adaptation) = &run.adaptation {
        out.push_str(&format!(
            "- Prompt: `{}`\n- Prompt hash: `{}`\n- Sandbox root: `{}`\n- Changed artifacts: {}\n- Changed model digests: {}\n\n",
            escape_cell(&adaptation.agent.prompt_path),
            escape_cell(&adaptation.agent.prompt_hash),
            escape_cell(adaptation.sandbox_root.as_deref().unwrap_or("-")),
            adaptation.outputs.rules_or_extensions_changed.len()
            ,
            adaptation.outputs.model_digests.len()
        ));
    } else {
        out.push_str("_none_\n\n");
    }
    if let Some(delta) = &run.adaptation_delta {
        out.push_str("| Accepted Models | Rejected Models | Resolved Unknowns | New FPs | Runtime Ratio | Cache Scope |\n");
        out.push_str("|---:|---:|---:|---:|---:|---|\n");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | `{}` |\n\n",
            delta.accepted_model_facts,
            delta.rejected_model_facts,
            delta.resolved_unknowns,
            delta.new_false_positives,
            metric_cell(delta.runtime_overhead_ratio),
            escape_cell(delta.cache_invalidation_scope.as_deref().unwrap_or("-")),
        ));
        if let Some(held_out) = &delta.held_out {
            out.push_str(&format!(
                "- Held-out: selection cases {}, held-out cases {}, unknown delta {}, precision delta {}, recall delta {}, runtime ratio {}, cache `{}`\n\n",
                held_out.selection_cases,
                held_out.held_out_cases,
                held_out.held_out_unknown_delta,
                metric_cell(held_out.held_out_precision_delta),
                metric_cell(held_out.held_out_recall_delta),
                metric_cell(held_out.held_out_runtime_overhead_ratio),
                escape_cell(held_out.held_out_cache_invalidation_scope.as_deref().unwrap_or("-")),
            ));
            if !held_out.missing_selection_case_ids.is_empty()
                || !held_out.missing_held_out_case_ids.is_empty()
            {
                out.push_str(&format!(
                    "- Held-out partition missing cases: selection `{}`, held-out `{}`\n\n",
                    escape_cell(&held_out.missing_selection_case_ids.join(", ")),
                    escape_cell(&held_out.missing_held_out_case_ids.join(", ")),
                ));
            }
            if !held_out.overlapping_case_ids.is_empty() {
                out.push_str(&format!(
                    "- Held-out partition overlaps: `{}`\n\n",
                    escape_cell(&held_out.overlapping_case_ids.join(", ")),
                ));
            }
        }
    }

    out.push_str("## Per-Language Deltas\n\n");
    out.push_str("| Language | Suite | Scoring Mode | Precision Tier | Precision Delta | Recall Delta | F0.5 Delta |\n");
    out.push_str("|---|---|---|---|---:|---:|---:|\n");
    if run.metrics.sections.per_language_deltas.is_empty() {
        out.push_str("| _none_ |  |  |  |  |  |  |\n\n");
    } else {
        for row in &run.metrics.sections.per_language_deltas {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | {} | {} | {} |\n",
                escape_cell(&row.language),
                escape_cell(&row.suite_id),
                escape_cell(&row.scoring_mode),
                escape_cell(&row.precision_tier),
                metric_cell(row.precision_delta),
                metric_cell(row.recall_delta),
                metric_cell(row.f0_5_delta),
            ));
        }
        out.push('\n');
    }

    out.push_str("## Limitations\n\n");
    for limitation in &run.limitations {
        out.push_str(&format!("- {}\n", escape_cell(limitation)));
    }
    if run.limitations.is_empty() {
        out.push_str("- none\n");
    }

    out
}

/// Render a standalone benchmark report (BENCH-01) from a measured
/// [`CurveSeries`](crate::eval::bench::curve::CurveSeries).
///
/// This is the markdown-report entry-point the benchmark sweep writes to
/// `benchmark-report.md`. It keeps the existing [`render_markdown`] signature
/// intact (evaluation reports are unchanged) and composes a report header over
/// the curve table produced by `crate::eval::bench::report::render_curve_markdown`
/// so peak RSS, cold/warm wall-clock, cache/store size, and budget-exhaustion
/// are recorded per curve point, and — when an `accuracy` baseline is supplied —
/// the "## Persisted-Graph Accuracy Baseline" section (BENCH-04) with the
/// pre-store Jelly + Go x/tools recall/precision.
#[cfg(test)]
pub(crate) fn render_benchmark_report(
    series: &crate::eval::bench::curve::CurveSeries,
    accuracy: Option<&crate::eval::bench::report::GraphAccuracyBaseline>,
) -> String {
    let mut out = String::new();
    out.push_str("# polint benchmark report\n\n");
    out.push_str(&crate::eval::bench::report::render_curve_markdown(series));
    if let Some(accuracy) = accuracy {
        out.push_str(&crate::eval::bench::report::render_graph_accuracy_markdown(
            accuracy,
        ));
    }
    out
}

fn result_source_label(source: &ResultSource) -> String {
    match source {
        ResultSource::ImportedPublished { source_name, .. } => {
            format!("imported: {}", escape_cell(source_name))
        }
        ResultSource::LocallyReproduced { artifact_path, .. } => {
            format!("local: `{}`", escape_cell(artifact_path))
        }
        ResultSource::PolintRun { report_path, .. } => {
            format!("polint: `{}`", escape_cell(report_path))
        }
        ResultSource::AdapterOnly { reason, .. } => {
            format!("adapter-only: {}", escape_cell(reason))
        }
    }
}

fn support_label(manifest: &crate::eval::suite::SuiteManifest) -> &'static str {
    match manifest.language_support {
        crate::eval::suite::SuiteLanguageSupport::Supported => "supported",
        crate::eval::suite::SuiteLanguageSupport::AdapterOnly => "adapter_only",
        crate::eval::suite::SuiteLanguageSupport::FutureLanguage => "future_language",
        crate::eval::suite::SuiteLanguageSupport::ResearchOnly => "research_only",
    }
}

fn mode_label(mode: EvaluationMode) -> &'static str {
    match mode {
        EvaluationMode::PolintBaseline => "polint_baseline",
        EvaluationMode::PolintAgentAdapted => "polint_agent_adapted",
        EvaluationMode::ImportedScanner => "imported_scanner",
        EvaluationMode::LocallyReproducedScanner => "locally_reproduced_scanner",
        EvaluationMode::AdapterOnly => "adapter_only",
    }
}

fn metric_cell(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_string(), |value| format!("{value:.4}"))
}

fn optional_u64_cell(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_string(), |value| value.to_string())
}

fn escape_cell(value: &str) -> String {
    // Neutralize both the column separator and any CR/LF so a value carrying a
    // newline cannot break the table structure or inject rows.
    value.replace('|', "\\|").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::competitors::{BenchmarkComparisonRow, ProductIdentity, ResultSource};
    use crate::eval::model::EvaluationMode;
    use crate::eval::performance::{
        CacheStatsSummary, EvalPerformanceReport, ProviderStatsRow, RssStatsSummary,
    };
    use crate::eval::report::{MetricSections, MetricSummary, PerLanguageDeltaRow};
    use crate::eval::suite::{
        CaseSelector, ExpectedSource, ExpectedSourceFormat, SuiteCheckout, SuiteCheckoutStrategy,
        SuiteId, SuiteKind, SuiteLanguageSupport, SuiteManifest, SuiteScoring, SuiteTier,
    };

    #[test]
    fn markdown_renderer_is_deterministic_for_shuffled_rows() {
        let mut left = report();
        let mut right = report();
        right.comparison_rows.reverse();
        right.performance.as_mut().unwrap().providers.reverse();

        assert_eq!(render_markdown(&left), render_markdown(&right));
        left.limitations.reverse();
        assert_eq!(render_markdown(&left), render_markdown(&right));
    }

    #[test]
    fn markdown_labels_adapter_only_and_imported_competitors() {
        let markdown = render_markdown(&report());

        assert!(markdown.contains("F0.5"));
        assert!(markdown.contains("## Per-Language Deltas"));
        assert!(markdown.contains("| `javascript` | `secbench-js-smoke` | `whole-repo` | `syntax` | 0.1000 | 0.2000 | 0.1500 |"));
        assert!(markdown.contains("adapter_only"));
        assert!(markdown.contains("imported: published table"));
        assert!(markdown.contains("adapter-only: adapter dry run"));
        assert!(markdown.contains("known limitation"));
        assert!(markdown.contains("Cold Threshold MiB"));
        assert!(markdown.contains("Peak Observed MiB"));
        assert!(markdown.contains("| 512 | 256 | 384 | 128 | - |"));
    }

    #[test]
    fn markdown_omits_transient_paths_and_timestamps() {
        let markdown = render_markdown(&report());

        assert!(!markdown.contains("/var/folders"));
        assert!(!markdown.contains("/tmp/polint"));
        assert!(!markdown.contains("2026-05-26T07:00:00Z"));
    }

    #[test]
    fn markdown_populates_peak_rss_from_peak_rss_bytes() {
        let mut report = report();
        let performance = report.performance.as_mut().unwrap();
        performance.rss = RssStatsSummary::default();
        performance.runtime.peak_rss_bytes = Some(2 * 1024 * 1024);

        let markdown = render_markdown(&report);

        assert!(markdown.contains("| - | - | - | - | 2 |"));
    }

    #[test]
    fn benchmark_report_composes_header_over_curve_table() {
        use crate::eval::bench::curve::{
            BudgetExhaustionCounters, CurvePoint, CurveSeries, StoreSizeBytes,
        };

        let mut series = CurveSeries::new();
        series.points.push(CurvePoint {
            repo_id: "alpha".to_string(),
            repo_file_count: 10,
            repo_source_bytes: 20_480,
            diff_files: 2,
            diff_hunk_lines: 24,
            cold_wall_clock_ms: 100,
            warm_wall_clock_ms: 40,
            peak_rss_bytes: 3 * 1024 * 1024,
            size: StoreSizeBytes {
                cache_bytes: 4096,
                store_bytes: 0,
            },
            budget: BudgetExhaustionCounters::default(),
        });

        let markdown = render_benchmark_report(&series, None);
        assert!(markdown.starts_with("# polint benchmark report"));
        assert!(markdown.contains("## Benchmark Curves"));
        assert!(markdown.contains("Peak RSS"));
        assert!(markdown.contains("`alpha`"));
        // Without an accuracy baseline the persisted-graph section is omitted.
        assert!(!markdown.contains("Persisted-Graph Accuracy"));

        // With an accuracy baseline the report gains the accuracy section.
        let accuracy = crate::eval::bench::report::GraphAccuracyBaseline {
            schema_version: crate::eval::bench::report::GRAPH_ACCURACY_BASELINE_SCHEMA_VERSION
                .to_string(),
            reference: crate::eval::bench::report::GraphAccuracyBaseline::PRE_STORE_REFERENCE
                .to_string(),
            rows: vec![crate::eval::bench::report::GraphAccuracyRow {
                suite_id: "jelly-callgraph-micro".to_string(),
                suite_commit: Some("b799ed4".to_string()),
                recall: Some(0.42),
                precision: Some(0.90),
                graph_edges_expected: 100,
                graph_edges_observed: 50,
                unknown_count: 7,
            }],
        };
        let with_accuracy = render_benchmark_report(&series, Some(&accuracy));
        assert!(with_accuracy.contains("## Persisted-Graph Accuracy Baseline"));
        assert!(with_accuracy.contains("Recall"));
        assert!(with_accuracy.contains("Precision"));
    }

    fn report() -> EvaluationRun {
        EvaluationRun {
            schema_version: crate::eval::report::EVALUATION_SCHEMA_VERSION.to_string(),
            suite_id: "secbench-js-smoke".to_string(),
            mode: EvaluationMode::AdapterOnly,
            suite_manifest: Some(suite_manifest()),
            cases: Vec::new(),
            metrics: {
                let mut sections = MetricSections::default();
                sections.scanner.f0_5 = Some(0.3333);
                sections.per_language_deltas = vec![PerLanguageDeltaRow {
                    language: "javascript".to_string(),
                    suite_id: "secbench-js-smoke".to_string(),
                    scoring_mode: "whole-repo".to_string(),
                    precision_tier: "syntax".to_string(),
                    baseline_precision: Some(0.4),
                    current_precision: Some(0.5),
                    precision_delta: Some(0.1),
                    baseline_recall: Some(0.05),
                    current_recall: Some(0.25),
                    recall_delta: Some(0.2),
                    baseline_f0_5: Some(0.18),
                    current_f0_5: Some(0.33),
                    f0_5_delta: Some(0.15),
                }];
                MetricSummary {
                    true_positives: 1,
                    false_positives: 2,
                    false_negatives: 3,
                    true_negatives: 4,
                    unconfirmed: 0,
                    false_positive_trap_hits: 0,
                    forbidden_hits: 0,
                    unknown_count: 5,
                    facts_present: 0,
                    facts_accepted: 0,
                    facts_rejected: 0,
                    graph_edges_expected: 6,
                    graph_edges_observed: 7,
                    graph_edges_unconfirmed: 8,
                    paths_expected: 9,
                    paths_observed: 10,
                    paths_unconfirmed: 11,
                    runtime_budget_passed: 1,
                    runtime_budget_failed: 0,
                    precision: Some(0.5),
                    recall: Some(0.25),
                    f1: Some(0.3333),
                    f2: None,
                    f3: None,
                    false_positive_rate: Some(0.2),
                    sections,
                }
            },
            performance: Some(EvalPerformanceReport {
                providers: vec![provider("polint.ts.syntax"), provider("polint.source")],
                cache: CacheStatsSummary::default(),
                demand_queries: Vec::new(),
                runtime: Default::default(),
                rss: RssStatsSummary {
                    cold_rss_threshold_mb: Some(512),
                    cold_rss_observed_mb: Some(256),
                    warm_rss_threshold_mb: Some(384),
                    warm_rss_observed_mb: Some(128),
                    peak_rss_observed_mb: None,
                },
            }),
            comparison_rows: vec![
                comparison("Semgrep", EvaluationMode::ImportedScanner),
                comparison("polint", EvaluationMode::AdapterOnly),
            ],
            adaptation: None,
            adaptation_delta: None,
            limitations: vec!["known limitation".to_string()],
            output_hash: String::new(),
        }
    }

    fn suite_manifest() -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("secbench-js-smoke".to_string()),
            name: "SecBench.js smoke".to_string(),
            kind: SuiteKind::ScannerVulnerability,
            languages: vec!["javascript".to_string()],
            adapter_id: "secbench_js_tests".to_string(),
            scoring_mode: crate::eval::suite::ScoringMode::WholeRepo,
            source_url: None,
            source_commit: None,
            license: "license-review-needed".to_string(),
            language_support: SuiteLanguageSupport::AdapterOnly,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: "research/evaluation-harness/repos/SecBench.js".to_string(),
                ignored_by_git: true,
                local_clone_policy: crate::eval::suite::LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::SuiteNative,
                path: "test/exploitable-paths.json".to_string(),
            },
            scoring: SuiteScoring {
                native: vec!["secbench_js.test_file_count".to_string()],
                unified: vec!["precision".to_string()],
            },
            tiers: [(
                SuiteTier::Fast,
                CaseSelector {
                    enabled: true,
                    selector: "sample:balanced:10".to_string(),
                    max_cases: Some(10),
                    deterministic_seed: Some("seed".to_string()),
                },
            )]
            .into_iter()
            .collect(),
        }
    }

    fn provider(provider_id: &str) -> ProviderStatsRow {
        ProviderStatsRow {
            provider_id: provider_id.to_string(),
            provider_version: "1".to_string(),
            schema_version: "schema".to_string(),
            output_digest: "digest".to_string(),
            precision: "syntax".to_string(),
            validation: "native_trusted".to_string(),
            dependency_input_count: 0,
            facts_emitted: None,
            diagnostics_emitted: None,
            validation_rejections: None,
            cache: CacheStatsSummary {
                hits: 1,
                misses: 2,
                recomputes: 3,
                writes: 4,
                bypasses_disabled: 0,
                invalid_evicted_reads: 0,
                verified_reuse: 0,
                quarantines: 0,
            },
            observed_runtime_ms: Some(999),
        }
    }

    fn comparison(product: &str, mode: EvaluationMode) -> BenchmarkComparisonRow {
        BenchmarkComparisonRow {
            suite_id: SuiteId("secbench-js-smoke".to_string()),
            suite_commit: None,
            mode,
            product: ProductIdentity {
                name: product.to_string(),
                version: Some("1".to_string()),
                vendor: None,
            },
            result_source: if mode == EvaluationMode::ImportedScanner {
                ResultSource::ImportedPublished {
                    source_name: "published table".to_string(),
                    source_url: "https://example.test".to_string(),
                    retrieved_at: "2026-05-26".to_string(),
                }
            } else {
                ResultSource::AdapterOnly {
                    manifest_path: "research/evaluation-harness/suites/secbench-js-smoke.toml"
                        .to_string(),
                    reason: "adapter dry run".to_string(),
                }
            },
            metrics: [
                ("precision".to_string(), 0.5),
                ("recall".to_string(), 0.25),
                ("f0_5".to_string(), 0.3333),
                ("f1".to_string(), 0.3333),
            ]
            .into_iter()
            .collect(),
            limitations: vec!["known limitation".to_string()],
        }
    }
}
