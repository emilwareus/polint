use serde::{Deserialize, Serialize};

use crate::eval::report::EvaluationRun;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GateVerdict {
    Pass,
    Warn,
    Fail,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct PromotionGateThresholds {
    pub(crate) min_native_pass_rate: f64,
    pub(crate) max_graph_edge_misses: u64,
    pub(crate) max_path_misses: u64,
    pub(crate) max_unknowns: u64,
    pub(crate) warn_unknowns_above: u64,
    pub(crate) max_rejected_facts: u64,
    pub(crate) max_runtime_budget_failures: u64,
    pub(crate) max_cache_quarantines: u64,
    pub(crate) require_deterministic_output_hash: bool,
}

impl Default for PromotionGateThresholds {
    fn default() -> Self {
        Self {
            min_native_pass_rate: 1.0,
            max_graph_edge_misses: 0,
            max_path_misses: 0,
            max_unknowns: 0,
            warn_unknowns_above: 0,
            max_rejected_facts: 0,
            max_runtime_budget_failures: 0,
            max_cache_quarantines: 0,
            require_deterministic_output_hash: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct SuiteGateConfig {
    pub(crate) suite_id: String,
    pub(crate) tier: String,
    pub(crate) thresholds: PromotionGateThresholds,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct PromotionGateReport {
    pub(crate) suite_id: String,
    pub(crate) tier: String,
    pub(crate) verdict: GateVerdict,
    pub(crate) checks: Vec<GateCheck>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GateCheck {
    pub(crate) metric: String,
    pub(crate) observed: String,
    pub(crate) threshold: String,
    pub(crate) verdict: GateVerdict,
}

pub(crate) fn evaluate_promotion_gates(
    run: &EvaluationRun,
    repeated_run: Option<&EvaluationRun>,
    config: &SuiteGateConfig,
) -> PromotionGateReport {
    let metrics = &run.metrics;
    let graph_misses = metrics
        .sections
        .graph
        .edges_expected
        .saturating_sub(metrics.sections.graph.edges_observed);
    let path_misses = metrics
        .sections
        .paths
        .paths_expected
        .saturating_sub(metrics.sections.paths.paths_observed);
    let cache_quarantines = run
        .performance
        .as_ref()
        .map_or(0, |performance| performance.cache.quarantines);

    let checks = vec![
        min_f64_check(
            "native_pass_rate",
            native_pass_rate(run),
            config.thresholds.min_native_pass_rate,
        ),
        max_u64_check(
            "graph_edge_misses",
            graph_misses,
            config.thresholds.max_graph_edge_misses,
        ),
        max_u64_check(
            "path_misses",
            path_misses,
            config.thresholds.max_path_misses,
        ),
        unknown_budget_check(
            metrics.unknown_count,
            config.thresholds.warn_unknowns_above,
            config.thresholds.max_unknowns,
        ),
        max_u64_check(
            "rejected_facts",
            metrics.facts_rejected,
            config.thresholds.max_rejected_facts,
        ),
        max_u64_check(
            "runtime_budget_failures",
            metrics.runtime_budget_failed,
            config.thresholds.max_runtime_budget_failures,
        ),
        max_u64_check(
            "cache_quarantines",
            cache_quarantines,
            config.thresholds.max_cache_quarantines,
        ),
        determinism_check(
            run,
            repeated_run,
            config.thresholds.require_deterministic_output_hash,
        ),
    ];

    let verdict = checks
        .iter()
        .map(|check| check.verdict)
        .max()
        .unwrap_or(GateVerdict::Pass);

    PromotionGateReport {
        suite_id: config.suite_id.clone(),
        tier: config.tier.clone(),
        verdict,
        checks,
    }
}

fn native_pass_rate(run: &EvaluationRun) -> f64 {
    let passed =
        run.metrics.true_positives + run.metrics.true_negatives + run.metrics.runtime_budget_passed;
    let total = passed
        + run.metrics.false_positives
        + run.metrics.false_negatives
        + run.metrics.runtime_budget_failed;
    if total == 0 {
        1.0
    } else {
        passed as f64 / total as f64
    }
}

fn min_f64_check(metric: &str, observed: f64, threshold: f64) -> GateCheck {
    GateCheck {
        metric: metric.to_string(),
        observed: format!("{observed:.4}"),
        threshold: format!(">= {threshold:.4}"),
        verdict: if observed >= threshold {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

fn max_u64_check(metric: &str, observed: u64, threshold: u64) -> GateCheck {
    GateCheck {
        metric: metric.to_string(),
        observed: observed.to_string(),
        threshold: format!("<= {threshold}"),
        verdict: if observed <= threshold {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

fn unknown_budget_check(observed: u64, warn_above: u64, fail_above: u64) -> GateCheck {
    GateCheck {
        metric: "unknown_count".to_string(),
        observed: observed.to_string(),
        threshold: format!("warn > {warn_above}; fail > {fail_above}"),
        verdict: if observed > fail_above {
            GateVerdict::Fail
        } else if observed > warn_above {
            GateVerdict::Warn
        } else {
            GateVerdict::Pass
        },
    }
}

fn determinism_check(
    run: &EvaluationRun,
    repeated_run: Option<&EvaluationRun>,
    required: bool,
) -> GateCheck {
    let observed = repeated_run.is_some_and(|repeated| repeated.output_hash == run.output_hash);
    GateCheck {
        metric: "deterministic_output_hash".to_string(),
        observed: observed.to_string(),
        threshold: required.to_string(),
        verdict: if !required || observed {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::model::EvaluationMode;
    use crate::eval::performance::{CacheStatsSummary, EvalPerformanceReport};
    use crate::eval::report::{
        GraphMetricSection, MetricSections, MetricSummary, PathMetricSection,
    };

    #[test]
    fn promotion_gates_pass_when_metrics_are_within_thresholds() {
        let run = gate_run("hash", 0, 0, 0);
        let report = evaluate_promotion_gates(&run, Some(&run), &config());

        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.verdict == GateVerdict::Pass)
        );
    }

    #[test]
    fn promotion_gates_fail_with_metric_and_threshold_names() {
        let run = gate_run("hash-a", 2, 1, 3);
        let repeated = gate_run("hash-b", 2, 1, 3);
        let report = evaluate_promotion_gates(&run, Some(&repeated), &config());

        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(report.checks.iter().any(|check| {
            check.metric == "graph_edge_misses"
                && check.observed == "2"
                && check.threshold == "<= 0"
        }));
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.metric == "deterministic_output_hash"
                    && check.verdict == GateVerdict::Fail)
        );
    }

    #[test]
    fn promotion_gates_warn_for_unknowns_below_failure_budget() {
        let mut config = config();
        config.thresholds.warn_unknowns_above = 0;
        config.thresholds.max_unknowns = 5;
        let run = gate_run("hash", 0, 0, 1);
        let report = evaluate_promotion_gates(&run, Some(&run), &config);

        assert_eq!(report.verdict, GateVerdict::Warn);
        assert!(report.checks.iter().any(|check| {
            check.metric == "unknown_count" && check.verdict == GateVerdict::Warn
        }));
    }

    fn config() -> SuiteGateConfig {
        SuiteGateConfig {
            suite_id: "native-promotion".to_string(),
            tier: "fast".to_string(),
            thresholds: PromotionGateThresholds::default(),
        }
    }

    fn gate_run(hash: &str, graph_misses: u64, path_misses: u64, unknowns: u64) -> EvaluationRun {
        let graph_expected = graph_misses + 1;
        let path_expected = path_misses + 1;
        EvaluationRun {
            schema_version: crate::eval::report::EVALUATION_SCHEMA_VERSION.to_string(),
            suite_id: "native-promotion".to_string(),
            mode: EvaluationMode::PolintBaseline,
            suite_manifest: None,
            cases: Vec::new(),
            metrics: MetricSummary {
                true_positives: 1,
                false_positives: 0,
                false_negatives: 0,
                true_negatives: 0,
                unconfirmed: 0,
                false_positive_trap_hits: 0,
                forbidden_hits: 0,
                unknown_count: unknowns,
                facts_present: 1,
                facts_accepted: 0,
                facts_rejected: 0,
                graph_edges_expected: graph_expected,
                graph_edges_observed: 1,
                graph_edges_unconfirmed: 0,
                paths_expected: path_expected,
                paths_observed: 1,
                paths_unconfirmed: 0,
                runtime_budget_passed: 1,
                runtime_budget_failed: 0,
                precision: Some(1.0),
                recall: Some(1.0),
                f1: Some(1.0),
                f2: Some(1.0),
                f3: Some(1.0),
                false_positive_rate: None,
                sections: MetricSections {
                    graph: GraphMetricSection {
                        edges_expected: graph_expected,
                        edges_observed: 1,
                        edges_unconfirmed: 0,
                    },
                    paths: PathMetricSection {
                        paths_expected: path_expected,
                        paths_observed: 1,
                        paths_unconfirmed: 0,
                    },
                    ..MetricSections::default()
                },
            },
            performance: Some(EvalPerformanceReport {
                cache: CacheStatsSummary::default(),
                ..EvalPerformanceReport::default()
            }),
            comparison_rows: Vec::new(),
            adaptation: None,
            limitations: Vec::new(),
            output_hash: hash.to_string(),
        }
    }
}
