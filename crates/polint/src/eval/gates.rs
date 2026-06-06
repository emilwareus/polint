use serde::{Deserialize, Serialize};

use crate::eval::report::{EvaluationRun, PerLanguageDeltaRow};

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) precision_floors: Vec<PromotionPrecisionFloor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_per_language_deltas: Vec<RequiredPerLanguageDelta>,
    pub(crate) max_graph_edge_misses: u64,
    pub(crate) max_path_misses: u64,
    #[serde(default)]
    pub(crate) max_false_positive_trap_hits: u64,
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
            precision_floors: Vec::new(),
            required_per_language_deltas: Vec::new(),
            max_graph_edge_misses: 0,
            max_path_misses: 0,
            max_false_positive_trap_hits: 0,
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
pub(crate) struct PromotionPrecisionFloor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scoring_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) precision_tier: Option<String>,
    pub(crate) min_precision: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct RequiredPerLanguageDelta {
    pub(crate) language: String,
    pub(crate) scoring_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) precision_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_precision_delta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_recall_delta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_f0_5_delta: Option<f64>,
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

    let mut checks = vec![
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
        max_u64_check(
            "false_positive_trap_hits",
            metrics.false_positive_trap_hits,
            config.thresholds.max_false_positive_trap_hits,
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
    checks.extend(precision_floor_checks(
        run,
        &config.thresholds.precision_floors,
    ));
    checks.extend(per_language_delta_checks(
        run,
        &config.thresholds.required_per_language_deltas,
    ));

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

fn precision_floor_checks(
    run: &EvaluationRun,
    floors: &[PromotionPrecisionFloor],
) -> Vec<GateCheck> {
    floors
        .iter()
        .map(|floor| {
            let metric = precision_floor_metric(floor);
            if floor.language.is_none()
                && floor.scoring_mode.is_none()
                && floor.precision_tier.is_none()
            {
                return optional_min_f64_check(&metric, run.metrics.precision, floor.min_precision);
            }

            let row = run
                .metrics
                .sections
                .per_language_deltas
                .iter()
                .find(|row| precision_floor_matches(row, floor));
            optional_min_f64_check(
                &metric,
                row.and_then(|row| row.current_precision),
                floor.min_precision,
            )
        })
        .collect()
}

fn precision_floor_metric(floor: &PromotionPrecisionFloor) -> String {
    [
        Some("precision_floor"),
        floor.language.as_deref(),
        floor.scoring_mode.as_deref(),
        floor.precision_tier.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(".")
}

fn precision_floor_matches(row: &PerLanguageDeltaRow, floor: &PromotionPrecisionFloor) -> bool {
    floor
        .language
        .as_deref()
        .is_none_or(|language| row.language == language)
        && floor
            .scoring_mode
            .as_deref()
            .is_none_or(|scoring_mode| row.scoring_mode == scoring_mode)
        && floor
            .precision_tier
            .as_deref()
            .is_none_or(|precision_tier| row.precision_tier == precision_tier)
}

fn per_language_delta_checks(
    run: &EvaluationRun,
    required: &[RequiredPerLanguageDelta],
) -> Vec<GateCheck> {
    let mut checks = Vec::new();
    for requirement in required {
        let row = run
            .metrics
            .sections
            .per_language_deltas
            .iter()
            .find(|row| delta_requirement_matches(row, requirement));
        if row.is_none() {
            checks.push(GateCheck {
                metric: delta_metric(requirement, "row"),
                observed: "missing".to_string(),
                threshold: "present".to_string(),
                verdict: GateVerdict::Fail,
            });
            continue;
        }
        let row = row.expect("checked is_some above");
        if let Some(threshold) = requirement.min_precision_delta {
            checks.push(optional_min_f64_check(
                &delta_metric(requirement, "precision"),
                row.precision_delta,
                threshold,
            ));
        }
        if let Some(threshold) = requirement.min_recall_delta {
            checks.push(optional_min_f64_check(
                &delta_metric(requirement, "recall"),
                row.recall_delta,
                threshold,
            ));
        }
        if let Some(threshold) = requirement.min_f0_5_delta {
            checks.push(optional_min_f64_check(
                &delta_metric(requirement, "f0_5"),
                row.f0_5_delta,
                threshold,
            ));
        }
    }
    checks
}

fn delta_requirement_matches(
    row: &PerLanguageDeltaRow,
    requirement: &RequiredPerLanguageDelta,
) -> bool {
    row.language == requirement.language
        && row.scoring_mode == requirement.scoring_mode
        && requirement
            .precision_tier
            .as_deref()
            .is_none_or(|precision_tier| row.precision_tier == precision_tier)
}

fn delta_metric(requirement: &RequiredPerLanguageDelta, metric: &str) -> String {
    let mut parts = vec![
        "per_language_delta",
        requirement.language.as_str(),
        requirement.scoring_mode.as_str(),
    ];
    if let Some(precision_tier) = &requirement.precision_tier {
        parts.push(precision_tier);
    }
    parts.push(metric);
    parts.join(".")
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

fn optional_min_f64_check(metric: &str, observed: Option<f64>, threshold: f64) -> GateCheck {
    match observed {
        Some(observed) => min_f64_check(metric, observed, threshold),
        None => GateCheck {
            metric: metric.to_string(),
            observed: "missing".to_string(),
            threshold: format!(">= {threshold:.4}"),
            verdict: GateVerdict::Fail,
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
        GraphMetricSection, MetricSections, MetricSummary, PathMetricSection, PerLanguageDeltaRow,
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

    #[test]
    fn promotion_gates_fail_when_go_precision_floor_is_missed() {
        let mut config = config();
        config.thresholds.precision_floors = vec![PromotionPrecisionFloor {
            language: Some("go".to_string()),
            scoring_mode: Some("oracle-rta".to_string()),
            precision_tier: Some("setup_aware".to_string()),
            min_precision: 0.60,
        }];
        let mut run = gate_run("hash", 0, 0, 0);
        run.metrics.sections.per_language_deltas = vec![delta_row(
            "go",
            "oracle-rta",
            "setup_aware",
            0.59,
            0.0,
            0.0,
            0.0,
        )];

        let report = evaluate_promotion_gates(&run, Some(&run), &config);

        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(report.checks.iter().any(|check| {
            check.metric == "precision_floor.go.oracle-rta.setup_aware"
                && check.observed == "0.5900"
                && check.threshold == ">= 0.6000"
                && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn promotion_gates_pass_when_go_precision_equals_floor() {
        let mut config = config();
        config.thresholds.precision_floors = vec![PromotionPrecisionFloor {
            language: Some("go".to_string()),
            scoring_mode: Some("oracle-rta".to_string()),
            precision_tier: Some("setup_aware".to_string()),
            min_precision: 0.60,
        }];
        let mut run = gate_run("hash", 0, 0, 0);
        run.metrics.sections.per_language_deltas = vec![delta_row(
            "go",
            "oracle-rta",
            "setup_aware",
            0.60,
            0.0,
            0.0,
            0.0,
        )];

        let report = evaluate_promotion_gates(&run, Some(&run), &config);

        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(report.checks.iter().any(|check| {
            check.metric == "precision_floor.go.oracle-rta.setup_aware"
                && check.observed == "0.6000"
                && check.verdict == GateVerdict::Pass
        }));
    }

    #[test]
    fn promotion_gates_support_configurable_jelly_precision_floor() {
        let mut config = config();
        config.thresholds.precision_floors = vec![PromotionPrecisionFloor {
            language: Some("typescript".to_string()),
            scoring_mode: Some("oracle-jelly".to_string()),
            precision_tier: Some("setup_aware".to_string()),
            min_precision: 0.55,
        }];
        let mut run = gate_run("hash", 0, 0, 0);
        run.metrics.sections.per_language_deltas = vec![delta_row(
            "typescript",
            "oracle-jelly",
            "setup_aware",
            0.54,
            0.0,
            0.0,
            0.0,
        )];

        let report = evaluate_promotion_gates(&run, Some(&run), &config);

        assert!(report.checks.iter().any(|check| {
            check.metric == "precision_floor.typescript.oracle-jelly.setup_aware"
                && check.threshold == ">= 0.5500"
                && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn promotion_gates_enforce_per_language_deltas_separately() {
        let mut config = config();
        config.thresholds.required_per_language_deltas = vec![
            RequiredPerLanguageDelta {
                language: "go".to_string(),
                scoring_mode: "oracle-rta".to_string(),
                precision_tier: Some("setup_aware".to_string()),
                min_precision_delta: Some(0.0),
                min_recall_delta: Some(0.25),
                min_f0_5_delta: Some(0.10),
            },
            RequiredPerLanguageDelta {
                language: "typescript".to_string(),
                scoring_mode: "oracle-jelly".to_string(),
                precision_tier: Some("setup_aware".to_string()),
                min_precision_delta: Some(0.0),
                min_recall_delta: Some(0.25),
                min_f0_5_delta: Some(0.10),
            },
        ];
        let mut run = gate_run("hash", 0, 0, 0);
        run.metrics.sections.per_language_deltas = vec![
            delta_row("go", "oracle-rta", "setup_aware", 0.80, 0.0, 0.30, 0.11),
            delta_row(
                "typescript",
                "oracle-jelly",
                "setup_aware",
                0.80,
                0.0,
                0.10,
                0.11,
            ),
        ];

        let report = evaluate_promotion_gates(&run, Some(&run), &config);

        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(report.checks.iter().any(|check| {
            check.metric == "per_language_delta.typescript.oracle-jelly.setup_aware.recall"
                && check.observed == "0.1000"
                && check.verdict == GateVerdict::Fail
        }));
        assert!(report.checks.iter().any(|check| {
            check.metric == "per_language_delta.go.oracle-rta.setup_aware.recall"
                && check.verdict == GateVerdict::Pass
        }));
    }

    #[test]
    fn promotion_gates_fail_when_required_delta_row_is_missing() {
        let mut config = config();
        config.thresholds.required_per_language_deltas = vec![RequiredPerLanguageDelta {
            language: "go".to_string(),
            scoring_mode: "oracle-rta".to_string(),
            precision_tier: Some("setup_aware".to_string()),
            min_precision_delta: None,
            min_recall_delta: Some(0.25),
            min_f0_5_delta: None,
        }];
        let run = gate_run("hash", 0, 0, 0);

        let report = evaluate_promotion_gates(&run, Some(&run), &config);

        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(report.checks.iter().any(|check| {
            check.metric == "per_language_delta.go.oracle-rta.setup_aware.row"
                && check.observed == "missing"
                && check.threshold == "present"
        }));
    }

    #[test]
    fn promotion_gates_reject_false_positive_trap_flooding() {
        let mut run = gate_run("hash", 0, 0, 0);
        run.metrics.true_positives = 10;
        run.metrics.false_positives = 8;
        run.metrics.false_negatives = 1;
        run.metrics.precision = Some(10.0 / 18.0);
        run.metrics.recall = Some(10.0 / 11.0);
        run.metrics.false_positive_trap_hits = 1;

        let report = evaluate_promotion_gates(&run, Some(&run), &config());

        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(report.checks.iter().any(|check| {
            check.metric == "false_positive_trap_hits"
                && check.observed == "1"
                && check.threshold == "<= 0"
                && check.verdict == GateVerdict::Fail
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
            adaptation_delta: None,
            limitations: Vec::new(),
            output_hash: hash.to_string(),
        }
    }

    fn delta_row(
        language: &str,
        scoring_mode: &str,
        precision_tier: &str,
        current_precision: f64,
        precision_delta: f64,
        recall_delta: f64,
        f0_5_delta: f64,
    ) -> PerLanguageDeltaRow {
        PerLanguageDeltaRow {
            language: language.to_string(),
            suite_id: "native-promotion".to_string(),
            scoring_mode: scoring_mode.to_string(),
            precision_tier: precision_tier.to_string(),
            baseline_precision: Some(current_precision - precision_delta),
            current_precision: Some(current_precision),
            precision_delta: Some(precision_delta),
            baseline_recall: Some(0.25),
            current_recall: Some(0.25 + recall_delta),
            recall_delta: Some(recall_delta),
            baseline_f0_5: Some(0.50),
            current_f0_5: Some(0.50 + f0_5_delta),
            f0_5_delta: Some(f0_5_delta),
        }
    }
}
