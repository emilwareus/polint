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
    out.push_str("| Product | Mode | Source | Precision | Recall | F1 | Limitations |\n|---|---|---|---:|---:|---:|---|\n");
    for row in &run.comparison_rows {
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} |\n",
            escape_cell(&row.product.name),
            mode_label(row.mode),
            result_source_label(&row.result_source),
            metric_cell(row.metrics.get("precision").copied()),
            metric_cell(row.metrics.get("recall").copied()),
            metric_cell(row.metrics.get("f1").copied()),
            escape_cell(&row.limitations.join("; "))
        ));
    }
    if run.comparison_rows.is_empty() {
        out.push_str("| _none_ |  |  |  |  |  |  |\n");
    }
    out.push('\n');

    out.push_str("## Scanner Metrics\n\n");
    out.push_str("| TP | FP | FN | TN | Precision | Recall | F1 | FPR |\n|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
        run.metrics.true_positives,
        run.metrics.false_positives,
        run.metrics.false_negatives,
        run.metrics.true_negatives,
        metric_cell(run.metrics.precision),
        metric_cell(run.metrics.recall),
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

    out.push_str("## Adaptation\n\n");
    if let Some(adaptation) = &run.adaptation {
        out.push_str(&format!(
            "- Prompt: `{}`\n- Prompt hash: `{}`\n- Changed artifacts: {}\n\n",
            escape_cell(&adaptation.agent.prompt_path),
            escape_cell(&adaptation.agent.prompt_hash),
            adaptation.outputs.rules_or_extensions_changed.len()
        ));
    } else {
        out.push_str("_none_\n\n");
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

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::competitors::{BenchmarkComparisonRow, ProductIdentity, ResultSource};
    use crate::eval::model::EvaluationMode;
    use crate::eval::performance::{CacheStatsSummary, EvalPerformanceReport, ProviderStatsRow};
    use crate::eval::report::{MetricSections, MetricSummary};
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

        assert!(markdown.contains("adapter_only"));
        assert!(markdown.contains("imported: published table"));
        assert!(markdown.contains("adapter-only: unsupported language"));
        assert!(markdown.contains("known limitation"));
    }

    #[test]
    fn markdown_omits_transient_paths_and_timestamps() {
        let markdown = render_markdown(&report());

        assert!(!markdown.contains("/var/folders"));
        assert!(!markdown.contains("/tmp/polint"));
        assert!(!markdown.contains("2026-05-26T07:00:00Z"));
    }

    fn report() -> EvaluationRun {
        EvaluationRun {
            schema_version: crate::eval::report::EVALUATION_SCHEMA_VERSION.to_string(),
            suite_id: "owasp-java".to_string(),
            mode: EvaluationMode::AdapterOnly,
            suite_manifest: Some(suite_manifest()),
            cases: Vec::new(),
            metrics: MetricSummary {
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
                sections: MetricSections::default(),
            },
            performance: Some(EvalPerformanceReport {
                providers: vec![provider("polint.ts.syntax"), provider("polint.source")],
                cache: CacheStatsSummary::default(),
                demand_queries: Vec::new(),
                runtime: Default::default(),
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
            id: SuiteId("owasp-java".to_string()),
            name: "OWASP Java".to_string(),
            kind: SuiteKind::ScannerVulnerability,
            languages: vec!["java".to_string()],
            adapter_id: "owasp_expected_results_csv".to_string(),
            source_url: None,
            source_commit: None,
            license: "license-review-needed".to_string(),
            language_support: SuiteLanguageSupport::AdapterOnly,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: "research/evaluation-harness/repos/BenchmarkJava".to_string(),
                ignored_by_git: true,
                local_clone_policy: crate::eval::suite::LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::OwaspExpectedResultsCsv,
                path: "expectedresults-1.2.csv".to_string(),
            },
            scoring: SuiteScoring {
                native: vec!["owasp_confusion_matrix".to_string()],
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
            suite_id: SuiteId("owasp-java".to_string()),
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
                    manifest_path: "research/evaluation-harness/suites/owasp-java.toml".to_string(),
                    reason: "unsupported language".to_string(),
                }
            },
            metrics: [("precision".to_string(), 0.5), ("recall".to_string(), 0.25)]
                .into_iter()
                .collect(),
            limitations: vec!["known limitation".to_string()],
        }
    }
}
